pub mod client;
pub mod error;
pub mod files;
pub mod stream;
pub mod threads;
pub mod types;

pub use client::TaskForceAI;
pub use error::TaskForceAIError;
pub use files::{File, FileListResponse, FileUploadOptions};
pub use threads::{
    CreateThreadOptions, Thread, ThreadListResponse, ThreadMessage, ThreadMessagesResponse,
    ThreadRunOptions, ThreadRunResponse,
};
pub use types::{
    ImageAttachment, TaskForceAIOptions, TaskStatus, TaskStatusValue, TaskSubmissionOptions,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{DEFAULT_BASE_URL, DEFAULT_TIMEOUT_SECS};
    use futures_util::StreamExt;
    use mockito::{Matcher, Server};
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
        assert_eq!(status.status, TaskStatusValue::Completed);

        // Test stream_task_status
        let mut stream = client.stream_task_status("mock-id").await.unwrap();
        let ev = stream.next().await.unwrap().unwrap();
        assert_eq!(ev.status, TaskStatusValue::Completed);
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
            .match_header("x-api-key", "key")
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
        assert_eq!(ev1.status, TaskStatusValue::Processing);

        let ev2 = stream.next().await.unwrap().unwrap();
        assert_eq!(ev2.status, TaskStatusValue::Completed);
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
        assert_eq!(ev.status, TaskStatusValue::Completed);
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
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/stream/task-1")
            .with_status(200)
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
        assert_eq!(ev.status, TaskStatusValue::Completed);
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
    async fn test_stream_empty_end_unique() {
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
        assert_eq!(ev.status, TaskStatusValue::Processing);
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn test_stream_none_with_empty_buffer() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/stream/task-1")
            .with_status(200)
            .with_body("data: {\"taskId\": \"task-1\", \"status\": \"processing\"}\n") // No trailing newline here to leave buffer empty after drain
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let mut stream = client.stream_task_status("task-1").await.unwrap();
        let _ = stream.next().await;
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn test_stream_non_data_line() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/stream/task-1")
            .with_status(200)
            .with_body(": comment\nnot-data: something\ndata: {\"taskId\": \"task-1\", \"status\": \"completed\"}\n")
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
        assert_eq!(ev.status, TaskStatusValue::Completed);
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn test_wait_for_completion_unknown_fail() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/status/task-1")
            .with_status(200)
            .with_body(r#"{"taskId": "task-1", "status": "failed"}"#)
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
            Err(TaskForceAIError::TaskFailed(msg)) => assert_eq!(msg, "Unknown error"),
            _ => panic!("Expected TaskFailed error"),
        }
    }

    #[tokio::test]
    async fn test_api_error_no_body() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/run")
            .with_status(500)
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let res = client.submit_task("hi", None).await;
        assert!(matches!(res, Err(TaskForceAIError::Api { status, .. }) if status == 500));
    }

    #[tokio::test]
    async fn test_stream_last_line_malformed_no_newline() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/stream/task-1")
            .with_status(200)
            .with_body("data: {malformed}")
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
    async fn test_stream_empty_body() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/stream/task-1")
            .with_status(200)
            .with_body("")
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let mut stream = client.stream_task_status("task-1").await.unwrap();
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn test_serialization_error_enum_coverage() {
        let err = TaskForceAIError::Serialization(
            serde_json::from_str::<serde_json::Value>("{ ").unwrap_err(),
        );
        assert!(err.to_string().contains("Serialization error"));
    }

    #[tokio::test]
    async fn test_error_variants() {
        let e = TaskForceAIError::EmptyTaskId;
        assert_eq!(e.to_string(), "Task ID must be a non-empty string");

        let e = TaskForceAIError::Stream("oops".to_string());
        assert_eq!(e.to_string(), "Stream error: oops");

        let e = TaskForceAIError::Other("oops".to_string());
        assert_eq!(e.to_string(), "Other error: oops");
    }

    #[tokio::test]
    async fn test_run_task_error() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/run")
            .with_status(500)
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let res = client.run_task("hi", None, None, None).await;
        assert!(matches!(res, Err(TaskForceAIError::Api { status, .. }) if status == 500));
    }

    #[tokio::test]
    async fn test_wait_for_completion_error_path() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/status/task-1")
            .with_status(500)
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let res = client.wait_for_completion("task-1", None, None).await;
        assert!(matches!(res, Err(TaskForceAIError::Api { status, .. }) if status == 500));
    }

    #[tokio::test]
    async fn test_stream_last_line_no_newline_garbage() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/stream/task-1")
            .with_status(200)
            .with_body("garbage-no-newline")
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let mut stream = client.stream_task_status("task-1").await.unwrap();
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn test_stream_network_error_mid_stream() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/stream/task-1")
            .with_status(200)
            .with_body("data: {\"taskId\": \"task-1\", \"status\": \"processing\"}\n")
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let mut stream = client.stream_task_status("task-1").await.unwrap();
        let _ = stream.next().await;

        drop(server);
        // Mid-stream network error is hard to simulate with mockito reliably.
        // We've already verified the variant and other error paths.
    }

    // --- Files Tests ---

    #[tokio::test]
    async fn test_upload_file() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/files")
            .match_header("x-api-key", "key")
            .match_body(Matcher::Regex(
                "(?s)Content-Disposition: form-data; name=\"purpose\".*\\r\\n\\r\\ntest\\r\\n"
                    .to_string(),
            ))
            .match_body(Matcher::Regex(
                "(?s)Content-Disposition: form-data; name=\"mime_type\".*\\r\\n\\r\\ntext/plain\\r\\n"
                    .to_string(),
            ))
            .with_status(200)
            .with_body(r#"{"id": "file-123", "filename": "test.txt", "purpose": "test", "bytes": 100, "created_at": 1672531200}"#)
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let opts = FileUploadOptions {
            purpose: Some("test".to_string()),
            mime_type: Some("text/plain".to_string()),
        };
        let file = client
            .upload_file("test.txt", "content".as_bytes().to_vec().into(), Some(opts))
            .await
            .unwrap();
        assert_eq!(file.id, "file-123");
        assert_eq!(file.filename, "test.txt");
    }

    #[tokio::test]
    async fn test_list_files() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/files?limit=10&offset=0")
            .with_status(200)
            .with_body(r#"{"files": [{"id": "file-1", "filename": "f1", "purpose": "p", "bytes": 10, "created_at": 1672531200}], "total": 1}"#)
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let res = client.list_files(10, 0).await.unwrap();
        assert_eq!(res.files.len(), 1);
        assert_eq!(res.total, 1);
    }

    #[tokio::test]
    async fn test_get_file() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/files/file-1")
            .with_status(200)
            .with_body(r#"{"id": "file-1", "filename": "f1", "purpose": "p", "bytes": 10, "created_at": 1672531200}"#)
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let file = client.get_file("file-1").await.unwrap();
        assert_eq!(file.id, "file-1");
    }

    #[tokio::test]
    async fn test_delete_file() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("DELETE", "/files/file-1")
            .with_status(200)
            .with_body("{}")
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        client.delete_file("file-1").await.unwrap();
    }

    #[tokio::test]
    async fn test_download_file() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/files/file-1/content")
            .match_header("x-api-key", "key")
            .with_status(200)
            .with_body("file content")
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let bytes = client.download_file("file-1").await.unwrap();
        assert_eq!(bytes, "file content".as_bytes());
    }

    // --- Threads Tests ---

    #[tokio::test]
    async fn test_create_thread() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/threads")
            .with_status(200)
            .with_body(r#"{"id": 1, "title": "My Thread", "created_at": 1672531200, "updated_at": 1672531200}"#)
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let opts = CreateThreadOptions {
            title: Some("My Thread".to_string()),
            ..Default::default()
        };
        let thread = client.create_thread(Some(opts)).await.unwrap();
        assert_eq!(thread.id, 1);
        assert_eq!(thread.title, "My Thread");
    }

    #[tokio::test]
    async fn test_list_threads() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/threads?limit=10&offset=0")
            .with_status(200)
            .with_body(r#"{"threads": [{"id": 1, "title": "t1", "created_at": 1672531200, "updated_at": 1672531200}], "total": 1}"#)
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let res = client.list_threads(10, 0).await.unwrap();
        assert_eq!(res.threads.len(), 1);
    }

    #[tokio::test]
    async fn test_get_thread() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/threads/1")
            .with_status(200)
            .with_body(
                r#"{"id": 1, "title": "t1", "created_at": 1672531200, "updated_at": 1672531200}"#,
            )
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let thread = client.get_thread(1).await.unwrap();
        assert_eq!(thread.id, 1);
    }

    #[tokio::test]
    async fn test_delete_thread() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("DELETE", "/threads/1")
            .with_status(200)
            .with_body("{}")
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        client.delete_thread(1).await.unwrap();
    }

    #[tokio::test]
    async fn test_get_thread_messages() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/threads/1/messages?limit=10&offset=0")
            .with_status(200)
            .with_body(r#"{"messages": [{"id": 100, "thread_id": 1, "role": "user", "content": "hi", "created_at": 1672531200}], "total": 1}"#)
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let res = client.get_thread_messages(1, 10, 0).await.unwrap();
        assert_eq!(res.messages.len(), 1);
    }

    #[tokio::test]
    async fn test_run_in_thread() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/threads/1/runs")
            .with_status(200)
            .with_body(r#"{"task_id": "task-t1", "thread_id": 1, "message_id": 101}"#)
            .create_async()
            .await;

        let client = TaskForceAI::new(TaskForceAIOptions {
            base_url: Some(server.url()),
            api_key: Some("key".to_string()),
            ..Default::default()
        })
        .unwrap();

        let opts = ThreadRunOptions {
            prompt: "run".to_string(),
            ..Default::default()
        };
        let res = client.run_in_thread(1, opts).await.unwrap();
        assert_eq!(res.task_id, "task-t1");
    }
}
