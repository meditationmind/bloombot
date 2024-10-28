use anyhow::{Context, Result};
use async_openai::{config::OpenAIConfig, types::CreateEmbeddingRequestArgs, Client};
use poise::serenity_prelude as serenity;
use std::env;

pub struct OpenAIHandler {
  client: Client<OpenAIConfig>,
}

impl OpenAIHandler {
  /// Creates and configures a client to interact with the [OpenAI API], using the default
  /// v1 API base url and an API key specified in the `OPENAI_API_KEY` environment variable.
  ///
  /// # Errors
  /// Returns an error if the `OPENAI_API_KEY` environment variable is missing.
  ///
  /// [OpenAI API]: https://platform.openai.com/docs/api-reference/introduction
  pub fn new() -> Result<Self> {
    let api_key =
      env::var("OPENAI_API_KEY").with_context(|| "Missing OPENAI_API_KEY environment variable")?;
    let config = OpenAIConfig::new().with_api_key(api_key);
    let client = Client::with_config(config);

    Ok(Self { client })
  }

  /// Creates an embedding vector representing the input text, using a ``UserID`` as the unique end-user identifier.
  ///
  /// # Errors
  /// Returns an error if more than one embedding was generated.
  pub async fn create_embedding(&self, input: String, user: serenity::UserId) -> Result<Vec<f32>> {
    let request = CreateEmbeddingRequestArgs::default()
      .model("text-embedding-ada-002")
      .input(input)
      .user(user.to_string())
      .build()?;

    let embeddings = self.client.embeddings().create(request).await?;

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
