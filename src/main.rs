use async_std::sync::Mutex;
use serde::{Serialize, Deserialize};
use http_types::headers::HeaderValues;

pub struct Global {
    guilded_cookies: HeaderValues,

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
    println!("{}", guilded_cookies);
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

trait ErrorBoxable: std::fmt::Debug + std::fmt::Display {}
impl ErrorBoxable for surf::Error {}
impl ErrorBoxable for String {}
struct ErrorBox(Box<dyn ErrorBoxable>);
impl std::fmt::Display for ErrorBox {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> { self.0.fmt(f) }
}
impl std::fmt::Debug for ErrorBox {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> { self.0.fmt(f) }
}
impl<T: ErrorBoxable + 'static> From<T> for ErrorBox {
    fn from(other: T) -> ErrorBox { ErrorBox(Box::new(other)) }
}
fn errorbox<T: ErrorBoxable + 'static >(err: T) -> ErrorBox { ErrorBox::from(err) }
