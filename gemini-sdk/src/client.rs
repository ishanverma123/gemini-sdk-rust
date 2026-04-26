use crate::error::GeminiError;
use crate::types::*;
use async_trait::async_trait;
use backoff::{future::retry, ExponentialBackoff};
use futures::stream::Stream;
use governor::{Quota, RateLimiter};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::{wrappers::UnboundedReceiverStream, StreamExt};

#[async_trait]
pub trait GeminiClientTrait {
    async fn generate_content(
        &self,
        model: GeminiModel,
        request: GenerateContentRequest,
    ) -> Result<GenerateContentResponse, GeminiError>;

    async fn generate_content_stream(
        &self,
        model: GeminiModel,
        request: GenerateContentRequest,
    ) -> Result<impl Stream<Item = Result<String, GeminiError>>, GeminiError>;

    async fn list_models(&self) -> Result<Value, GeminiError>;
}

pub struct GeminiClient {
    api_key: String,
    client: Client,
    rate_limiter: Arc<RateLimiter<governor::state::NotKeyed, governor::state::InMemoryState, governor::clock::DefaultClock>>,
}

impl GeminiClient {
    pub fn new(api_key: String) -> Self {
        let quota = Quota::per_minute(NonZeroU32::new(60).unwrap()); // Example: 60 requests per minute
        let rate_limiter = Arc::new(RateLimiter::direct(quota));

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();

        Self {
            api_key,
            client,
            rate_limiter,
        }
    }

    async fn make_request<T: for<'de> Deserialize<'de>>(
        &self,
        url: &str,
        body: &impl Serialize,
    ) -> Result<T, GeminiError> {
        self.rate_limiter.until_ready().await;

        let response = retry(ExponentialBackoff::default(), || async {
            let res = self
                .client
                .post(url)
                .header("Content-Type", "application/json")
                .json(body)
                .send()
                .await
                .map_err(|e| backoff::Error::Permanent(GeminiError::Http(e)))?;

            if res.status().is_success() {
                Ok(res)
            } else {
                let status = res.status();
                let error_text = res.text().await.unwrap_or_default();
                Err(backoff::Error::Permanent(GeminiError::Api {
                    message: error_text,
                    code: status.as_u16(),
                }))
            }
        })
        .await?;

        response.json().await.map_err(Into::into)
    }

    async fn make_get_request<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<T, GeminiError> {
        self.rate_limiter.until_ready().await;

        let response = retry(ExponentialBackoff::default(), || async {
            let res = self
                .client
                .get(url)
                .send()
                .await
                .map_err(|e| backoff::Error::Permanent(GeminiError::Http(e)))?;

            if res.status().is_success() {
                Ok(res)
            } else {
                let status = res.status();
                let error_text = res.text().await.unwrap_or_default();
                Err(backoff::Error::Permanent(GeminiError::Api {
                    message: error_text,
                    code: status.as_u16(),
                }))
            }
        })
        .await?;

        response.json().await.map_err(Into::into)
    }

    async fn make_stream_request(
        &self,
        url: &str,
        body: &impl Serialize,
    ) -> Result<impl Stream<Item = Result<String, GeminiError>>, GeminiError> {
        self.rate_limiter.until_ready().await;

        let response = retry(ExponentialBackoff::default(), || async {
            let res = self
                .client
                .post(url)
                .header("Content-Type", "application/json")
                .json(body)
                .send()
                .await
                .map_err(|e| backoff::Error::Permanent(GeminiError::Http(e)))?;

            if res.status().is_success() {
                Ok(res)
            } else {
                let status = res.status();
                let error_text = res.text().await.unwrap_or_default();
                Err(backoff::Error::Permanent(GeminiError::Api {
                    message: error_text,
                    code: status.as_u16(),
                }))
            }
        })
        .await?;

        let (tx, rx) = mpsc::unbounded_channel();
        let mut bytes_stream = response.bytes_stream();

        tokio::spawn(async move {
            let mut buffer = String::new();
            while let Some(chunk_result) = bytes_stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));
                        while let Some(delim_index) = buffer.find("\n\n") {
                            let mut frame = buffer.drain(..delim_index + 2).collect::<String>();
                            frame = frame.trim().to_string();
                            if frame.is_empty() {
                                continue;
                            }
                            if let Some(text) = parse_sse_frame(&frame) {
                                if tx.send(Ok(text)).is_err() {
                                    return;
                                }
                            }
                        }
                    }
                    Err(err) => {
                        let _ = tx.send(Err(GeminiError::Http(err)));
                        return;
                    }
                }
            }

            if !buffer.trim().is_empty() {
                if let Some(text) = parse_sse_frame(buffer.trim()) {
                    let _ = tx.send(Ok(text));
                }
            }
        });

        Ok(UnboundedReceiverStream::new(rx))
    }
}

#[async_trait]
impl GeminiClientTrait for GeminiClient {
    async fn generate_content(
        &self,
        model: GeminiModel,
        request: GenerateContentRequest,
    ) -> Result<GenerateContentResponse, GeminiError> {
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model.as_str(), self.api_key);
        self.make_request(&url, &request).await
    }

    async fn generate_content_stream(
        &self,
        model: GeminiModel,
        request: GenerateContentRequest,
    ) -> Result<impl Stream<Item = Result<String, GeminiError>>, GeminiError> {
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?key={}", model.as_str(), self.api_key);
        self.make_stream_request(&url, &request).await
    }

    async fn list_models(&self) -> Result<Value, GeminiError> {
        let url = format!("https://generativelanguage.googleapis.com/v1/models?key={}", self.api_key);
        self.make_get_request(&url).await
    }
}

fn parse_sse_frame(frame: &str) -> Option<String> {
    let payload = frame
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join("\n");

    let payload = payload.trim();
    if payload.is_empty() || payload == "[DONE]" {
        return None;
    }

    let value: Value = serde_json::from_str(payload).ok()?;
    let text = extract_text_from_value(&value);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn extract_text_from_value(value: &Value) -> String {
    let mut pieces = Vec::new();
    collect_texts(value, &mut pieces);
    pieces.join("")
}

fn collect_texts(value: &Value, pieces: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            pieces.push(text.clone());
        }
        Value::Array(items) => {
            for item in items {
                collect_texts(item, pieces);
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                if key == "text" {
                    if let Some(text) = item.as_str() {
                        pieces.push(text.to_string());
                    }
                } else {
                    collect_texts(item, pieces);
                }
            }
        }
        _ => {}
    }
}
