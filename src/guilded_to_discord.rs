use crate::error_boxable::*;
use crate::multi_recv::*;
use async_tungstenite::tungstenite::Message as WsMessage;
use http_types::headers::HeaderValues;
use futures::{StreamExt};
use serde_json::{Value as JsValue, from_str as deserialize};
use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
struct Data {
    webhooks: BTreeMap<String, BTreeMap<String, String>>
}

pub async fn guilded_to_discord(guilded_cookies: HeaderValues, from_guilded: MultiRecv<WsMessage>) -> async_std::task::JoinHandle<()> {
    let print_all_msg = std::env::var("print_all_msg").is_ok();
    async_std::task::spawn(from_guilded.for_each(move |msg| async move {
        if let WsMessage::Text(msg) = &*msg {
            if print_all_msg { println!("{}", msg) };
            if let Some(json_begin_at) = msg.find('[') {
                if let Ok(JsValue::Array(contents)) = serde_json::from_str::<JsValue>(&msg[json_begin_at..]) {
                    if contents.len() >= 2 {
                        if let JsValue::String(msg_type) = &contents[0] {
                            match &**msg_type {
                                "ChatMessageCreated" => {
                                    if let Ok(msg) = ChatMessageCreated::deserialize(&contents[1]) { chat_message_created(msg).await };
                                },
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

fn extract_text_from_node(node: &JsValue, out: &mut String) {
    if let JsValue::Object(contents) = node {
        if let Some(JsValue::String(object)) = contents.get("object") {
            match object.as_str() {
                "document" | "inline" => {
                    if let Some(JsValue::Array(nodes)) = contents.get("nodes") {
                        for node in nodes { extract_text_from_node(node, out) };
                    }
                },
                "block" => {
                    if !out.is_empty() { *out += "\n" };
                    if let Some(JsValue::Array(nodes)) = contents.get("nodes") {
                        for node in nodes { extract_text_from_node(node, out) };                        
                    }
                },
                "text"  => {
                    if let Some(JsValue::Array(leaves)) = contents.get("leaves") {
                        for leaf in leaves { extract_text_from_node(leaf, out); }
                    }
                },
                "leaf" => {
                    if let Some(JsValue::String(text)) = contents.get("text") {
                        *out += &text;
                    }
                },
                _ => eprintln!("GD: Unexpected node type {}", object)
            }
        }
    }
}


async fn chat_message_created(msg: ChatMessageCreated) {
    let mut content = String::new();
    extract_text_from_node(&msg.message.content.document, &mut content);
    println!("{}", content);
}
