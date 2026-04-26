# Gemini SDK for Rust

An unofficial, fully typed, async-first Rust SDK for Google's Gemini API, featuring streaming responses, multimodal support, retry/backoff, rate limiting, and a CLI tool.

## Features

- **Async-first**: Built with Tokio for high performance.
- **Fully typed**: Strong typing for all API interactions.
- **Streaming support**: Real-time streaming of responses.
- **Multimodal**: Support for text and images.
- **Resilient**: Automatic retry with exponential backoff and rate limiting.
- **CLI tool**: Command-line interface for easy interaction (`gemini generate`, `gemini chat`).

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
gemini-sdk = "0.1.0"
```

For the CLI:

```bash
cargo install gemini-cli
```

## Usage

### SDK

```rust
use gemini_sdk::{GeminiClient, GeminiModel, Content, Part, GenerateContentRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("GEMINI_API_KEY")?;
    let client = GeminiClient::new(api_key);

    let request = GenerateContentRequest {
        contents: vec![Content {
            role: "user".to_string(),
            parts: vec![Part::Text { text: "Hello, Gemini!".to_string() }],
        }],
        generation_config: None,
        safety_settings: None,
        system_instruction: None,
    };

    let response = client.generate_content(GeminiModel::Gemini1_5Flash, request).await?;
    println!("{}", response.candidates[0].content.parts[0].text);
    Ok(())
}
```

### CLI

Set your API key:

```bash
export GEMINI_API_KEY=your_api_key_here
```

Generate text:

```bash
gemini generate "Explain quantum computing in simple terms"
```

Chat mode:

```bash
gemini chat
```

Generate with image:

```bash
gemini generate "Describe this image" --image path/to/image.png
```

## API Reference

See the [docs](https://docs.rs/gemini-sdk) for full API details.

## Challenges Overcome

- **Streaming Parsing**: Handling SSE-like responses from Gemini's streaming endpoint.
- **Multimodal Encoding**: Base64 encoding images with proper MIME types.
- **Error Handling**: Mapping Gemini's error responses to Rust types with retries.
- **Rate Limiting**: Implementing token bucket rate limiting to respect API quotas.
- **Async Design**: Ensuring all operations are non-blocking and composable.

## Contributing

Contributions welcome! Please open issues or PRs on GitHub.

## License

MIT License