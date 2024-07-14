use core::convert::Into;

pub enum MediaStatus {
    Stopped,
    Paused,
    Playing,
}

impl MediaStatus {
    pub fn from_str(s: &str) -> MediaStatus {
        match s {
            "stopped" => MediaStatus::Stopped,
            "paused" => MediaStatus::Paused,
            "playing" => MediaStatus::Playing,
            _ => MediaStatus::Stopped,
        }
    }
    pub fn from_string(s: String) -> MediaStatus {
        MediaStatus::from_str(s.as_str())
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            MediaStatus::Stopped => "stopped",
            MediaStatus::Paused => "paused",
            MediaStatus::Playing => "playing",
        }
    }
    pub fn to_string(&self) -> String {
        self.as_str().to_string()
    }
}

impl Into<String> for MediaStatus {
    fn into(self) -> String {
        self.to_string()
    }
}