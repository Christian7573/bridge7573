pub trait ErrorBoxable: std::fmt::Debug + std::fmt::Display {}
impl ErrorBoxable for surf::Error {}
impl ErrorBoxable for String {}
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
