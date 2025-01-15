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

#[cfg(windows)]
impl From<windows::core::Error> for Error {
    fn from(e: windows::core::Error) -> Self {
        Self {
            message: e.message().to_string(),
        }
    }
}

#[cfg(unix)]
impl From<dbus::Error> for Error {
    fn from(value: dbus::Error) -> Self {
        Self {
            message: value.message().unwrap_or("Unknown error").to_string(),
        }
    }
}
