use core::convert::Into;

pub enum PlaybackState {
    Stopped,
    Paused,
    Playing,
}

impl PlaybackState {
    pub fn from_str(s: &str) -> Self {
        match s {
            "stopped" => Self::Stopped,
            "paused" => Self::Paused,
            "playing" => Self::Playing,
            _ => Self::Stopped,
        }
    }
    pub fn from_string(s: String) -> Self {
        Self::from_str(s.as_str())
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Paused => "paused",
            Self::Playing => "playing",
        }
    }
    pub fn to_string(&self) -> String {
        self.as_str().to_string()
    }
}

impl Into<String> for PlaybackState {
    fn into(self) -> String {
        self.to_string()
    }
}

impl From<String> for PlaybackState {
    fn from(s: String) -> Self {
        Self::from_string(s)
    }
}

impl From<&str> for PlaybackState {
    fn from(s: &str) -> Self {
        Self::from_str(s)
    }
}
