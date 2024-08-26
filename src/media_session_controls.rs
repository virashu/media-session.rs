use crate::error::Error;

pub trait MediaSessionControls {
    async fn toggle_pause(&self) -> Result<(), Error>;
    async fn pause(&self) -> Result<(), Error>;
    async fn play(&self) -> Result<(), Error>;
    async fn stop(&self) -> Result<(), Error>;
    async fn next(&self) -> Result<(), Error>;
    async fn prev(&self) -> Result<(), Error>;
}
