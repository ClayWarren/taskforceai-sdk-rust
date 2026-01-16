# TaskForceAI Rust SDK

Official Rust SDK for the TaskForceAI multi-agent orchestration API.

## Installation

Install using `cargo`:

```bash
cargo add taskforceai-sdk
```

Or add this to your `Cargo.toml`:

```toml
[dependencies]
taskforceai-sdk = "0.1.0"
tokio = { version = "1.0", features = ["full"] }
```

## Quick Start

```rust
use taskforceai_sdk::{TaskForceAI, TaskForceAIOptions};

#[tokio::main]
async fn main() {
    let opts = TaskForceAIOptions {
        api_key: Some("your-api-key-here".to_string()),
        ..Default::default()
    };

    let client = TaskForceAI::new(opts).expect("Failed to create client");

    // Run a task and wait for completion
    let status = client.run_task("Analyze this sentiment: 'Rust is amazing!'", None, None, None)
        .await
        .expect("Task failed");

    if let Some(result) = status.result {
        println!("Result: {}", result);
    }
}
```

## API Reference

### TaskForceAI

The main client struct.

#### `new(options: TaskForceAIOptions) -> Result<Self, TaskForceAIError>`

Creates a new client instance.

**Options:**

- `api_key`: Your API key (required unless `mock_mode` is true)
- `base_url`: Optional custom endpoint
- `timeout`: Request timeout in seconds
- `mock_mode`: Enable local mocking

### Methods

#### `submit_task(&self, prompt: &str, options: Option<TaskSubmissionOptions>) -> Result<String, TaskForceAIError>`

Submits a task and returns the Task ID.

#### `get_task_status(&self, task_id: &str) -> Result<TaskStatus, TaskForceAIError>`

Gets current status/result for a task.

#### `wait_for_completion(&self, task_id: &str, interval: Option<Duration>, max_attempts: Option<u32>) -> Result<TaskStatus, TaskForceAIError>`

Polls until the task is finished.

#### `run_task(...)`

Shortcut for submit + wait.

#### `stream_task_status(&self, task_id: &str) -> Result<TaskStatusStream, TaskForceAIError>`

Returns a Stream of status updates using SSE.

#### `run_task_stream(...)`

Shortcut for submit + stream.

## Real-time Streaming

```rust
use futures_util::StreamExt;

let mut stream = client.run_task_stream("Generate a long story...", None).await?;

while let Some(event) = stream.next().await {
    let status = event?;
    println!("Status: {}", status.status);
}
```

## License

MIT
