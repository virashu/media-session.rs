use crate::error::Error;
use std::str::FromStr;

#[derive(Default)]
pub enum PlaybackState {
    #[default]
    Stopped,
    Paused,
    Playing,
}

impl PlaybackState {
    pub fn from_string(s: String) -> Result<Self, Error> {
        Self::from_str(s.as_str())
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Paused => "paused",
            Self::Playing => "playing",
        }
    }
}

impl std::fmt::Display for PlaybackState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for PlaybackState {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "stopped" => Ok(Self::Stopped),
            "paused" => Ok(Self::Paused),
            "playing" => Ok(Self::Playing),
            "" => Err(Error::new("cannot parse playback state from empty string")),
            _ => Err(Error::new("cannot parse playback state")),
        }
    }
}

impl From<PlaybackState> for String {
    fn from(state: PlaybackState) -> Self {
        state.to_string()
    }
}

impl From<String> for PlaybackState {
    fn from(s: String) -> Self {
        Self::from_string(s).unwrap_or_default()
    }
}

impl From<&str> for PlaybackState {
    fn from(s: &str) -> Self {
        Self::from_str(s).unwrap_or_default()
    }
}
