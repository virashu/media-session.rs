use std::cmp::min;

use crate::{utils::micros_since_epoch, PlaybackState};

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct MediaInfo {
    pub title: String,
    pub artist: String,

    pub album_title: String,
    pub album_artist: String,

    /// Microseconds
    pub duration: i64,
    /// Microseconds since start
    pub position: i64,

    pub cover_b64: String,
    #[cfg_attr(feature = "serde", serde(skip_serializing))]
    pub cover_raw: Vec<u8>,

    pub state: String, // stopped, paused, playing
}

impl MediaInfo {
    fn apply_position(&mut self, pos_info: &PositionInfo) {
        let position = match PlaybackState::from(self.state.as_ref()) {
            PlaybackState::Stopped => 0,
            PlaybackState::Paused => pos_info.pos_raw,
            PlaybackState::Playing => {
                let update_delta = micros_since_epoch() - pos_info.pos_last_update;

                #[allow(clippy::cast_precision_loss, reason = "needed for multiplication")]
                let track_delta = update_delta as f64 * pos_info.playback_rate;

                #[allow(clippy::cast_possible_truncation, reason = "rounded")]
                min(self.duration, pos_info.pos_raw + track_delta.round() as i64)
            }
        };

        self.position = position;
    }

    /// Return a [`MediaInfo`] with updated position
    #[must_use]
    pub fn with_position(&self, pos_info: &PositionInfo) -> Self {
        let mut info = self.clone();
        info.apply_position(pos_info);
        info
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

impl std::fmt::Debug for MediaInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        struct Field<'a> {
            inner: &'a str,
        }
        impl std::fmt::Debug for Field<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.inner)
            }
        }

        #[allow(dead_code)]
        #[derive(Debug)]
        struct MediaInfo<'a> {
            title: &'a str,
            artist: &'a str,
            album_title: &'a str,
            album_artist: &'a str,
            duration: &'a i64,
            position: &'a i64,
            state: &'a str,

            cover_b64: Field<'a>,
            cover_raw: Field<'a>,
        }

        let Self {
            title,
            artist,
            album_title,
            album_artist,
            duration,
            position,
            state,

            cover_raw: cr,
            cover_b64: c64,
        } = self;

        std::fmt::Debug::fmt(
            &MediaInfo {
                title,
                artist,
                album_title,
                album_artist,
                duration,
                position,
                state,

                cover_raw: Field {
                    inner: if cr.is_empty() { "<none>" } else { "<...>" },
                },
                cover_b64: Field {
                    inner: if c64.is_empty() { "<none>" } else { "<...>" },
                },
                // cover_b64: Field { inner: c64 }, // raw display
            },
            f,
        )
    }
}

#[derive(Clone, Debug)]
pub struct PositionInfo {
    pub playback_rate: f64,
    pub pos_last_update: i64,
    pub pos_raw: i64,
}

impl Default for PositionInfo {
    fn default() -> Self {
        Self {
            playback_rate: 1.0,
            pos_last_update: 0,
            pos_raw: 0,
        }
    }
}
