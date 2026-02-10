use crate::client::TaskForceAI;
use crate::error::TaskForceAIError;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};

/// Represents an uploaded file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub id: String,
    pub filename: String,
    pub purpose: String,
    pub bytes: i64,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Options for uploading a file.
#[derive(Debug, Clone, Default)]
pub struct FileUploadOptions {
    pub purpose: Option<String>,
    pub mime_type: Option<String>,
}

/// Response containing a list of files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileListResponse {
    pub files: Vec<File>,
    pub total: i64,
}

impl TaskForceAI {
    /// Uploads a file to the API.
    pub async fn upload_file(
        &self,
        filename: &str,
        content: Bytes,
        options: Option<FileUploadOptions>,
    ) -> Result<File, TaskForceAIError> {
        let mime_type = options
            .as_ref()
            .and_then(|o| o.mime_type.clone())
            .unwrap_or_else(|| "application/octet-stream".to_string());

        let mut form = Form::new().part(
            "file",
            Part::bytes(content.to_vec())
                .file_name(filename.to_string())
                .mime_str(&mime_type)
                .map_err(|e| TaskForceAIError::Other(e.to_string()))?,
        );

        if let Some(opts) = options {
            if let Some(purpose) = opts.purpose {
                form = form.text("purpose", purpose);
            }
            if let Some(mime_type) = opts.mime_type {
                form = form.text("mime_type", mime_type);
            }
        }

        let url = format!("{}/files", self.base_url);
        let mut request = self.client.post(&url).multipart(form);

        if !self.api_key.is_empty() {
            request = request.header("x-api-key", &self.api_key);
        }
        request = request.header("X-SDK-Language", "rust");

        let response = request.send().await?;
        let status = response.status();

        if !status.is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error message".to_string());
            return Err(TaskForceAIError::Api { status, message });
        }

        Ok(response.json().await?)
    }

    /// Retrieves a list of uploaded files.
    pub async fn list_files(
        &self,
        limit: i32,
        offset: i32,
    ) -> Result<FileListResponse, TaskForceAIError> {
        let path = format!("/files?limit={}&offset={}", limit, offset);
        self.request(reqwest::Method::GET, &path, None).await
    }

    /// Retrieves metadata for a specific file.
    pub async fn get_file(&self, file_id: &str) -> Result<File, TaskForceAIError> {
        let path = format!("/files/{}", file_id);
        self.request(reqwest::Method::GET, &path, None).await
    }

    /// Deletes a file by ID.
    pub async fn delete_file(&self, file_id: &str) -> Result<(), TaskForceAIError> {
        let path = format!("/files/{}", file_id);
        let _: serde_json::Value = self.request(reqwest::Method::DELETE, &path, None).await?;
        Ok(())
    }

    /// Downloads the content of a file.
    pub async fn download_file(&self, file_id: &str) -> Result<Bytes, TaskForceAIError> {
        let url = format!("{}/files/{}/content", self.base_url, file_id);
        let mut request = self.client.get(&url);

        if !self.api_key.is_empty() {
            request = request.header("x-api-key", &self.api_key);
        }
        request = request.header("X-SDK-Language", "rust");

        let response = request.send().await?;
        let status = response.status();

        if !status.is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error message".to_string());
            return Err(TaskForceAIError::Api { status, message });
        }

        Ok(response.bytes().await?)
    }
}
