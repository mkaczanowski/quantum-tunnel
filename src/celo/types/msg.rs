use crate::config::CosmosConfig;
use crate::utils::prost_serialize;
use crate::cosmos::types::{StdMsg, MsgCreateWasmClient, MsgUpdateWasmClient, WasmHeader};
use crate::cosmos::proto::{
    ibc::core::commitment::v1::MerkleRoot,
    ibc::lightclients::wasm::v1::{ClientState, ConsensusState, Header as IbcWasmHeader},
    ibc::core::client::v1::{MsgCreateClient, MsgUpdateClient, Height},
};
use celo_light_client::{
    contract::msg::{InitMsg, ClientStateData},
    Header as CeloHeader,
    StateEntry,
    ToRlp,
};
use serde::{Deserialize, Serialize};
use num::cast::ToPrimitive;
use prost::Message as ProstMessage;
use prost_types::Any;
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CeloWrappedHeader {
    pub header: CeloHeader,
    pub initial_state_entry: StateEntry,
}

impl WasmHeader for CeloWrappedHeader {
    fn chain_name() -> &'static str {
        "Celo"
    }

    fn height(&self) -> u64 {
        self.header.number.to_u64().unwrap()
    }

    fn to_wasm_create_msg(&self, cfg: &CosmosConfig, address: String) -> Result<Vec<Any>, Box<dyn Error>> {
        let code_id = hex::decode(&cfg.wasm_id)?;

        let client_state_data = ClientStateData {
            max_clock_drift: parse_duration::parse(cfg.max_clock_drift.as_str())?.as_secs(),
        };

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        let client_state = ClientState {
            data: client_state_data.to_rlp(),
            code_id: code_id.clone(),
            frozen: false,
            frozen_height: None,
            latest_height: Some(Height {
                revision_number: 0,
                revision_height: 0,
            }),
            r#type: "wasm_dummy".to_string(),
        };

        let init_msg = InitMsg {
            header: self.header.to_rlp(),
            initial_state_entry: self.initial_state_entry.to_rlp(),
        };

        let consensus_state = ConsensusState {
            data: init_msg.to_rlp(),
            code_id,
            timestamp,
            root: Some(MerkleRoot {
                hash: vec![1,2,3, 4], // TODO: this is required to not be empty
            }),
            r#type: "wasm_dummy".to_string()
        };

        let msg = MsgCreateClient {
            client_state: Some(Any {
                type_url: "/ibc.lightclients.wasm.v1.ClientState".to_string(),
                value: prost_serialize(&client_state)?,
            }),
            consensus_state: Some(Any {
                type_url: "/ibc.lightclients.wasm.v1.ConsensusState".to_string(),
                value: prost_serialize(&consensus_state)?,
            }),
            signer: address,
        };

        let mut serialized_msg = Vec::new();
        msg.encode(&mut serialized_msg)?;

        Ok(vec![
            Any {
                type_url: MsgCreateWasmClient::<Self>::get_type(),
                value: serialized_msg,
            }
        ])

        // TODO: what about the max_clock_drift, unbonding_period and trusting_period?
    }

    fn to_wasm_update_msg(&self, address: String, client_id: String) -> Result<Vec<Any>, Box<dyn Error>> {
        let header = IbcWasmHeader {
            data: self.header.to_rlp().to_owned(),
            height: Some(Height {
                revision_number: 0,
                revision_height: 0,
            }),
            r#type: "wasm_dummy".to_string()
        };

        let msg = MsgUpdateClient {
            client_id,
            header: Some(Any {
                type_url: "/ibc.lightclients.wasm.v1.Header".to_string(),
                value: prost_serialize(&header)?,
            }),
            signer: address,
        };

        Ok(vec![
            Any {
                type_url: MsgUpdateWasmClient::<Self>::get_type(),
                value: prost_serialize(&msg)?,
            }
        ])
    }
}
