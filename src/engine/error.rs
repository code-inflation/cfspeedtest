use thiserror::Error;

#[derive(Error, Debug)]
pub enum SpeedTestError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Failed to parse metadata: {0}")]
    MetadataParse(String),

    #[error("Failed to send event")]
    ChannelClosed,

    #[error("{0}")]
    Other(String),
}
