# Gemini SDK for Rust

An unofficial Rust workspace for working with Google's Gemini API. This repository contains:

- `gemini-sdk`: the async library crate
- `gemini-cli`: a command-line client built on top of the SDK

This README is intentionally detailed. It documents the repository structure, the implementation flow, the public functions, the enums, the request/response types, the constructor patterns, and the CLI arguments as they exist in the current codebase.

## Table of Contents

1. [Workspace Overview](#workspace-overview)
2. [Repository Structure](#repository-structure)
3. [What the Project Does](#what-the-project-does)
4. [Build and Run Steps](#build-and-run-steps)
5. [Implementation Flow: Step by Step](#implementation-flow-step-by-step)
6. [SDK Crate Reference](#sdk-crate-reference)
7. [Types, Structs, Enums, and Constructors](#types-structs-enums-and-constructors)
8. [CLI Crate Reference](#cli-crate-reference)
9. [Streaming Internals](#streaming-internals)
10. [Error Handling](#error-handling)
11. [Current Behaviors and Notes](#current-behaviors-and-notes)
12. [Verification](#verification)

## Workspace Overview

This is a Cargo workspace with two members:

- `gemini-sdk`
- `gemini-cli`

The workspace root `Cargo.toml` centralizes shared dependencies such as:

- `tokio`
- `reqwest`
- `serde`
- `serde_json`
- `thiserror`
- `futures`
- `tokio-stream`
- `backoff`
- `governor`
- `base64`
- `clap`
- `async-trait`
- `mime_guess`

The workspace uses resolver version `2`.

## Repository Structure

```text
gemini-sdk-rust/
|-- Cargo.toml
|-- Cargo.lock
|-- README.md
|-- gemini-sdk/
|   |-- Cargo.toml
|   `-- src/
|       |-- lib.rs
|       |-- client.rs
|       |-- types.rs
|       `-- error.rs
`-- gemini-cli/
    |-- Cargo.toml
    `-- src/
        `-- main.rs
```

### File-by-file purpose

#### Root

- `Cargo.toml`
  Declares the workspace and shared dependencies.

- `Cargo.lock`
  Locks dependency versions for reproducible builds.

- `README.md`
  Project-level documentation.

#### `gemini-sdk`

- `src/lib.rs`
  Re-exports the library modules and public types.

- `src/client.rs`
  Contains the main Gemini client, its trait, retry logic, rate limiting, streaming support, and HTTP request helpers.

- `src/types.rs`
  Contains request/response structs, content parts, configuration types, safety enums, and model enums.

- `src/error.rs`
  Defines the SDK's error enum.

#### `gemini-cli`

- `src/main.rs`
  Contains the CLI parser, subcommands, runtime entrypoint, and helper functions for model parsing, image loading, prompt routing, and binary output saving.

## What the Project Does

At a high level, the repository gives you two ways to work with Gemini:

1. As a library:
   Create a `GeminiClient`, build a `GenerateContentRequest`, choose a model, and call async SDK methods.

2. As a CLI:
   Run commands like `generate`, `chat`, `image`, or `list-models` from the terminal.

The current implementation supports:

- standard content generation
- streaming content generation
- multimodal input with text and image data
- image output handling when Gemini returns binary content as inline data
- model listing
- retry with exponential backoff
- simple per-minute rate limiting

## Build and Run Steps

### Prerequisites

- Rust toolchain installed
- Cargo installed
- A valid Gemini API key

### 1. Clone the repository

```bash
git clone <your-repo-url>
cd gemini-sdk-rust
```

### 2. Provide the API key

The CLI currently expects the environment variable `GEMINI_API_KEY`.

```bash
export GEMINI_API_KEY=your_api_key_here
```

### 3. Build the workspace

```bash
cargo build
```

### 4. Verify compilation

```bash
cargo check
```

### 5. Run the CLI

Generate text:

```bash
cargo run -p gemini-cli -- generate "Explain async Rust simply"
```

Run interactive chat:

```bash
cargo run -p gemini-cli -- chat
```

List models:

```bash
cargo run -p gemini-cli -- list-models
```

Generate an image:

```bash
cargo run -p gemini-cli -- image "A watercolor fox reading a book"
```

### 6. Use the SDK from another Rust project

Add the dependency:

```toml
[dependencies]
gemini-sdk = { path = "../gemini-sdk-rust/gemini-sdk" }
tokio = { version = "1", features = ["full"] }
```

Example:

```rust
use gemini_sdk::{
    Content,
    GenerateContentRequest,
    GeminiClient,
    GeminiClientTrait,
    GeminiModel,
    Part,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("GEMINI_API_KEY")?;
    let client = GeminiClient::new(api_key);

    let request = GenerateContentRequest {
        contents: vec![Content {
            role: "user".to_string(),
            parts: vec![Part::Text {
                text: "Hello from Rust".to_string(),
            }],
        }],
        generation_config: None,
        safety_settings: None,
        system_instruction: None,
    };

    let response = client
        .generate_content(GeminiModel::Gemini1_5Flash, request)
        .await?;

    if let Some(candidate) = response.candidates.first() {
        for part in &candidate.content.parts {
            if let Part::Text { text } = part {
                println!("{}", text);
            }
        }
    }

    Ok(())
}
```

## Implementation Flow: Step by Step

This section explains the main steps the code takes from input to output.

### Step 1: Parse CLI arguments

In `gemini-cli`, Clap parses the top-level `Cli` struct and the `Commands` enum.

Possible commands:

- `Generate`
- `ListModels`
- `Chat`
- `Image`

### Step 2: Load the API key

The `main` function reads:

```text
GEMINI_API_KEY
```

If the variable is missing, the CLI exits immediately using `expect`.

### Step 3: Construct the client

The CLI creates:

```rust
let client = GeminiClient::new(api_key);
```

Inside `GeminiClient::new`:

- a rate limiter is created with a quota of 60 requests per minute
- a `reqwest::Client` is created
- the HTTP client timeout is set to 30 seconds
- the API key, HTTP client, and limiter are stored on the struct

### Step 4: Convert user input into request types

Depending on the command:

- text prompts become `Part::Text`
- image files become `Part::InlineData`
- chat history becomes a `Vec<Content>`
- image generation uses a prompt wrapped into a `GenerateContentRequest`

### Step 5: Resolve the target model

The code uses:

- `GeminiModel` for text/content generation
- `ImageGenerationModel` for image generation

The CLI helper `parse_model` converts known model strings into enum variants and treats unknown strings as custom model names.

### Step 6: Make the HTTP call

The SDK methods build URLs against Gemini endpoints and call internal helpers:

- `make_request`
- `make_get_request`
- `make_stream_request`

These helpers:

- wait for the rate limiter
- send the request with `reqwest`
- retry using `backoff::future::retry`
- map HTTP failures into `GeminiError`
- deserialize JSON responses

### Step 7: Handle the response

For normal text generation:

- the code reads `response.candidates`
- it prints text parts directly
- if inline image data is returned, it saves it to disk

For chat:

- the code consumes the streaming API
- partial text chunks are printed live
- the final accumulated text is appended to chat history as the model response

### Step 8: Persist output files when needed

If a response contains image bytes in `InlineData`, the CLI decodes the base64 payload and writes a timestamped file such as:

```text
gemini_output_<unix_timestamp>.png
```

## SDK Crate Reference

The SDK crate exposes three modules:

- `client`
- `types`
- `error`

From `lib.rs`, the crate publicly re-exports:

- `GeminiClient`
- `GeminiClientTrait`
- all types from `types.rs`
- `GeminiError`

### Public client trait

The main async interface is `GeminiClientTrait`.

It defines four async methods:

#### `generate_content`

```rust
async fn generate_content(
    &self,
    model: GeminiModel,
    request: GenerateContentRequest,
) -> Result<GenerateContentResponse, GeminiError>;
```

Purpose:

- send a normal non-streaming content generation request

Inputs:

- `model`: which Gemini model to call
- `request`: the full request payload

Output:

- `GenerateContentResponse`

#### `generate_content_stream`

```rust
async fn generate_content_stream(
    &self,
    model: GeminiModel,
    request: GenerateContentRequest,
) -> Result<impl futures::stream::Stream<Item = Result<String, GeminiError>>, GeminiError>;
```

Purpose:

- send a streaming generation request

Output:

- a stream of partial text chunks

#### `generate_image`

```rust
async fn generate_image(
    &self,
    model: ImageGenerationModel,
    prompt: String,
) -> Result<GenerateContentResponse, GeminiError>;
```

Purpose:

- wrap a plain text prompt into a `GenerateContentRequest`
- send it to an image-capable Gemini model

#### `list_models`

```rust
async fn list_models(&self) -> Result<serde_json::Value, GeminiError>;
```

Purpose:

- fetch the model list from the API

### `GeminiClient`

`GeminiClient` is the concrete implementation of `GeminiClientTrait`.

#### Fields

The struct stores:

- `api_key: String`
- `client: reqwest::Client`
- `rate_limiter: Arc<RateLimiter<...>>`

The rate limiter uses a direct in-memory `governor` limiter.

#### Constructor

```rust
pub fn new(api_key: String) -> Self
```

What it does:

- accepts the API key as an owned `String`
- creates a quota of 60 requests per minute
- builds a `reqwest::Client` with a 30-second timeout
- returns a fully initialized client

There are currently no additional constructors such as:

- `with_timeout`
- `with_base_url`
- `with_rate_limit`
- `from_env`

So the current construction pattern is intentionally simple:

```rust
let client = GeminiClient::new(api_key);
```

### Internal helper functions in `client.rs`

These are not re-exported as public crate API, but they are important to understand the implementation.

#### `make_request`

```rust
async fn make_request<T: for<'de> Deserialize<'de>>(
    &self,
    url: &str,
    body: &impl Serialize,
) -> Result<T, GeminiError>
```

Responsibilities:

- rate-limit outgoing requests
- send a POST request
- serialize the request body as JSON
- retry failed requests through the `backoff` crate
- convert API failures into `GeminiError::Api`
- deserialize the JSON response into `T`

#### `make_get_request`

```rust
async fn make_get_request<T: for<'de> Deserialize<'de>>(
    &self,
    url: &str,
) -> Result<T, GeminiError>
```

Responsibilities:

- rate-limit the call
- send a GET request
- retry on failure
- deserialize the JSON result

#### `make_stream_request`

```rust
async fn make_stream_request(
    &self,
    url: &str,
    body: &impl Serialize,
) -> Result<impl Stream<Item = Result<String, GeminiError>>, GeminiError>
```

Responsibilities:

- send the streaming request
- read raw byte chunks
- buffer partial frames
- split SSE-style frames on blank lines
- extract text from JSON payloads
- expose the result as a Tokio receiver-backed stream

#### `parse_sse_frame`

```rust
fn parse_sse_frame(frame: &str) -> Option<String>
```

Responsibilities:

- keep only lines beginning with `data:`
- join them into a payload
- ignore empty payloads and `[DONE]`
- parse the payload as JSON
- extract text using `extract_text_from_value`

#### `extract_text_from_value`

```rust
fn extract_text_from_value(value: &serde_json::Value) -> String
```

Responsibilities:

- recursively gather text segments from nested JSON
- join them into a single string

#### `collect_texts`

```rust
fn collect_texts(value: &serde_json::Value, pieces: &mut Vec<String>)
```

Responsibilities:

- walk strings, arrays, and objects
- collect values from any `text` fields
- recurse into nested structures

## Types, Structs, Enums, and Constructors

This section focuses on the data model in `gemini-sdk/src/types.rs`.

### `GenerateContentRequest`

```rust
pub struct GenerateContentRequest {
    pub contents: Vec<Content>,
    pub generation_config: Option<GenerationConfig>,
    pub safety_settings: Option<Vec<SafetySetting>>,
    pub system_instruction: Option<Content>,
}
```

Purpose:

- top-level request body sent to Gemini for content generation

Construction pattern:

- this struct does not provide a dedicated constructor
- it is built using a struct literal

Example:

```rust
let request = GenerateContentRequest {
    contents: vec![Content {
        role: "user".to_string(),
        parts: vec![Part::Text {
            text: "Summarize Rust traits".to_string(),
        }],
    }],
    generation_config: Some(GenerationConfig {
        temperature: Some(0.7),
        top_k: None,
        top_p: Some(0.95),
        max_output_tokens: Some(512),
        stop_sequences: None,
    }),
    safety_settings: None,
    system_instruction: None,
};
```

### `Content`

```rust
pub struct Content {
    pub role: String,
    pub parts: Vec<Part>,
}
```

Purpose:

- represents one message-like content block

Typical values for `role` in the current code:

- `"user"`
- `"model"`

Construction pattern:

- built with a struct literal

### `Part`

```rust
pub enum Part {
    Text { text: String },
    InlineData { inline_data: Blob },
}
```

Purpose:

- represents either text or binary inline content

Variants:

- `Text { text: String }`
- `InlineData { inline_data: Blob }`

Examples:

```rust
let text_part = Part::Text {
    text: "Describe this image".to_string(),
};
```

```rust
let image_part = Part::InlineData {
    inline_data: Blob {
        mime_type: "image/png".to_string(),
        data: "<base64>".to_string(),
    },
};
```

### `Blob`

```rust
pub struct Blob {
    pub mime_type: String,
    pub data: String,
}
```

Purpose:

- stores inline binary data as base64 text

Notes:

- `mime_type` identifies the binary content type
- `data` must contain base64-encoded bytes

### `GenerationConfig`

```rust
pub struct GenerationConfig {
    pub temperature: Option<f32>,
    pub top_k: Option<i32>,
    pub top_p: Option<f32>,
    pub max_output_tokens: Option<i32>,
    pub stop_sequences: Option<Vec<String>>,
}
```

Purpose:

- optional decoding and generation settings

Construction pattern:

- struct literal
- every field is optional

### `SafetySetting`

```rust
pub struct SafetySetting {
    pub category: SafetyCategory,
    pub threshold: SafetyThreshold,
}
```

Purpose:

- one safety rule pairing a category with a threshold

Example:

```rust
let setting = SafetySetting {
    category: SafetyCategory::DangerousContent,
    threshold: SafetyThreshold::BlockMediumAndAbove,
};
```

### `SafetyCategory`

```rust
pub enum SafetyCategory {
    Harassment,
    HateSpeech,
    SexuallyExplicit,
    DangerousContent,
}
```

Serialized values:

- `HARM_CATEGORY_HARASSMENT`
- `HARM_CATEGORY_HATE_SPEECH`
- `HARM_CATEGORY_SEXUALLY_EXPLICIT`
- `HARM_CATEGORY_DANGEROUS_CONTENT`

### `SafetyThreshold`

```rust
pub enum SafetyThreshold {
    BlockLowAndAbove,
    BlockMediumAndAbove,
    BlockOnlyHigh,
    BlockNone,
}
```

Serialized values:

- `BLOCK_LOW_AND_ABOVE`
- `BLOCK_MEDIUM_AND_ABOVE`
- `BLOCK_ONLY_HIGH`
- `BLOCK_NONE`

### `GenerateContentResponse`

```rust
pub struct GenerateContentResponse {
    pub candidates: Vec<Candidate>,
    pub usage_metadata: Option<UsageMetadata>,
}
```

Purpose:

- top-level parsed response from the Gemini API

### `Candidate`

```rust
pub struct Candidate {
    pub content: Content,
    pub finish_reason: Option<String>,
    pub index: Option<i32>,
    pub safety_ratings: Option<Vec<SafetyRating>>,
}
```

Purpose:

- one returned candidate from Gemini

### `SafetyRating`

```rust
pub struct SafetyRating {
    pub category: SafetyCategory,
    pub probability: String,
}
```

Purpose:

- returned safety metadata for a candidate

### `UsageMetadata`

```rust
pub struct UsageMetadata {
    pub prompt_token_count: i32,
    pub candidates_token_count: i32,
    pub total_token_count: i32,
}
```

Purpose:

- token accounting metadata when the API returns it

### `ImageGenerationModel`

```rust
pub struct ImageGenerationModel(pub String);
```

This is a tuple struct that wraps a model name.

#### Constructor patterns

There are two ways to construct it.

Associated helper constructors:

```rust
ImageGenerationModel::nano_banana_2_5()
ImageGenerationModel::nano_banana_2()
ImageGenerationModel::nano_banana_pro()
```

These currently resolve to:

- `gemini-2.5-flash-image`
- `gemini-3.1-flash-image-preview`
- `gemini-3-pro-image-preview`

Direct tuple construction:

```rust
let model = ImageGenerationModel("my-custom-image-model".to_string());
```

#### Accessor

```rust
pub fn as_str(&self) -> &str
```

Purpose:

- returns the wrapped model string as `&str`

### `GeminiModel`

```rust
pub enum GeminiModel {
    Gemini1_5Pro,
    Gemini1_5Flash,
    Gemini1_0Pro,
    Custom(String),
}
```

Purpose:

- strongly typed selection for text/content models

Variants map to:

- `Gemini1_5Pro` -> `gemini-1.5-pro`
- `Gemini1_5Flash` -> `gemini-1.5-flash`
- `Gemini1_0Pro` -> `gemini-pro`
- `Custom(String)` -> caller-provided model name

#### Accessor

```rust
pub fn as_str(&self) -> &str
```

Purpose:

- returns the model name used in the request URL

Example:

```rust
let model = GeminiModel::Custom("gemini-3.0-pro".to_string());
assert_eq!(model.as_str(), "gemini-3.0-pro");
```

## CLI Crate Reference

The CLI is defined in `gemini-cli/src/main.rs`.

### Top-level parser

```rust
#[derive(Parser)]
struct Cli {
    command: Commands,
}
```

Purpose:

- parse one subcommand from the terminal

### `Commands` enum

```rust
enum Commands {
    Generate { ... },
    ListModels,
    Chat { ... },
    Image { ... },
}
```

Each variant corresponds to a CLI mode.

### `Generate` command

```rust
Generate {
    prompt: String,
    model: String,
    image: Option<String>,
}
```

Arguments:

- `prompt`
  Required positional text prompt.

- `--model`, `-m`
  Optional model string.
  Default in the current CLI: `gemini-3.0-pro`

- `--image`, `-i`
  Optional path to an image file for multimodal input.

Behavior:

- if the prompt looks like an image-generation request and `--image` is not provided, the CLI routes to `generate_image`
- otherwise it creates a standard `GenerateContentRequest`

### `ListModels` command

```rust
ListModels
```

Arguments:

- none

Behavior:

- calls `client.list_models()`
- pretty-prints the raw JSON response

### `Chat` command

```rust
Chat {
    model: String,
}
```

Arguments:

- `--model`, `-m`
  Optional model string.
  Default in the current CLI: `gemini-3.0-pro`

Behavior:

- starts an interactive loop
- reads user input from stdin
- sends the entire chat history each turn
- consumes the streaming endpoint
- prints partial text as it arrives
- appends the final model text back into history

Exit condition:

- typing `exit`

### `Image` command

```rust
Image {
    prompt: String,
    model: String,
    size: Option<String>,
}
```

Arguments:

- `prompt`
  Required positional image prompt.

- `--model`, `-m`
  Optional image model string.
  Default in the current CLI: `gemini-image-001`

- `--size`, `-s`
  Optional output size argument.

Important note:

- `size` is parsed by Clap but is not currently used anywhere in the implementation

### Helper functions in the CLI

#### `parse_model`

```rust
fn parse_model(model_str: &str) -> anyhow::Result<GeminiModel>
```

Behavior:

- maps known model names to enum variants
- falls back to `GeminiModel::Custom`

Mappings:

- `gemini-1.5-pro` -> `GeminiModel::Gemini1_5Pro`
- `gemini-1.5-flash` -> `GeminiModel::Gemini1_5Flash`
- `gemini-pro` -> `GeminiModel::Gemini1_0Pro`
- anything else -> `GeminiModel::Custom`

#### `is_image_request`

```rust
fn is_image_request(prompt: &str) -> bool
```

Behavior:

- lowercases the prompt
- checks it for image-oriented keywords such as:
  - `generate image`
  - `create image`
  - `draw`
  - `illustrate`
  - `paint`
  - `render`
  - `picture of`

Purpose:

- auto-route some prompts to image generation

#### `load_image`

```rust
fn load_image(path: &str) -> anyhow::Result<gemini_sdk::Blob>
```

Behavior:

- reads the file from disk
- guesses the MIME type with `mime_guess`
- base64-encodes the bytes
- returns a `Blob`

Purpose:

- convert an image file into `Part::InlineData`

#### `save_blob`

```rust
fn save_blob(blob: &gemini_sdk::Blob) -> anyhow::Result<String>
```

Behavior:

- decodes base64 image bytes
- chooses a file extension from `mime_type`
- writes the file into the current working directory
- returns the saved filename

Supported file extension mapping includes:

- `image/png` -> `png`
- `image/jpeg` and `image/jpg` -> `jpg`
- `image/webp` -> `webp`
- `image/gif` -> `gif`
- `image/bmp` -> `bmp`
- `image/heic` -> `heic`
- everything else -> `bin`

## Streaming Internals

The streaming path is one of the more important parts of this repository.

### How streaming works in this codebase

1. `generate_content_stream` builds a `streamGenerateContent` URL.
2. `make_stream_request` sends the POST request.
3. The SDK reads `response.bytes_stream()`.
4. Raw bytes are appended into a string buffer.
5. The buffer is split on `\n\n`, which acts as the SSE frame delimiter.
6. Each frame is passed into `parse_sse_frame`.
7. `parse_sse_frame` extracts `data:` lines and parses them as JSON.
8. `extract_text_from_value` and `collect_texts` recursively gather text.
9. Partial text is sent through an unbounded Tokio channel.
10. The caller receives a `Stream<Item = Result<String, GeminiError>>`.

### Why this matters

This design lets the CLI print text progressively instead of waiting for the full response to complete.

## Error Handling

The SDK defines `GeminiError` in `gemini-sdk/src/error.rs`.

```rust
pub enum GeminiError {
    Http(reqwest::Error),
    Json(serde_json::Error),
    Api { message: String, code: u16 },
    Auth,
    RateLimit,
    InvalidRequest(String),
    Io(std::io::Error),
    Base64(base64::DecodeError),
}
```

### Variants

- `Http`
  Wraps `reqwest::Error`.

- `Json`
  Wraps `serde_json::Error`.

- `Api`
  Stores an API message and HTTP status code.

- `Auth`
  Reserved authentication-oriented error variant.

- `RateLimit`
  Reserved rate-limit-oriented error variant.

- `InvalidRequest`
  Stores a custom invalid-request message.

- `Io`
  Wraps `std::io::Error`.

- `Base64`
  Wraps `base64::DecodeError`.

### Important implementation note

Not every error variant is currently constructed by the code paths in `client.rs` or `main.rs`. Some are available for future expansion.

## Current Behaviors and Notes

This section documents a few important implementation details exactly as the repository behaves now.

### 1. The client is opinionated but simple

`GeminiClient::new` hardcodes:

- 60 requests per minute
- 30-second HTTP timeout

There is no builder API yet.

### 2. Request types use public fields instead of builders

Most SDK request and config types are built with direct struct literals. That keeps the code easy to inspect, but it means callers build payloads manually.

### 3. The CLI defaults to model strings not explicitly represented as enum variants

The CLI defaults `generate` and `chat` to `gemini-3.0-pro`, which is not a dedicated `GeminiModel` enum variant. That is still valid because `parse_model` falls back to `GeminiModel::Custom`.

### 4. Image command `size` is not wired up yet

The argument exists, but the current implementation ignores it.

### 5. The CLI prints debug-style progress messages

Examples include:

- `API key loaded successfully`
- `Client created`
- `Sending request to Gemini...`

This is useful for local visibility, but it also means the CLI is somewhat verbose.

### 6. Image output is saved locally

If Gemini returns inline image data, the CLI writes it to a timestamped file in the current directory.

## Verification

The repository was checked with:

```bash
cargo check
```
