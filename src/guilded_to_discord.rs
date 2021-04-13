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
use surf::Body;

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

pub(crate) async fn guilded_to_discord(env: Arc<Environment>, mut from_guilded: MultiRecv<WsMessage>) -> async_std::task::JoinHandle<()> {
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
        while let Some(msg) = from_guilded.next().await {
            if let WsMessage::Text(msg) = &*msg {
                if print_all_msg { println!("{}", msg) };
                if let Some(json_begin_at) = msg.find('[') {
                    if let Ok(JsValue::Array(contents)) = serde_json::from_str::<JsValue>(&msg[json_begin_at..]) {
                        if contents.len() >= 2 {
                            if let JsValue::String(msg_type) = &contents[0] {
                                match &**msg_type {
                                    "ChatMessageCreated" => {
                                        match ChatMessageCreated::deserialize(&contents[1]) {
                                            Ok(msg) => { chat_message_created(&env, &mut data, msg).await },
                                            Err(err) => { eprintln!("Failed to deserialize ChatMessageCreated\n{}", err); },
                                        }
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
    content: GuildedMessageContent,
    #[serde(rename = "webhookId")]
    webhook_id: Option<String>,
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
    if msg.message.webhook_id.is_some() { return };
    let discord_channel = if let Some(c) = get_linked_discord_channel(env, data, &msg.channel_id) { c } else { return };
    let mut content = String::new();
    extract_text_from_node(&msg.message.content.document, &mut content);
    let webhook = match get_webhook(env, data, &msg.author, &discord_channel).await {
        Ok(w) => w, 
        Err(err) => { eprintln!("GD Chat Message Get Webhook: {:?}", err); return; }
    };
    
    #[derive(Serialize, Deserialize)]
    struct WebhookMessage {
        content: String,
    }
    let body = WebhookMessage {
        content
    };
    let response = surf::post(webhook)
        .header("Content-Type", "application/json")
        .body(Body::from_json(&body).expect("How did we get here")).await;
    match response {
        Ok(response) => {
            if !response.status().is_success() { eprintln!("GD Chat Message Message for user {} was not success: {}", msg.author, response.status()) };
            //Woo guess it worked lol
        },
        Err(err) => { eprintln!("GD Chat Mesage Send Message: {:?}", err); return; }
    }
}

fn get_linked_discord_channel<'e>(env: &'e Arc<Environment>, _data: &mut Data, guilded_channel: &str) -> Option<&'e str> {
    env.config.text_channel_gd.get(guilded_channel).map(|s| &**s)    
}

async fn get_webhook<'d>(env: &Arc<Environment>, data: &'d mut Data, guilded_user: &str, discord_channel: &str) -> Result<String, ErrorBox> {
    //Get from database
    if data.webhooks.get(discord_channel).is_none() { data.webhooks.insert(discord_channel.to_owned(), BTreeMap::new()); }
    let channel = data.webhooks.get_mut(discord_channel).unwrap();
    if let Some(webhook) = channel.get(guilded_user) { return Ok(webhook.to_owned()) };

    #[derive(Serialize, Deserialize)]
    struct UserResponse {
        user: UserData,
    }
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
    let user = user_response.body_json::<UserResponse>().await?.user;
    let avatar = if let Some(avatar) = &user.avatar {
        let mut avatar_response = surf::get(avatar)
            .header("Cookie", &env.guilded_cookies)
            .send().await?;
        if let Some(header) = avatar_response.header("Content-Type").map(|res| res[0].to_string()) {
            Some(format!("data:{};base64,{}", header, base64::encode(avatar_response.body_bytes().await?)))
        } else {
            return Err(format!("GD Make Webhook: Failed to fetch avatar for guilded user {}: {}", guilded_user, avatar_response.status()).into())
        }
    } else { None };

    //Create with discord
    #[derive(Serialize, Deserialize)]
    struct CreateWebhook {
        name: String,
        avatar: Option<String>
    }
    #[derive(Serialize, Deserialize)]
    struct WebhookResponse {
        id: String,
        token: String,
    }
    let body = CreateWebhook {
        name: format!("📀 {}", user.name),
        avatar
    };
    let mut response = surf::post(format!("{}/channels/{}/webhooks", DISCORD_API, discord_channel))
        .header("Authorization", &env.discord_auth_header)
        .body(Body::from_json(&body)?).await?;
    if !response.status().is_success() { return Err (format!("GD Make Webhook: Webhook creation response for guilded user {} is not success: {}", guilded_user, response.status()).into()) };
    let created_webhook = response.body_json::<WebhookResponse>().await?;

    let webhook = format!("https://discord.com/api/webhooks/{}/{}", created_webhook.id, created_webhook.token);
    data.webhooks.get_mut(discord_channel).unwrap().insert(guilded_user.to_owned(), webhook.clone());        
    data.save().await;
    Ok(webhook)
}
