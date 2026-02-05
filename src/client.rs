use crate::error::TaskForceAIError;
use crate::types::{
    SubmitTaskResponse, TaskForceAIOptions, TaskStatus, TaskStatusValue, TaskSubmissionOptions,
};
use std::time::Duration;
use tokio::time::sleep;

pub const DEFAULT_BASE_URL: &str = "https://taskforceai.chat/api/developer";
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;
pub const DEFAULT_POLL_INTERVAL_MS: u64 = 1000;
pub const DEFAULT_MAX_POLL_ATTEMPTS: u32 = 60;

pub struct TaskForceAI {
    pub(crate) api_key: String,
    pub(crate) base_url: String,
    #[allow(dead_code)]
    pub(crate) timeout: Duration,
    pub(crate) mock_mode: bool,
    pub(crate) client: reqwest::Client,
}

impl TaskForceAI {
    pub fn new(options: TaskForceAIOptions) -> Result<Self, TaskForceAIError> {
        let mock_mode = options.mock_mode.unwrap_or(false);
        let api_key = options.api_key.unwrap_or_default();

        if !mock_mode && api_key.is_empty() {
            return Err(TaskForceAIError::MissingApiKey);
        }

        let base_url = options
            .base_url
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
            .trim_end_matches('/')
            .to_string();

        let timeout = Duration::from_secs(options.timeout.unwrap_or(DEFAULT_TIMEOUT_SECS));

        let client = reqwest::Client::builder().timeout(timeout).build()?;

        Ok(Self {
            api_key,
            base_url,
            timeout,
            mock_mode,
            client,
        })
    }

    pub(crate) async fn request<T>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T, TaskForceAIError>
    where
        T: serde::de::DeserializeOwned,
    {
        if self.mock_mode {
            return self.mock_response(path, &method);
        }

        let url = format!("{}{}", self.base_url, path);
        let mut request = self.client.request(method, &url);

        if !self.api_key.is_empty() {
            request = request.header("x-api-key", &self.api_key);
        }

        request = request.header("X-SDK-Language", "rust");

        if let Some(b) = body {
            request = request.json(&b);
        }

        let response = request.send().await?;
        let status = response.status();

        if !status.is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error message from response body".to_string());
            return Err(TaskForceAIError::Api { status, message });
        }

        Ok(response.json().await?)
    }

    fn mock_response<T>(&self, path: &str, method: &reqwest::Method) -> Result<T, TaskForceAIError>
    where
        T: serde::de::DeserializeOwned,
    {
        let val = if method == reqwest::Method::POST && path == "/run" {
            serde_json::json!({ "taskId": "mock-task-123" })
        } else if path.starts_with("/status/") {
            serde_json::json!({
                "taskId": "mock-task-123",
                "status": "completed",
                "result": "This is a mock response. Configure your API key to get real results."
            })
        } else {
            serde_json::json!({ "status": "ok" })
        };

        Ok(serde_json::from_value(val)?)
    }

    pub async fn submit_task(
        &self,
        prompt: &str,
        options: Option<TaskSubmissionOptions>,
    ) -> Result<String, TaskForceAIError> {
        if prompt.trim().is_empty() {
            return Err(TaskForceAIError::EmptyPrompt);
        }

        let mut body = serde_json::json!({ "prompt": prompt });
        if let Some(opts) = options {
            let images = opts.images.clone();
            if let Some(obj) = body.as_object_mut() {
                obj.insert("options".to_string(), serde_json::to_value(opts)?);
                if let Some(imgs) = images {
                    if !imgs.is_empty() {
                        obj.insert("attachments".to_string(), serde_json::to_value(imgs)?);
                    }
                }
            }
        }

        let response: SubmitTaskResponse = self
            .request(reqwest::Method::POST, "/run", Some(body))
            .await?;
        Ok(response.task_id)
    }

    pub async fn get_task_status(&self, task_id: &str) -> Result<TaskStatus, TaskForceAIError> {
        if task_id.trim().is_empty() {
            return Err(TaskForceAIError::EmptyTaskId);
        }
        self.request(reqwest::Method::GET, &format!("/status/{}", task_id), None)
            .await
    }

    pub async fn wait_for_completion(
        &self,
        task_id: &str,
        poll_interval: Option<Duration>,
        max_attempts: Option<u32>,
    ) -> Result<TaskStatus, TaskForceAIError> {
        let interval = poll_interval.unwrap_or(Duration::from_millis(DEFAULT_POLL_INTERVAL_MS));
        let max = max_attempts.unwrap_or(DEFAULT_MAX_POLL_ATTEMPTS);

        for _ in 0..max {
            let status = self.get_task_status(task_id).await?;
            match status.status {
                TaskStatusValue::Completed => return Ok(status),
                TaskStatusValue::Failed => {
                    return Err(TaskForceAIError::TaskFailed(
                        status.error.unwrap_or_else(|| "Unknown error".to_string()),
                    ))
                }
                TaskStatusValue::Processing => (),
            }
            sleep(interval).await;
        }

        Err(TaskForceAIError::Timeout)
    }

    pub async fn run_task(
        &self,
        prompt: &str,
        options: Option<TaskSubmissionOptions>,
        poll_interval: Option<Duration>,
        max_attempts: Option<u32>,
    ) -> Result<TaskStatus, TaskForceAIError> {
        let task_id = self.submit_task(prompt, options).await?;
        self.wait_for_completion(&task_id, poll_interval, max_attempts)
            .await
    }
}
