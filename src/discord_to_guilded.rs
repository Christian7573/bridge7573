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
        #[derive(Deserialize)]
        struct IncomingMsg {
            op: i64,
            t: String,
            d: JsValue
        }

        while let Some(msg) = from_discord.next().await {
            if let WsMessage::Text(msg) = &*msg {
                if let Ok(msg) = deserialize::<IncomingMsg>(&msg) {
                   if msg.op == 0 {
                       match &*msg.t {
                           "MESSAGE_CREATE" => {
                               if let Ok(msg) = DiscordMessage::deserialize(msg.d) {
                                   message_created(&env, &mut data, msg).await;
                               }
                           }
                           _ => ()
                       };
                   }
                }
            }
        }

    })
}

#[derive(Deserialize)]
struct DiscordMessage {
    //id: String,
    channel_id: String,
    author: DiscordUser,
    webhook_id: Option<String>,
    content: Option<String>,
}
#[derive(Deserialize)]
struct DiscordUser {
    id: String,
    username: String,
    avatar: Option<String>,
}

async fn message_created(env: &Arc<Environment>, data: &mut Data, msg: DiscordMessage) {
    if msg.webhook_id.is_some() || msg.content.is_none() { return };
    let guilded_channel = if let Some(c) = get_linked_guilded_channel(env, data, &msg.channel_id) { c } else { return };
    let webhook = match get_webhook(env, data, &msg.author, guilded_channel).await {
        Ok(w) => w, 
        Err(err) => { eprintln!("GD Chat Message Get Webhook: {:?}", err); return; }
    };

    #[derive(Serialize)]
    struct ToWebhook {
        content: String,
    }
    let body = ToWebhook {
        content: msg.content.unwrap()
    };
    let response = surf::post(webhook)
        .header("Content-Type", "application/json")
        .body(surf::Body::from_json(&body).expect("How did we get here?")).await;
}

fn get_linked_guilded_channel<'e>(env: &'e Arc<Environment>, _data: &mut Data, discord_channel: &str) -> Option<&'e str> {
    env.config.text_channel_dg.get(discord_channel).map(|s| &**s)
}

async fn get_webhook(env: &Arc<Environment>, data: &mut Data, user: &DiscordUser, guilded_channel: &str) -> Result<String, ErrorBox> {
    //Get from database
    if data.webhooks.get(guilded_channel).is_none() { data.webhooks.insert(guilded_channel.to_owned(), BTreeMap::new()); }
    let channel = data.webhooks.get_mut(guilded_channel).unwrap();
    if let Some(webhook) = channel.get(&user.id) { return Ok(webhook.to_owned()) };

    /*let avatar = if let Some(avatar_hash) = user.avatar.as_ref() {
        let mut response = surf::get(format!("https://cdn.discordapp.com/avatars/{}/{}.png?size=512", user.id, avatar_hash))
            .send().await?;
        if !response.status().is_success() { return Err(format!("DG Make Webhook: Failed to get avatar for user {}: {}", user.id, response.status()).into()) };
        let bytes = response.body_bytes().await?;
        todo!()
    } else { None };*/

    #[derive(Serialize)]
    struct CreateWebhookBody {
        #[serde(rename="channelId")]
        channel: String,
        name: String,
        #[serde(rename="iconUrl")]
        avatar_url: Option<String>,
    }
    let body = CreateWebhookBody {
        channel: guilded_channel.to_owned(),
        name: format!("💬 {}", user.username),
        //avatar_url: user.avatar.as_ref().map(|avatar_hash| format!("https://cdn.discordapp.com/avatars/{}/{}.png?size=512", user.id, avatar_hash))
        avatar_url: None,
    };

    let mut response = surf::post(format!("{}/webhooks", GUILDED_API))
        .header("Content-Type", "application/json")
        .header("Cookie", &env.guilded_cookies)
        .body(surf::Body::from_json(&body)?).await?;
    if !response.status().is_success() { return Err(format!("DG Make Webhook: Failed to make webhook for user {} in channel {}: {}", user.id, guilded_channel, response.status()).into()) };

    #[derive(Deserialize)]
    struct CreateWebhookResponse {
        id: String,
        token: String,
    }
    let webhook = response.body_json::<CreateWebhookResponse>().await?;
    let webhook = format!("https://media.guilded.gg/webhooks/{}/{}", webhook.id, webhook.token);
    data.webhooks.get_mut(guilded_channel).unwrap().insert(user.id.to_owned(), webhook.clone());        
    data.save().await;
    Ok(webhook)
}

/*async fn upload_avatar(env: &Arc<Environment>, data: &mut Data, png_bytes: &[u8]) -> Result<String, ErrorBox> {
 * guilded media uploads are wack
    
    todo!()
}*/
