use crate::config::CosmosConfig;
use crate::cosmos::types::{StdMsg, MsgCreateWasmClient, MsgUpdateWasmClient, WasmHeader};
use serde::{Deserialize, Serialize};
use celo_light_client::{
    Header as CeloHeader,
    StateEntry,
    ToRlp,
};
use serde_json::Value;
use parse_duration::parse;
use num::cast::ToPrimitive;
use std::error::Error;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InitMsg {
    pub header: Vec<u8>,
    pub initial_state_entry: Vec<u8>,
}

// FIXME: The WASM update message can't deserialize Vec<u8> on the contract end. To me it looks
// like a bug? especially the CreateMsg works fine with vector of bytes.
//
// The workaround is to serialize to RLP and then to hex string
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UpdateMsg {
    pub header: String,
}

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

    fn to_wasm_create_msg(&self, cfg: &CosmosConfig, address: String, client_id:String) -> Result<Vec<Value>, Box<dyn Error>> {
        let msg = MsgCreateWasmClient {
            header: InitMsg {
                header: self.header.to_rlp(),
                initial_state_entry: self.initial_state_entry.to_rlp(),
            },
            address,
            trusting_period: parse(&cfg.trusting_period)?
                .as_nanos()
                .to_string(),
            max_clock_drift: parse(&cfg.max_clock_drift)?
                .as_nanos()
                .to_string(),
            unbonding_period: parse(&cfg.unbonding_period)?
                .as_nanos()
                .to_string(),
            client_id,
            wasm_id: cfg.wasm_id,
        };

        Ok(vec![serde_json::json!({"type": MsgCreateWasmClient::<Self>::get_type(), "value": &msg})])
    }

    fn to_wasm_update_msg(&self, address: String, client_id: String) -> Vec<Value> {
        let msg = MsgUpdateWasmClient {
            header: UpdateMsg {
                header: hex::encode(self.header.to_rlp().to_owned())
            },
            address,
            client_id,
        };

        vec![serde_json::json!({"type": MsgUpdateWasmClient::<Self>::get_type(), "value": &msg})]
    }
}
