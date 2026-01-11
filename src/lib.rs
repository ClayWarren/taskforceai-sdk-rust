pub mod client;
pub mod error;
pub mod stream;
pub mod types;

pub use client::TaskForceAI;
pub use error::TaskForceAIError;
pub use types::{TaskForceAIOptions, TaskStatus, TaskSubmissionOptions};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{DEFAULT_BASE_URL, DEFAULT_TIMEOUT_SECS};
    use futures_util::StreamExt;
    use mockito::Server;
    use std::time::Duration;

    #[tokio::test]
    async fn test_new_client_defaults() {
        let client = TaskForceAI::new(TaskForceAIOptions {
            api_key: Some("test-key".to_string()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(client.base_url, DEFAULT_BASE_URL);
        assert_eq!(client.timeout, Duration::from_secs(DEFAULT_TIMEOUT_SECS));
    }

    #[tokio::test]
    async fn test_new_client_error() {
        let res = TaskForceAI::new(TaskForceAIOptions {
            api_key: None,
            mock_mode: Some(false),
            ..Default::default()
        });
        assert!(matches!(res, Err(TaskForceAIError::MissingApiKey)));
    }

    #[tokio::test]
    async fn test_mock_mode() {
        let opts = TaskForceAIOptions {
            mock_mode: Some(true),
            ..Default::default()
        };
        let client = TaskForceAI::new(opts).unwrap();

        // Test run_task
        let status = client.run_task("hello", None, None, None).await.unwrap();
        assert_eq!(status.task_id, "mock-task-123");

        // Test stream_task_status
        let mut stream = client.stream_task_status("mock-id").await.unwrap();
        let ev = stream.next().await.unwrap().unwrap();
        assert_eq!(ev.status, "completed");
    }

    #[tokio::test]
    async fn test_submit_task_errors() {
        let client = TaskForceAI::new(TaskForceAIOptions {
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let res = client.submit_task("  ", None).await;
        assert!(matches!(res, Err(TaskForceAIError::EmptyPrompt)));
    }

    #[tokio::test]
    async fn test_api_error() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/run")
            .with_status(401)
            .with_body("Unauthorized")
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("wrong".to_string()),
            ..Default::default()
        })
        .unwrap();

        let res = client.submit_task("hi", None).await;
        match res {
            Err(TaskForceAIError::Api { status, .. }) => assert_eq!(status, 401),
            _ => panic!("Expected API error"),
        }
    }

    #[tokio::test]
    async fn test_wait_for_completion_timeout() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/status/task-1")
            .with_status(200)
            .with_body(r#"{"taskId": "task-1", "status": "processing"}"#)
            .expect(2)
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let res = client
            .wait_for_completion("task-1", Some(Duration::from_millis(1)), Some(2))
            .await;
        assert!(matches!(res, Err(TaskForceAIError::Timeout)));
    }

    #[tokio::test]
    async fn test_wait_for_completion_failed() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/status/task-1")
            .with_status(200)
            .with_body(r#"{"taskId": "task-1", "status": "failed", "error": "oops"}"#)
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let res = client.wait_for_completion("task-1", None, None).await;
        match res {
            Err(TaskForceAIError::TaskFailed(msg)) => assert_eq!(msg, "oops"),
            _ => panic!("Expected TaskFailed error"),
        }
    }

    #[tokio::test]
    async fn test_stream_task_status() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("GET", "/stream/task-1")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body("data: {\"taskId\": \"task-1\", \"status\": \"processing\"}\ndata: {\"taskId\": \"task-1\", \"status\": \"completed\", \"result\": \"stream-done\"}\n")
            .create_async().await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let mut stream = client.stream_task_status("task-1").await.unwrap();

        let ev1 = stream.next().await.unwrap().unwrap();
        assert_eq!(ev1.status, "processing");

        let ev2 = stream.next().await.unwrap().unwrap();
        assert_eq!(ev2.status, "completed");
        assert_eq!(ev2.result.unwrap(), "stream-done");

        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn test_run_task_stream() {
        let mut server = Server::new_async().await;
        let _run_mock = server
            .mock("POST", "/run")
            .with_status(200)
            .with_body(r#"{"taskId": "task-2"}"#)
            .create_async()
            .await;
        let _stream_mock = server
            .mock("GET", "/stream/task-2")
            .with_status(200)
            .with_body("data: {\"taskId\": \"task-2\", \"status\": \"completed\"}\n")
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let mut stream = client.run_task_stream("hi", None).await.unwrap();
        let ev = stream.next().await.unwrap().unwrap();
        assert_eq!(ev.status, "completed");
    }

    #[tokio::test]
    async fn test_stream_error_status() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/stream/task-1")
            .with_status(403)
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let res = client.stream_task_status("task-1").await;
        assert!(matches!(res, Err(TaskForceAIError::Api { .. })));
    }

    #[tokio::test]
    async fn test_stream_malformed_json() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/stream/task-1")
            .with_status(200)
            .with_body("data: {malformed}\n")
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let mut stream = client.stream_task_status("task-1").await.unwrap();
        let res = stream.next().await.unwrap();
        assert!(matches!(res, Err(TaskForceAIError::Serialization(_))));
    }

    #[tokio::test]
    async fn test_get_task_status_errors() {
        let client = TaskForceAI::new(TaskForceAIOptions {
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let res = client.get_task_status("  ").await;
        assert!(matches!(res, Err(TaskForceAIError::EmptyTaskId)));
    }

    #[tokio::test]
    async fn test_stream_task_status_empty_id() {
        let client = TaskForceAI::new(TaskForceAIOptions {
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let res = client.stream_task_status("").await;
        assert!(matches!(res, Err(TaskForceAIError::EmptyTaskId)));
    }

    #[tokio::test]
    async fn test_stream_bytes_error() {
        // This is hard to trigger with mockito without a real network failure
        // but we can test the partial buffer at the end of the stream
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/stream/task-1")
            .with_status(200)
            // No newline at the end
            .with_body("data: {\"taskId\": \"task-1\", \"status\": \"completed\"}")
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let mut stream = client.stream_task_status("task-1").await.unwrap();
        let ev = stream.next().await.unwrap().unwrap();
        assert_eq!(ev.status, "completed");
    }

    #[tokio::test]
    async fn test_submit_task_with_options() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/run")
            .with_status(200)
            .with_body(r#"{"taskId": "task-opts"}"#)
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let opts = TaskSubmissionOptions {
            model_id: Some("gpt-4".to_string()),
            silent: Some(true),
            ..Default::default()
        };
        let task_id = client.submit_task("hello", Some(opts)).await.unwrap();
        assert_eq!(task_id, "task-opts");
    }

    #[tokio::test]
    async fn test_stream_empty_end() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/stream/task-1")
            .with_status(200)
            .with_body("data: {\"taskId\": \"task-1\", \"status\": \"processing\"}\n\n")
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let mut stream = client.stream_task_status("task-1").await.unwrap();
        let ev = stream.next().await.unwrap().unwrap();
        assert_eq!(ev.status, "processing");
        assert!(stream.next().await.is_none());
    }
}
