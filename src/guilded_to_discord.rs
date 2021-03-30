use crate::error_boxable::*;
use crate::multi_recv::*;
use async_tungstenite::tungstenite::Message as WsMessage;
use http_types::headers::HeaderValues;
use futures::{StreamExt};
use serde_json::{Value as JsValue, from_str as deserialize, to_string as serialize};
use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use async_std::fs::File;
use futures::AsyncReadExt;
use futures::AsyncWriteExt;

use crate::*;

const DATA_FILE: &'static str = "gd_data.json";
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

pub async fn guilded_to_discord(env: Arc<Environment>, mut from_guilded: MultiRecv<WsMessage>) -> async_std::task::JoinHandle<()> {
    let mut data = Data::default();
    if let Ok(data_file) = File::open(DATA_FILE).await {
        let mut data_dat = String::new();
        if let Ok(_) = data_file.read_to_string(&mut data_dat).await {
            if let Ok(data_parsed) = deserialize(&data_dat) {
                data = data_parsed;
            }
        }
    }
    let print_all_msg = std::env::var("print_all_msg").is_ok();

    async_std::task::spawn(async move {
        while let Some(msg) = from_guilded.next().await {
            if let WsMessage::Text(msg) = &*msg {
                if print_all_msg { println!("{}", msg) };
                if let Some(json_begin_at) = msg.find('[') {
                    if let Ok(JsValue::Array(contents)) = serde_json::from_str::<JsValue>(&msg[json_begin_at..]) {
                        if contents.len() >= 2 {
                            if let JsValue::String(msg_type) = &contents[0] {
                                match &**msg_type {
                                    "ChatMessageCreated" => {
                                        if let Ok(msg) = ChatMessageCreated::deserialize(&contents[1]) { chat_message_created(&env, &mut data, msg).await };
                                    },
                                    _ => (),
                                }
                            }
                        }
                    }
                }
            }
        }
    })
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


async fn chat_message_created(env: &Arc<Environment>, data: &mut Data, msg: ChatMessageCreated) {
    let mut content = String::new();
    extract_text_from_node(&msg.message.content.document, &mut content);
    println!("{}", content);
}

async fn get_webhook<'a>(env: &Arc<Environment>, data: &'a mut Data, guilded_user: &str, discord_channel: &str) -> Result<&'a str, ErrorBox> {
    //Get from database
    if data.webhooks.get(discord_channel).is_none() { data.webhooks.insert(discord_channel.to_owned(), BTreeMap::new()); }
    let channel = data.webhooks.get_mut(discord_channel).unwrap();
    if let Some(webhook) = channel.get(guilded_user) { return Ok(webhook) };

    #[derive(Serialize, Deserialize)]
    struct UserData {
        name: String,
        #[serde(rename = "profilePictureSm")]
        avatar: Option<String>,
    }

    //Get user data
    let mut user_response = surf::get(format!("{}/users/{}", GUILDED_API, guilded_user))
        .header("Cookie", &env.guilded_cookies)
        .send().await?;
    if !user_response.status().is_success() { return Err(format!("GD Make Webhook: Failed to fetch guilded user {}: {}", guilded_user, user_response.status()).into()) };
    let user = user_response.body_json::<UserData>().await?;
    let avatar = if let Some(avatar) = &user.avatar {
        let avatar_data = surf::get(avatar)
            .header("Cookie", &env.guilded_cookies)
            .send().await?;

    } else { None };

    struct CreateWebhook {
                
    }

    //Create with discord
    /*let response = surf::post(format!("{}/channels/{}/webhooks", DISCORD_API, discord_channel))
        .header("Authorization", env.discord_auth_header)
        .body().await;*/
        
    todo!()
}
