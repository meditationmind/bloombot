use anyhow::{Context as AnyhowContext, Result};
use poise::CreateReply;

use crate::Context;
use crate::commands::helpers::common;
use crate::config::{BloomBotEmbed, EMOJI};
use crate::database::DatabaseHandler;

/// Get a meditation/mindfulness quote
///
/// Get a random meditation/mindfulness quote.
#[poise::command(
  slash_command,
  category = "Informational",
  member_cooldown = 300,
  guild_only
)]
pub async fn quote(
  ctx: Context<'_>,
  #[description = "Refine quote pool with one or more keywords in search engine format"]
  keyword: Option<String>,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  if let Some(keyword) = keyword {
    if common::is_supporter(ctx).await {
      if let Some(quote) =
        DatabaseHandler::get_random_quote_with_keyword(&mut transaction, &guild_id, &keyword)
          .await?
      {
        let embed = BloomBotEmbed::new().description(format!(
          "{}\n\n\\― {}",
          quote.quote,
          quote.author.unwrap_or("Anonymous".to_string())
        ));
        ctx.send(CreateReply::default().embed(embed)).await?;
        return Ok(());
      }

      ctx
        .send(
          CreateReply::default()
            .content("No quotes found. Fetching random quote.")
            .ephemeral(true),
        )
        .await?;
    }

    ctx
      .send(
        CreateReply::default()
          .content(format!(
            "{} The keyword option is only available to [subscription-based donators]\
            (<https://discord.com/channels/244917432383176705/1030424719138246667/1031137243345211413>).",
            EMOJI.mminfo
          ))
          .ephemeral(true),
      )
      .await?;
  }

  match DatabaseHandler::get_random_quote(&mut transaction, &guild_id).await? {
    None => ctx.say("No quotes found.").await?,
    Some(quote) => {
      let embed = BloomBotEmbed::new().description(format!(
        "{}\n\n\\― {}",
        quote.quote,
        quote.author.unwrap_or("Anonymous".to_string())
      ));
      ctx.send(CreateReply::default().embed(embed)).await?
    }
  };

  Ok(())
}
