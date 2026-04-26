pub mod client;
pub mod types;
pub mod error;

pub use client::{GeminiClient, GeminiClientTrait};
pub use types::*;
pub use error::GeminiError;