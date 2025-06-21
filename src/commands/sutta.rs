use std::sync::Arc;

use anyhow::Result;
use poise::CreateReply;
use poise::serenity_prelude::CreateEmbedAuthor;

use crate::Context;
use crate::config::BloomBotEmbed;
use crate::data::sutta::{SuttaCentralSutta, SuttaCentralSuttaData, SuttaSection, Suttapitaka};

/// Get a random Pali Canon sutta
///
/// Get a random sutta from the Early Discourses (Sutta Piṭaka) of the Pali Canon
/// via SuttaCentral.net, with the option to specify the collection from which the
/// sutta is chosen.
#[poise::command(slash_command, category = "Informational", guild_only)]
pub async fn sutta(
  ctx: Context<'_>,
  #[description = "Specify the collection (defaults to all)"] collection: Option<Suttapitaka>,
) -> Result<()> {
  let data = ctx.data();

  let rng = Arc::clone(&data.rng);
  let rng = rng.lock().await;

  let collection = collection.unwrap_or_default();
  let sutta_id = collection.random(rng);

  let mut sutta = SuttaCentralSutta::new(sutta_id);
  sutta.data = data
    .http
    .get(sutta.api_url())
    .send()
    .await?
    .json::<SuttaCentralSuttaData>()
    .await?;

  let title = sutta.construct_sutta(&SuttaSection::Title);
  let verses = sutta.construct_sutta(&SuttaSection::Verses);
  let text = format!("```{verses}```{}", sutta.footer());

  let embed = BloomBotEmbed::new()
    .author(CreateEmbedAuthor::new(title))
    .description(text);
  ctx.send(CreateReply::default().embed(embed)).await?;

  Ok(())
}
