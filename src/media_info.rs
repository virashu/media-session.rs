use crate::PlaybackState;

#[derive(Clone, Debug)]
pub struct MediaInfo {
    pub title: String,
    pub artist: String,

    pub album_title: String,
    pub album_artist: String,

    pub duration: i64,
    pub position: i64,

    pub cover_b64: String,
    pub cover_raw: Vec<u8>,

    pub state: String, // stopped, paused, playing
}

impl MediaInfo {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            artist: String::new(),

            album_title: String::new(),
            album_artist: String::new(),

            duration: 0,
            position: 0,

            cover_b64: String::new(),
            cover_raw: Vec::new(),

            state: PlaybackState::Stopped.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PositionInfo {
    pub playback_rate: f64,
    pub pos_last_update: i64,
    pub pos_raw: i64,
}

impl PositionInfo {
    pub fn new() -> Self {
        Self {
            playback_rate: 1.0,
            pos_last_update: 0,
            pos_raw: 0,
        }
    }
}

impl Default for PositionInfo {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "json")]
impl From<MediaInfo> for json::JsonValue {
    fn from(info: MediaInfo) -> Self {
        json::object! {
            title: info.title,
            artist: info.artist,
            album_title: info.album_title,
            album_artist: info.album_artist,
            duration: info.duration,
            position: info.position,
            cover_b64: info.cover_b64,
            state: info.state,
        }
    }
}

impl Default for MediaInfo {
    fn default() -> Self {
        Self {
            title: String::new(),
            artist: String::new(),

            album_title: String::new(),
            album_artist: String::new(),

            duration: 0,
            position: 0,

            cover_b64: String::new(),
            cover_raw: Vec::new(),

            state: PlaybackState::Stopped.into(),
        }
    }
}
