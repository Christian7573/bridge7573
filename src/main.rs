#[macro_use] extern crate futures;
use async_std::sync::Mutex;
use serde::{Serialize, Deserialize};
use http_types::headers::HeaderValues;
use serde_json::Value as JsValue;
use async_tungstenite::tungstenite::Message;
use async_tungstenite::WebSocketStream;
use async_std::channel::{Sender, unbounded};
use futures::{StreamExt, SinkExt, FutureExt};

mod multi_recv;
mod error_boxable;
use multi_recv::*;
use error_boxable::*;

pub struct Global {
    guilded_cookies: HeaderValues,
    from_guilded: MultiRecv<JsValue>,
    //from_discord: MultiRecv<JsValue>,
}
const GUILDED_API: &'static str = "https://www.guilded.gg/api";

static mut GLOBAL_RAW: Option<Global> = None;
fn global() -> &'static mut Global { unsafe { GLOBAL_RAW.as_mut().unwrap() } }

#[async_std::main]
async fn main() {
    let guilded_email = std::env::var("guilded_email").expect("No guilded_email env variable");
    let guilded_password = std::env::var("guilded_password").expect("No guilded_password env variable");
    let discord_auth_header = std::env::var("discord_auth").expect("No discord_auth env variable");

    let guilded_cookies = authenticate_guilded(&guilded_email, &guilded_password).await.expect("Failed to authenticate");
    let (_to_guilded, mut from_guilded) = guilded_websocket(guilded_cookies.clone()).await.expect("Died while connecting to guilded");
    println!("{:?}", from_guilded.next().await);
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
        |request, value| request.header("Set-Cookie", value.as_str().to_owned())
    ).body(()).unwrap();
    let (ws, _response) = async_tungstenite::async_std::connect_async(request).await?;
    Ok(my_ws_task(ws))
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
                        if let Err(err) = ws.feed(msg).await {
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
