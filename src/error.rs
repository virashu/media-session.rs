#[derive(Debug)]
pub struct Error {
    message: String,
}

impl Error {
    pub fn new<T: Into<String>>(message: T) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        core::write!(f, "{}", self.message)
    }
}

impl std::error::Error for Error {}
