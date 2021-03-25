use async_std::sync::Mutex;
use serde::{Serialize, Deserialize};

pub struct Global {
    
}
const GUILDED_API: &'static str = "https://www.guilded.gg/api";

static mut GLOBAL_RAW: Option<Mutex<Global>> = None;
fn global() -> &'static mut Mutex<Global> { unsafe { GLOBAL_RAW.as_mut().unwrap() } }

#[async_std::main]
async fn main() {
    let guidled_email = std::env::var("guilded_email").expect("No guilded_email env variable");
    let guilded_password = std::env::var("guilded_password").expect("No guilded_password env variable");
    let discord_auth_header = std::env::var("discord_auth").expect("No discord_auth env variable");


}

async fn authenticate_guilded(guilded_email: &str, guilded_password: &str) -> Result<String, ErrorBox> {
    #[derive(Serialize)]
    struct LoginBody {

    }
}

trait ErrorBoxable: std::fmt::Debug + std::fmt::Display {}
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
