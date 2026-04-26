use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use clap::{Parser, Subcommand};
use gemini_sdk::{GeminiClient, GeminiClientTrait, GeminiModel, Content, Part, GenerateContentRequest};
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};
use futures::stream::StreamExt;

#[derive(Parser)]
#[command(name = "gemini")]
#[command(about = "Unofficial CLI for Gemini SDK")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate content from a prompt
    Generate {
        /// The prompt text
        prompt: String,
        /// Model to use (gemini-1.5-pro, gemini-1.5-flash, gemini-pro, gemini-3.0-pro, gemini-3.5-pro, or custom)
        #[arg(short, long, default_value = "gemini-3.0-pro")]
        model: String,
        /// Path to an image file (optional, for multimodal)
        #[arg(short, long)]
        image: Option<String>,
    },
    /// List available models for your API key
    ListModels,
    /// Interactive chat mode
    Chat {
        /// Model to use (gemini-1.5-pro, gemini-1.5-flash, gemini-pro, gemini-3.0-pro, gemini-3.5-pro, or custom)
        #[arg(short, long, default_value = "gemini-3.0-pro")]
        model: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // TODO: Load API key from env or config
    let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY must be set");
    println!("API key loaded successfully");

    let client = GeminiClient::new(api_key);
    println!("Client created");

    match cli.command {
        Commands::Generate { prompt, model, image } => {
            println!("Generate command: prompt='{}', model='{}', image={:?}", prompt, model, image);
            let model = parse_model(&model)?;
            let mut parts = vec![Part::Text { text: prompt }];
            if let Some(image_path) = image {
                let image_data = load_image(&image_path)?;
                parts.push(Part::InlineData { inline_data: image_data });
            }
            let request = GenerateContentRequest {
                contents: vec![Content {
                    role: "user".to_string(),
                    parts,
                }],
                generation_config: None,
                safety_settings: None,
                system_instruction: None,
            };

            println!("Sending request to Gemini...");
            let response = client.generate_content(model, request).await?;
            println!("Received response with {} candidates", response.candidates.len());
            if let Some(candidate) = response.candidates.first() {
                for part in &candidate.content.parts {
                    match part {
                        Part::Text { text } => {
                            println!("{}", text);
                        }
                        Part::InlineData { inline_data } => {
                            let output_path = save_blob(inline_data)?;
                            println!("Saved generated image to {}", output_path);
                        }
                    }
                }
            }
        }
        Commands::ListModels => {
            println!("Listing available models...");
            let response = client.list_models().await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        Commands::Chat { model } => {
            let model = parse_model(&model)?;
            println!("Starting chat mode. Type 'exit' to quit.");
            let mut history = Vec::new();

            loop {
                print!("You: ");
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                let input = input.trim();

                if input == "exit" {
                    break;
                }

                history.push(Content {
                    role: "user".to_string(),
                    parts: vec![Part::Text { text: input.to_string() }],
                });

                let request = GenerateContentRequest {
                    contents: history.clone(),
                    generation_config: None,
                    safety_settings: None,
                    system_instruction: None,
                };

                let stream_result = client.generate_content_stream(model.clone(), request).await?;
                let mut stream = stream_result;
                print!("Gemini: ");
                io::stdout().flush()?;
                let mut response_text = String::new();

                while let Some(result) = stream.next().await {
                    match result {
                        Ok(partial) => {
                            print!("{}", partial);
                            io::stdout().flush()?;
                            response_text.push_str(&partial);
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            break;
                        }
                    }
                }
                println!();

                history.push(Content {
                    role: "model".to_string(),
                    parts: vec![Part::Text { text: response_text }],
                });
            }
        }
    }

    Ok(())
}

fn parse_model(model_str: &str) -> anyhow::Result<GeminiModel> {
    match model_str {
        "gemini-1.5-pro" => Ok(GeminiModel::Gemini1_5Pro),
        "gemini-1.5-flash" => Ok(GeminiModel::Gemini1_5Flash),
        "gemini-pro" => Ok(GeminiModel::Gemini1_0Pro),
        _ => Ok(GeminiModel::Custom(model_str.to_string())),
    }
}

fn load_image(path: &str) -> anyhow::Result<gemini_sdk::Blob> {
    use std::fs;
    let data = fs::read(path)?;
    let mime_type = mime_guess::from_path(path).first_or_octet_stream().to_string();
    let encoded = STANDARD.encode(data);
    Ok(gemini_sdk::Blob {
        mime_type,
        data: encoded,
    })
}

fn save_blob(blob: &gemini_sdk::Blob) -> anyhow::Result<String> {
    use std::fs;
    let decoded = STANDARD.decode(&blob.data)?;
    let ext = match blob.mime_type.as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        "image/heic" => "heic",
        _ => "bin",
    };
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let file_name = format!("gemini_output_{}.{}", timestamp, ext);
    fs::write(&file_name, decoded)?;
    Ok(file_name)
}