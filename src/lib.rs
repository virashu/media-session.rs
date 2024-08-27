mod error;
mod media_info;
mod playback_state;
pub mod traits;
mod utils;

pub(crate) mod imp;
mod media_session;

pub use error::Error;
pub use media_info::{MediaInfo, PositionInfo};
pub use media_session::MediaSession;
pub use playback_state::PlaybackState;

type Result<T> = core::result::Result<T, Error>;
