use crate::error_boxable::*;
use crate::multi_recv::*;
use async_tungstenite::tungstenite::Message as WsMessage;
use http_types::headers::HeaderValues;
use futures::{StreamExt};
use serde_json::Value as JsValue;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct Data {

}

pub async fn guilded_to_discord(guilded_cookies: HeaderValues, from_guilded: MultiRecv<WsMessage>) -> async_std::task::JoinHandle<()> {
    async_std::task::spawn(from_guilded.for_each(|msg| async move {
        if let WsMessage::Text(msg) = &*msg {
            if let Some(json_begin_at) = msg.find('[') {
                if let Ok(JsValue::Array(contents)) = serde_json::from_str::<JsValue>(&msg[json_begin_at..]) {
                    if contents.len() > 2 {
                        if let JsValue::String(msg_type) = &contents[0] {
                            match &**msg_type {
                                "ChatMessageCreated" => { },
                                _ => (),
                            }
                        }
                    }
                }
            }
        }
    }))
}

#[derive(Serialize, Deserialize)]
struct ChatMessageCreated {
    #[serde(rename = "channelId")]
    channel_id: String,
    #[serde(rename = "contentType")]
    content_type: String,
    message: GuildedMessage,
    #[serde(rename = "createdBy")]
    author: String,
}

#[derive(Serialize, Deserialize)]
struct GuildedMessage {
    #[serde(rename = "type")]
    msg_type: String,
    content: GuildedMessageContent    
}

#[derive(Serialize, Deserialize)]
struct GuildedMessageContent {
    document: JsValue
}


async fn chat_message_created(msg: ChatMessageCreated) {

}
