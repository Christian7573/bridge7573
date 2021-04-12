use serde::{Serialize, Deserialize};
use std::sync::Arc;
use std::fs::File;
use std::io::Read;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    pub text_channel_bindings: Vec<ChannelBinding>,
}

pub struct Config {
    pub text_channel_bindings: Vec<ChannelBinding>,
    pub text_channel_gd: BTreeMap<String, String>,
    pub text_channel_dg: BTreeMap<String, String>,
}

impl Config {
    pub fn load_blocking() -> Config {
        let mut string_cause_yes = String::new();
        File::open("config.json").expect("No config.json").read_to_string(&mut string_cause_yes).expect("Died while reading config.json");
        let raw = serde_json::from_str::<RawConfig>(&string_cause_yes).expect("Invalid config.json");
        
        Config {
            text_channel_gd: raw.text_channel_bindings.iter().map(|binding| (binding.guilded.to_owned(), binding.discord.to_owned())).collect(),
            text_channel_dg: raw.text_channel_bindings.iter().map(|binding| (binding.discord.to_owned(), binding.guilded.to_owned())).collect(),
            text_channel_bindings: raw.text_channel_bindings,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ChannelBinding {
    guilded: String,
    discord: String,
}
