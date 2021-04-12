#[macro_use] extern crate futures;
use async_std::sync::Mutex;
use serde::{Serialize, Deserialize};
use http_types::headers::HeaderValues;
use serde_json::Value as JsValue;
use async_tungstenite::tungstenite::Message;
use async_tungstenite::WebSocketStream;
use async_std::channel::{Sender, unbounded};
use futures::{StreamExt, SinkExt, FutureExt};
use std::time::Duration;
use std::sync::Arc;

mod multi_recv;
mod error_boxable;
mod config;
use multi_recv::*;
use error_boxable::*;
use config::*;

mod guilded_to_discord;
mod discord_to_guilded;

struct Environment {
    guilded_email: String,
    guilded_password: String,
    discord_auth_header: String,
    guilded_cookies: HeaderValues,
    config: Config,
}

pub const GUILDED_API: &'static str = "https://www.guilded.gg/api";
pub const DISCORD_API: &'static str = "https://discord.com/api/v8";
pub const DISCORD_HEARTBEAT: &'static str = "{\"op\": 1}";
pub const DISCORD_HEARTBEAT_OP: u8 = 1;

#[derive(Deserialize)] struct IncomingHeartbeat { op: u8 }

#[derive(Serialize)]
struct OutgoingHeartbeat {
    op: u8,
    d: i64,
}

#[async_std::main]
async fn main() {
    let guilded_email = std::env::var("guilded_email").expect("No guilded_email env variable");
    let guilded_password = std::env::var("guilded_password").expect("No guilded_password env variable");
    let discord_auth_header = std::env::var("discord_auth").expect("No discord_auth env variable");

    let config = Config::load_blocking();

    let guilded_cookies = authenticate_guilded(&guilded_email, &guilded_password).await.expect("Failed to authenticate");
    let (to_guilded, from_guilded) = guilded_websocket(guilded_cookies.clone()).await.expect("Died while connecting to guilded");
    let to_guilded_heartbeat = to_guilded.clone();
    async_std::task::spawn(async move {
        while let Ok(_) = to_guilded_heartbeat.send(Message::Text("2".to_owned())).await {
            async_std::task::sleep(Duration::from_secs(24)).await;
        };
        eprintln!("Guilded heartbeat died");
        std::process::exit(1);
    });

    let mut discord_sequence_number: Arc<Mutex<Option<i64>>> = Arc::new(Mutex::new(None));
    let (to_discord, from_discord, discord_heartbeat_interval) = discord_websocket(discord_auth_header.clone(), discord_sequence_number.clone()).await.expect("Died while connecting to discord");

    let to_discord_heartbeat = to_discord.clone();
    let discord_heartbeat_interval = (discord_heartbeat_interval as f32 * 0.95).ceil() as u64;
    let mut discord_heartbeat_sequence_number = discord_sequence_number.clone();
    async_std::task::spawn(async move {
        async_std::task::sleep(Duration::from_millis(discord_heartbeat_interval)).await;
        while let Ok(_) = to_discord_heartbeat.send(make_discord_heartbeat(&mut discord_heartbeat_sequence_number).await).await {
            async_std::task::sleep(Duration::from_millis(discord_heartbeat_interval)).await;
        };
        eprintln!("Discord heartbeat died");
        std::process::exit(1);
    });

    let to_discord_heartbeat = to_discord.clone();
    let mut from_discord_heartbeat = from_discord.clone();
    let mut discord_heartbeat_sequence_number = discord_sequence_number.clone();
    async_std::task::spawn(async move {
        while let Some(msg) = from_discord_heartbeat.next().await {
            if let Message::Text(msg) = &*msg {
                if let Ok(heartbeat) = serde_json::from_str::<IncomingHeartbeat>(&msg) {
                    if heartbeat.op == DISCORD_HEARTBEAT_OP {
                        to_discord_heartbeat.send(make_discord_heartbeat(&mut discord_heartbeat_sequence_number).await).await;
                    }
                }
            }
        }
    });

    let env = Arc::new(Environment {
        guilded_email, guilded_password, discord_auth_header, config, guilded_cookies    
    });

    guilded_to_discord::guilded_to_discord(env.clone(), from_guilded.clone()).await;
    discord_to_guilded::discord_to_guilded(env.clone(), from_discord.clone()).await;

    futures::future::pending().await
}

async fn authenticate_guilded(guilded_email: &str, guilded_password: &str) -> Result<HeaderValues, ErrorBox> {
    #[derive(Serialize)]
    struct LoginBody { email: String, password: String, };
    let uri = GUILDED_API.to_owned() + "/login";
    let body = LoginBody { email: guilded_email.to_owned(), password: guilded_password.to_owned() };
    let res = surf::post(uri).body(surf::Body::from_json(&body)?).await?;
    if !res.status().is_success() { return Err(format!("authenticate_guilded {} {:?}", res.status(), res).into()) };
    Ok(res.header("Set-Cookie").map(|values| values.clone()).ok_or("authenticate_guilded no set-cookie".to_owned())?)
}

async fn guilded_websocket(guilded_cookies: HeaderValues) -> Result<(Sender<Message>, MultiRecv<Message>), ErrorBox> {
    let request = guilded_cookies.iter().fold(
        http::Request::builder()
            .uri(format!("wss://api.guilded.gg/socket.io/?jwt=undefined&EIO=3&transport=websocket")),
        |request, value| request.header("Cookie", value.as_str().to_owned())
    ).body(()).unwrap();
    let (ws, _response) = async_tungstenite::async_std::connect_async(request).await?;
    Ok(my_ws_task(ws))
}

async fn discord_websocket(discord_auth_header: String, discord_sequence_number: Arc<Mutex<Option<i64>>>) -> Result<(Sender<Message>, MultiRecv<Message>, u64), ErrorBox> {
    let gateway_get_endpoint = if discord_auth_header.len() > 4 && &discord_auth_header[0..4] == "Bot " { format!("{}/gateway/bot", DISCORD_API) } else { format!("{}/gateway", DISCORD_API) };
    #[derive(Serialize, Deserialize)]
    struct GatewayResponse { url: String }
    let mut get_response = surf::get(gateway_get_endpoint)
        .header("Authorization", &discord_auth_header)
        .send().await?;
    if !get_response.status().is_success() { return Err(format!("Failed to get a gateway endpoint: {}", get_response.status()).into()) };
    let get_response = get_response.body_json::<GatewayResponse>().await?;

    let request = http::Request::builder()
        .uri(get_response.url)
        .header("Authorization", discord_auth_header.clone())
        .body(())
        .unwrap();
    let (ws, _response) = async_tungstenite::async_std::connect_async(request).await?;
    let (to_discord, mut from_discord) = my_ws_task(ws);

    #[derive(Deserialize)]
    struct SequenceNumber { s: i64 }
    let mut my_from_discord = from_discord.clone();
    async_std::task::spawn(async move {
        while let Some(msg) = my_from_discord.next().await {
            if let Message::Text(msg) = &*msg {
                if let Ok(sequence_number) = serde_json::from_str::<SequenceNumber>(&msg) {
                    *discord_sequence_number.lock().await = Some(sequence_number.s);
                }
            }
        }
    });

    #[derive(Serialize, Deserialize)]
    struct GatewayHello {
        op: u8,
        d: HeartbeatInformation
    }
    #[derive(Serialize, Deserialize)]
    struct HeartbeatInformation {
        heartbeat_interval: u64,
    }
    if let Some(msg) = from_discord.next().await {
        if let Message::Text(msg) = &*msg {
            if let Ok(gateway_hello) = serde_json::from_str::<GatewayHello>(&msg) {
                if gateway_hello.op == 10 {
                    to_discord.send(Message::Text(format!("{{\"op\": 2, \"d\": {{ \"token\": \"{}\", \"intents\": 1536, \"properties\": {{ \"$os\": \"linux\", \"$browser\": \"bridge7573\", \"$device\": \"bridge7573\" }} }} }}", discord_auth_header))).await?;
                    if let Some(msg) = from_discord.next().await {
                        if let Message::Text(msg) = &*msg {
                            if let Ok(accepted) = serde_json::from_str::<IncomingHeartbeat>(&msg) {
                                if accepted.op == 0 {
                                    return Ok((to_discord, from_discord, gateway_hello.d.heartbeat_interval));
                                }
                            }
                        }
                    }
                }
            }
        }
    } 
    return Err("Didn't get Hello message from discord gateway".into())
}

async fn make_discord_heartbeat(sequence_number: &mut Arc<Mutex<Option<i64>>>) -> Message {
    if let Some(seq_num) = &*sequence_number.lock().await {
        Message::Text(format!("{{ \"op\": 1, \"d\": {} }}", seq_num))
    } else {
        Message::Text("{ \"op\": 1, \"d\": null }".to_owned())
    }
}

fn my_ws_task<S: futures::AsyncRead + futures::AsyncWrite + Unpin + Send + 'static>(ws: WebSocketStream<S>) -> (Sender<Message>, MultiRecv<Message>) {
    let (send_msgs, msgs_to_send) = unbounded::<Message>();
    let send_msgs_keep_alive = send_msgs.clone();
    let (msg_out, msgs_received) = MultiRecv::<Message>::new();
    async_std::task::spawn(async move {
        let mut msgs_to_send = msgs_to_send;
        let msg_out = msg_out;
        let mut ws = ws;
        loop {
            select_biased! {
                incoming_msg = ws.next().fuse() => {
                    match incoming_msg {
                        Some(Ok(msg)) => {
                            if let Err(err) = msg_out.send(msg).await {
                                eprintln!("Died while msg_out: {:?}", err);
                                std::process::exit(1);                            
                            }
                        },
                        Some(Err(err)) => {
                            eprintln!("Died while incoming_msg: {:?}", err);
                            std::process::exit(1);
                        },
                        None => {
                            eprintln!("Died while incoming_msg of uselessness");
                            std::process::exit(1);
                        }
                    }
                },
                send_msg = msgs_to_send.next().fuse() => {
                    if let Some(msg) = send_msg {
                        if let Err(err) = ws.send(msg).await {
                            eprintln!("Died while send_msg: {:?}", err);
                            std::process::exit(1);
                        }
                    } else {
                        eprintln!("Died while send_msg of uselessness");
                        std::process::exit(1);
                    }
                }
            }
        }
        drop(send_msgs_keep_alive);
    });
    (send_msgs, msgs_received)
}
