use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    text_channel_bindings: Vec<ChannelBinding>    
}

#[derive(Serialize, Deserialize)]
pub struct ChannelBinding {

}
