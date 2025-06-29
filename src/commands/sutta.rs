use anyhow::Result;
use poise::CreateReply;
use poise::serenity_prelude::CreateEmbedAuthor;
use tracing::warn;

use crate::Context;
use crate::config::{BloomBotEmbed, EMOJI};
use crate::data::sutta::{SuttaCentralSutta, Suttapitaka};
use crate::data::tracking_profile::Privacy;

/// Get a random Pali Canon sutta
///
/// Get a random sutta from the Early Discourses (Sutta Piṭaka) of the Pali Canon
/// via SuttaCentral.net, with the option to specify the collection from which the
/// sutta is chosen.
#[poise::command(slash_command, category = "Informational", guild_only)]
pub async fn sutta(
  ctx: Context<'_>,
  #[description = "Specify the collection (defaults to all)"] collection: Option<Suttapitaka>,
  #[description = "Specify the visibility (defaults to public; private can show more text)"]
  visibility: Option<Privacy>,
) -> Result<()> {
  let data = ctx.data();
  let ephemeral = matches!(visibility.unwrap_or_default(), Privacy::Private);

  let collection = collection.unwrap_or_default();
  let sutta_id = collection.random(data.rng.clone()).await;
  let sutta = match SuttaCentralSutta::new(sutta_id)
    .populate(data.http.clone())
    .await
  {
    Ok(sutta) => sutta,
    Err(e) => {
      warn!("Failed to retrieve sutta data for '{sutta_id}': {e}");
      let msg = format!(
        "{} Failed to retrieve sutta data from SuttaCentral. Please try again.",
        EMOJI.mminfo
      );
      ctx
        .send(CreateReply::default().content(msg).ephemeral(true))
        .await?;
      return Ok(());
    }
  };

  let (title, verses) = sutta.construct(ephemeral);

  let embed = BloomBotEmbed::new()
    .author(CreateEmbedAuthor::new(title))
    .description(format!("```{verses}```{}", sutta.footer()));
  ctx
    .send(CreateReply::default().embed(embed).ephemeral(ephemeral))
    .await?;

  Ok(())
}
