pub trait ErrorBoxable: std::fmt::Debug + std::fmt::Display {}
impl ErrorBoxable for surf::Error {}
impl ErrorBoxable for String {}
impl ErrorBoxable for async_tungstenite::tungstenite::Error {}
impl ErrorBoxable for &str {}
impl<T> ErrorBoxable for async_std::channel::SendError<T> {}
pub struct ErrorBox(Box<dyn ErrorBoxable>);
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
