use thiserror::Error;

#[derive(Error, Debug)]
pub enum TaskForceAIError {
    #[error("API key is required when not in mock mode")]
    MissingApiKey,
    #[error("Prompt must be a non-empty string")]
    EmptyPrompt,
    #[error("Task ID must be a non-empty string")]
    EmptyTaskId,
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Task failed: {0}")]
    TaskFailed(String),
    #[error("Task did not complete within the expected time")]
    Timeout,
    #[error("API error (status {status}): {message}")]
    Api {
        status: reqwest::StatusCode,
        message: String,
    },
    #[error("Stream error: {0}")]
    Stream(String),
    #[error("Other error: {0}")]
    Other(String),
}
