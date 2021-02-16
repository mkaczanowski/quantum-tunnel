use crate::cosmos::types::Coins;
use crate::config::CosmosConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;

pub trait StdMsg {
    fn get_type() -> String
    where
        Self: Sized;
}

/// Payload to initialize substrate light client
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MsgCreateWasmClient<T> {
    pub client_id: String,
    pub header: T,
    pub trusting_period: String,
    pub unbonding_period: String,
    pub max_clock_drift: String,
    pub address: String,
    #[serde(with = "crate::utils::from_str")]
    pub wasm_id: u32,
}

pub trait WasmHeader {
    fn chain_name() -> &'static str;
    fn height(&self) -> u64;

    fn to_wasm_create_msg(&self, cfg: &CosmosConfig, address: String, client_id: String) -> Result<Vec<Value>, Box<dyn Error>>;
    fn to_wasm_update_msg(&self, address: String, client_id: String) -> Vec<Value>;
}

impl<T> StdMsg for MsgCreateWasmClient<T> {
    fn get_type() -> String {
        "ibc/client/MsgCreateWasmClient".to_owned()
    }
}

/// Payload to update substrate light client
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MsgUpdateWasmClient<T>{
    pub client_id: String,
    pub header: T,
    pub address: String,
}

impl<T> StdMsg for MsgUpdateWasmClient<T> {
    fn get_type() -> String {
        "ibc/client/MsgUpdateWasmClient".to_owned()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MsgSend {
    pub from_address: String,
    pub to_address: String,
    pub amount: Coins,
}

impl StdMsg for MsgSend {
    fn get_type() -> String {
        "cosmos-sdk/MsgSend".to_owned()
    }
}
