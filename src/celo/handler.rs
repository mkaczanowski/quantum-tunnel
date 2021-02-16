use crate::config::{CeloChainConfig, CeloConfig};
use crate::cosmos::types::TMHeader;
use celo_light_client::Header as CeloHeader;
use crate::utils::to_string;
use crossbeam_channel::{Receiver, Sender, TryRecvError};
use futures::{SinkExt, StreamExt};
use log::*;
use std::path::Path;
use std::string::ToString;
use tokio_tungstenite::connect_async;
use serde_json::{from_str, Value};

pub struct CeloHandler {}
impl CeloHandler {
    /// Receive handler entrypoint
    /// Branches to different internal methods depending upon whether
    /// configuration is `Real` or `Simulation`
    pub async fn recv_handler(
        cfg: CeloChainConfig,
        outchan: Sender<CeloHeader>,
        monitoring_inchan: Receiver<(bool, u64)>,
    ) -> Result<(), String> {
        match cfg {
            CeloChainConfig::Real(cfg) => Self::chain_recv_handler(cfg, outchan).await,
            CeloChainConfig::Simulation(cfg) => {
                Self::simulate_recv_handler(
                    cfg.simulation_file_path,
                    cfg.should_run_till_height,
                    outchan,
                    monitoring_inchan,
                )
                .await
            }
        }
    }

    /// Simulation receive handler, which as the name suggests
    /// take the chain headers from simulation target instead of
    /// live chain. It also monitors send handler of opposite chain to detect
    /// whether or not simulation is successful.
    pub async fn simulate_recv_handler(
        test_file: String,
        should_run_till_height: u64,
        outchan: Sender<CeloHeader>,
        monitoring_inchan: Receiver<(bool, u64)>,
    ) -> Result<(), String> {
        let simulation_data =
            std::fs::read_to_string(Path::new(test_file.as_str())).map_err(to_string)?;
        let stringified_headers: Vec<&str> = simulation_data.split("\n\n").collect();
        let number_of_simulated_headers = stringified_headers.len();
        for str in stringified_headers {
            let payload = from_str(str).map_err(to_string)?;
            outchan.try_send(payload).map_err(to_string)?;
        }

        let mut number_of_headers_ingested_till = 0;
        let mut successfully_ingested_till = 0;
        // Let's wait for the receive handler on other side to catch up
        loop {
            let result = monitoring_inchan.try_recv();
            if result.is_err() {
                match result.err().unwrap() {
                    TryRecvError::Empty => {
                        // Let's wait for data to appear
                        tokio::time::delay_for(core::time::Duration::new(1, 0)).await;
                    }
                    TryRecvError::Disconnected => {
                        return Err(
                            "monitoring channel of substrate send handler is disconnected"
                                .to_string(),
                        );
                    }
                }
                continue;
            }

            let (terminated, reported_height) = result.unwrap();
            if !terminated {
                successfully_ingested_till = reported_height;
                number_of_headers_ingested_till += 1;
            }

            if terminated || (number_of_headers_ingested_till == number_of_simulated_headers) {
                if successfully_ingested_till != should_run_till_height {
                    return Err(format!("Ingesting simulation data failed on cosmos chain. Expected to ingest headers till height: {}, ingested till: {}", should_run_till_height, successfully_ingested_till));
                } else {
                    info!(
                        "Celo headers simulated successfully. Ingested headers till height: {}",
                        successfully_ingested_till
                    );
                }
                break;
            } else {
                info!(
                    "Cosmos light client has successfully ingested header at: {}",
                    successfully_ingested_till
                );
            }
        }
        Ok(())
    }

    /// Subscribes to new blocks from Websocket, and pushes CeloHeader objects into the Channel.
    pub async fn chain_recv_handler(
        cfg: CeloConfig,
        outchan: Sender<CeloHeader>,
    ) -> Result<(), String> {
        let (mut socket, _) = connect_async(&cfg.ws_addr).await.map_err(to_string)?;
        info!("connected websocket to {:?}", &cfg.ws_addr);
        let subscribe_message = tokio_tungstenite::tungstenite::Message::Text(r#"{"id": 0, "method": "eth_subscribe", "params": ["newHeads"]}"#.to_string());
        socket.send(subscribe_message).await.map_err(to_string)?;

        async fn process_msg(
            msg: tokio_tungstenite::tungstenite::Message,
        ) -> Result<CeloHeader, String> {
            let msgtext = msg.to_text().map_err(to_string)?;
            let json = from_str::<Value>(msgtext).map_err(to_string)?;
            let raw_header = json["params"]["result"].to_string();
            let header: CeloHeader = serde_json::from_slice(&raw_header.as_bytes()).map_err(to_string)?;

            Ok(header)
        }

        while let Some(msg) = socket.next().await {
            if let Ok(msg) = msg {
                info!("Received message from celo chain: {:?}", msg);
                match process_msg(msg.clone()).await {
                    Ok(celo_header) => outchan
                        .try_send(celo_header)
                        .map_err(to_string)?,
                    Err(err) => error!("Error: {}", err),
                }
            }
        }

        Ok(())
    }

    /// Send handler entrypoint
    /// Branches to different internal methods depending upon whether
    /// configuration is `Real` or `Simulation`
    /// If other side is simulation, some additional bookkeeping is done to
    /// make sure `simulation_recv_handler` gets accurate data.
    pub async fn send_handler(
        cfg: CeloChainConfig,
        client_id: Option<String>,
        inchan: Receiver<(TMHeader, Vec<tendermint::validator::Info>)>,
        monitoring_outchan: Sender<(bool, u64)>,
    ) -> Result<(), String> {
        match cfg {
            CeloChainConfig::Real(cfg) => {
                if cfg.is_other_side_simulation {
                    // Swallow up the error to prevent quantum tunnel to terminate. This will give simulation data reader the chance to print the result.
                    let result = Self::chain_send_handler(
                        cfg,
                        client_id,
                        inchan,
                        monitoring_outchan.clone(),
                    )
                    .await;
                    // Send signal to simulation_recv_handler that receive handler is terminated
                    monitoring_outchan.try_send((true, 0)).map_err(to_string)?;
                    if result.is_err() {
                        error!("Error occurred while trying to send simulated cosmos data to celo chain: {}", result.err().unwrap());
                    }
                    // This gives simulation_recv_handler time to print result and then exit.
                    futures::future::pending::<()>().await;
                    Ok(())
                } else {
                    Self::chain_send_handler(cfg, client_id, inchan, monitoring_outchan).await
                }
            }
            // If we are running simulation, we cannot ingest any headers.
            CeloChainConfig::Simulation(_cfg) => {
                loop {
                    let result = inchan.try_recv();
                    if result.is_err() {
                        match result.err().unwrap() {
                            TryRecvError::Disconnected => {
                                return Err(
                                    "cosmos chain-data channel's input end is disconnected."
                                        .to_string(),
                                );
                            }
                            _ => {}
                        }
                    }
                    // Compulsory delay of 1 second to not enter in busy loop.
                    tokio::time::delay_for(core::time::Duration::new(1, 0)).await;
                }
            }
        }
    }

    /// Transforms header data received from opposite chain to
    /// light client payload and sends it to tendermint light client running in
    /// substrate chain.
    /// If client id is not passed, first payload sent would be for creating the client.
    pub async fn chain_send_handler(
        cfg: CeloConfig,
        _client_id: Option<String>,
        inchan: Receiver<(TMHeader, Vec<tendermint::validator::Info>)>,
        monitoring_outchan: Sender<(bool, u64)>,
    ) -> Result<(), String> {
        let mut new_client = false;
        // TODO: We don't have a CosmosClient (tendermint-light-client) running on the CeloBlockchain yet,
        // so this is just a stub method.
        //
        // Cosmos[CeloLightWasm] in: Celo out: Cosmos <---> in: Cosmos, out: Celo Celo[TendermintLight?]
        // ^^ we implement this                             !!^^ not this

        loop {
            let result = inchan.try_recv();
            let msg = if result.is_err() {
                match result.err().unwrap() {
                    TryRecvError::Disconnected => {
                        return Err(
                            "cosmos chain-data channel's input end is disconnected.".to_string()
                        );
                    }
                    _ => {
                        warn!("Did not receive any data from Cosmos chain-data channel. Retrying in a second ...");
                        tokio::time::delay_for(core::time::Duration::new(1, 0)).await;
                        continue;
                    }
                }
            } else {
                result.unwrap()
            };
            let current_height = msg.0.signed_header.header.height.value();

            if new_client {
                new_client = false;
                info!("Created Cosmos light client");
            } else {
                info!(
                    "{}",
                    format!(
                        "Updating Cosmos light client with block at height: {}",
                        current_height
                    )
                );
                info!("Updated Cosmos light client");
            }
            if cfg.is_other_side_simulation {
                monitoring_outchan
                    .try_send((false, current_height))
                    .map_err(to_string)?;
            }
        }
    }
}
