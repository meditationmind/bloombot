use crate::config::BloomBotEmbed;
use crate::database::DatabaseHandler;
use crate::Context;
use anyhow::Result;

/// Get a meditation/mindfulness quote
///
/// Get a random meditation/mindfulness quote.
#[poise::command(
  slash_command,
  category = "Informational",
  member_cooldown = 300,
  guild_only
)]
pub async fn quote(ctx: Context<'_>) -> Result<()> {
  let data = ctx.data();

  // We unwrap here, because we know that the command is guild-only.
  let guild_id = ctx.guild_id().unwrap();

  let mut transaction = data.db.start_transaction_with_retry(5).await?;
  match DatabaseHandler::get_random_quote(&mut transaction, &guild_id).await? {
    None => {
      ctx.say("No quotes found.").await?;
    }
    Some(quote) => {
      let embed = BloomBotEmbed::new()
        .description(format!(
          "{}\n\n\\― {}",
          quote.quote.as_str(),
          quote.author.unwrap_or("Anonymous".to_string())
        ))
        .clone();

      ctx
        .send(poise::CreateReply {
          embeds: vec![embed],
          ..Default::default()
        })
        .await?;
    }
  }

  Ok(())
}
