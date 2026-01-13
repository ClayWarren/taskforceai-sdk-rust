use crate::client::TaskForceAI;
use crate::error::TaskForceAIError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a conversation thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub id: i64,
    pub title: String,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub updated_at: DateTime<Utc>,
}

/// Represents a message within a thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadMessage {
    pub id: i64,
    pub thread_id: i64,
    pub role: String, // "user" or "assistant"
    pub content: String,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,
}

/// Options for creating a thread.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateThreadOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<ThreadMessage>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Response containing a list of threads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadListResponse {
    pub threads: Vec<Thread>,
    pub total: i64,
}

/// Response containing messages from a thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadMessagesResponse {
    pub messages: Vec<ThreadMessage>,
    pub total: i64,
}

/// Options for running a prompt in a thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadRunOptions {
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<HashMap<String, serde_json::Value>>,
}

/// Response from running in a thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadRunResponse {
    pub task_id: String,
    pub thread_id: i64,
    pub message_id: i64,
}

impl TaskForceAI {
    /// Creates a new conversation thread.
    pub async fn create_thread(
        &self,
        options: Option<CreateThreadOptions>,
    ) -> Result<Thread, TaskForceAIError> {
        let body = options
            .map(|o| serde_json::to_value(o))
            .transpose()?
            .unwrap_or_else(|| serde_json::json!({}));

        self.request(reqwest::Method::POST, "/threads", Some(body))
            .await
    }

    /// Retrieves a list of threads.
    pub async fn list_threads(
        &self,
        limit: i32,
        offset: i32,
    ) -> Result<ThreadListResponse, TaskForceAIError> {
        let path = format!("/threads?limit={}&offset={}", limit, offset);
        self.request(reqwest::Method::GET, &path, None).await
    }

    /// Retrieves a specific thread by ID.
    pub async fn get_thread(&self, thread_id: i64) -> Result<Thread, TaskForceAIError> {
        let path = format!("/threads/{}", thread_id);
        self.request(reqwest::Method::GET, &path, None).await
    }

    /// Deletes a thread by ID.
    pub async fn delete_thread(&self, thread_id: i64) -> Result<(), TaskForceAIError> {
        let path = format!("/threads/{}", thread_id);
        let _: serde_json::Value = self.request(reqwest::Method::DELETE, &path, None).await?;
        Ok(())
    }

    /// Retrieves messages from a thread.
    pub async fn get_thread_messages(
        &self,
        thread_id: i64,
        limit: i32,
        offset: i32,
    ) -> Result<ThreadMessagesResponse, TaskForceAIError> {
        let path = format!(
            "/threads/{}/messages?limit={}&offset={}",
            thread_id, limit, offset
        );
        self.request(reqwest::Method::GET, &path, None).await
    }

    /// Submits a prompt within a thread context.
    pub async fn run_in_thread(
        &self,
        thread_id: i64,
        options: ThreadRunOptions,
    ) -> Result<ThreadRunResponse, TaskForceAIError> {
        if options.prompt.trim().is_empty() {
            return Err(TaskForceAIError::EmptyPrompt);
        }

        let path = format!("/threads/{}/runs", thread_id);
        let body = serde_json::to_value(options)?;

        self.request(reqwest::Method::POST, &path, Some(body)).await
    }
}
