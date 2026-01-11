use crate::client::TaskForceAI;
use crate::error::TaskForceAIError;
use crate::types::{TaskStatus, TaskSubmissionOptions};
use futures_util::{Stream, StreamExt};
use std::pin::Pin;

pub type TaskStatusStream =
    Pin<Box<dyn Stream<Item = Result<TaskStatus, TaskForceAIError>> + Send>>;

impl TaskForceAI {
    pub async fn stream_task_status(
        &self,
        task_id: &str,
    ) -> Result<TaskStatusStream, TaskForceAIError> {
        if task_id.trim().is_empty() {
            return Err(TaskForceAIError::EmptyTaskId);
        }

        if self.mock_mode {
            let status = self.get_task_status(task_id).await?;
            let stream = futures_util::stream::iter(vec![Ok(status)]);
            return Ok(Box::pin(stream));
        }

        let url = format!("{}/stream/{}", self.base_url, task_id);
        let mut request = self.client.get(&url);

        if !self.api_key.is_empty() {
            request = request.bearer_auth(&self.api_key);
        }

        request = request.header("Accept", "text/event-stream");

        let response = request.send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let message = response.text().await.unwrap_or_default();
            return Err(TaskForceAIError::Api { status, message });
        }

        let mut bytes_stream = response.bytes_stream();
        let mut buffer = String::new();

        let s = futures_util::stream::poll_fn(move |cx| {
            loop {
                if let Some(line_end) = buffer.find('\n') {
                    let line = buffer.drain(..line_end + 1).collect::<String>();
                    let line = line.trim();

                    if let Some(data) = line.strip_prefix("data:") {
                        let data = data.trim();
                        match serde_json::from_str::<TaskStatus>(data) {
                            Ok(status) => return std::task::Poll::Ready(Some(Ok(status))),
                            Err(e) => {
                                return std::task::Poll::Ready(Some(Err(
                                    TaskForceAIError::Serialization(e),
                                )))
                            }
                        }
                    }
                    continue;
                }

                match bytes_stream.poll_next_unpin(cx) {
                    std::task::Poll::Ready(Some(Ok(bytes))) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));
                        continue;
                    }
                    std::task::Poll::Ready(Some(Err(e))) => {
                        return std::task::Poll::Ready(Some(Err(TaskForceAIError::Network(e))))
                    }
                    std::task::Poll::Ready(None) => {
                        if buffer.is_empty() {
                            return std::task::Poll::Ready(None);
                        } else {
                            // Handle potential last line without newline
                            let line = std::mem::take(&mut buffer);
                            let line = line.trim();
                            if let Some(data) = line.strip_prefix("data:") {
                                let data = data.trim();
                                match serde_json::from_str::<TaskStatus>(data) {
                                    Ok(status) => return std::task::Poll::Ready(Some(Ok(status))),
                                    Err(e) => {
                                        return std::task::Poll::Ready(Some(Err(
                                            TaskForceAIError::Serialization(e),
                                        )))
                                    }
                                }
                            }
                            return std::task::Poll::Ready(None);
                        }
                    }
                    std::task::Poll::Pending => return std::task::Poll::Pending,
                }
            }
        });

        Ok(Box::pin(s))
    }

    pub async fn run_task_stream(
        &self,
        prompt: &str,
        options: Option<TaskSubmissionOptions>,
    ) -> Result<TaskStatusStream, TaskForceAIError> {
        let task_id = self.submit_task(prompt, options).await?;
        self.stream_task_status(&task_id).await
    }
}
