use anyhow::{Context, Result};
use async_openai::{
  config::OpenAIConfig,
  types::{CreateEmbeddingRequest, EmbeddingInput},
  Client,
};
use poise::serenity_prelude as serenity;
use std::env;

pub struct OpenAIHandler {
  client: Client<OpenAIConfig>,
}

impl OpenAIHandler {
  pub fn new() -> Result<Self> {
    let api_key =
      env::var("OPENAI_API_KEY").with_context(|| "Missing OPENAI_API_KEY environment variable")?;
    let config = OpenAIConfig::new();
    let config = config.with_api_key(api_key);
    let client = Client::with_config(config);

    Ok(Self { client })
  }

  pub async fn create_embedding(&self, input: String, user: serenity::UserId) -> Result<Vec<f32>> {
    let input = CreateEmbeddingRequest {
      model: "text-embedding-ada-002".to_owned(),
      input: EmbeddingInput::String(input),
      user: Some(user.to_string()),
      ..Default::default()
    };

    let embeddings = self.client.embeddings().create(input).await?;

    let embedding = match embeddings.data.len() {
      1 => embeddings.data[0].embedding.clone(),
      _ => {
        return Err(anyhow::anyhow!(
          "Expected 1 embedding, got {}",
          embeddings.data.len()
        ))
      }
    };

    Ok(embedding)
  }
}
