use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;
use serde_json::{Value as JsValue, from_str as deserialize, to_string as serialize};
use async_std::fs::File;
use std::sync::Arc;
use crate::*;
use async_tungstenite::tungstenite::Message as WsMessage;
use futures::{AsyncReadExt, AsyncWriteExt};

const DATA_FILE: &'static str = "dg_data.json";
#[derive(Serialize, Deserialize, Default)]
struct Data {
    webhooks: BTreeMap<String, BTreeMap<String, String>>
}
impl Data {
    pub async fn save(&self) {
        let mut file = File::create(DATA_FILE).await.expect("Failed to overwrite gd data file");
        file.write_all(serialize(self).expect("Failed to serialize gd data").as_bytes()).await.expect("Failed to write gd data file");
    }
}

pub(crate) async fn discord_to_guilded(env: Arc<Environment>, mut from_discord: MultiRecv<WsMessage>) -> async_std::task::JoinHandle<()> {
    let mut data = Data::default();
    if let Ok(mut data_file) = File::open(DATA_FILE).await {
        let mut data_dat = String::new();
        if let Ok(_) = data_file.read_to_string(&mut data_dat).await {
            if let Ok(data_parsed) = deserialize(&data_dat) {
                data = data_parsed;
            }
        }
    }
    let print_all_msg = std::env::var("print_all_msg").is_ok();

    async_std::task::spawn(async move {

    })
}
