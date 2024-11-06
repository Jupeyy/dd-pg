use std::fmt::Debug;

use anyhow::Error;
use hiarc::Hiarc;
use thiserror::Error;

pub type OffairSoundManagerId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FetchSoundManagerIndex {
    Onair,
    Offair(OffairSoundManagerId),
}

#[derive(Debug, Hiarc, Error)]
pub enum FetchSoundManagerError {
    #[error(
        "sound manager with the id, which was obtained by `current_fetch_index`, was not found."
    )]
    SoundManagerNotFound,
    #[error("the backend had an error: {0}")]
    DriverErr(String),
}

impl From<Error> for FetchSoundManagerError {
    fn from(value: Error) -> Self {
        FetchSoundManagerError::DriverErr(value.to_string())
    }
}

#[derive(Debug, Hiarc)]
pub struct BackendAudioFrame {
    pub left: f32,
    pub right: f32,
}

/// Frame fetchers that read onair sound must be fast.
/// Blocking operations could lead to static noise.
pub trait BackendFrameFetcher: Debug + Sync + Send + 'static {
    fn next_frame(&self, frame_data: BackendAudioFrame);

    /// generally a frame fetcher should only fetch the content of a specific
    /// sound manager. If for whatever reason it changes it can however,
    /// the backend must respect it for every frame.
    fn current_fetch_index(&self) -> FetchSoundManagerIndex;

    /// informs that fetching failed for some reason
    fn fetch_err(&self, err: FetchSoundManagerError);
}
