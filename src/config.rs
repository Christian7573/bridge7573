use serde::{Serialize, Deserialize};
use std::sync::Arc;
use std::fs::File;
use std::io::Read;

#[derive(Serialize, Deserialize)]
pub struct Config {
    text_channel_bindings: Vec<ChannelBinding>    
}

impl Config {
    pub fn load_blocking() -> Config {
        let mut string_cause_yes = String::new();
        File::open("config.json").expect("No config.json").read_to_string(&mut string_cause_yes).expect("Died while reading config.json");
        serde_json::from_str(&string_cause_yes).expect("Invalid config.json")
    }
}

#[derive(Serialize, Deserialize)]
pub struct ChannelBinding {
    guilded: String,
    discord: String,
}
