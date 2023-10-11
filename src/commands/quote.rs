use crate::database::DatabaseHandler;
use crate::Context;
use anyhow::Result;
use crate::config::BloomBotEmbed;

/// Gets a motivational quote.
#[poise::command(slash_command, member_cooldown = 1200)]
pub async fn quote(ctx: Context<'_>) -> Result<()> {
  let data = ctx.data();

  // We unwrap here, because we know that the command is guild-only.
  let guild_id = ctx.guild_id().unwrap();

  let mut transaction = data.db.start_transaction().await?;
  match DatabaseHandler::get_random_quote(&mut transaction, &guild_id).await? {
    None => {
      ctx.say("No quotes found.").await?;
    }
    Some(quote) => {
      let embed = BloomBotEmbed::new()
        .description(format!(
          "> {}\n\n\\― {}",
          quote.quote.as_str(),
          quote.author.unwrap_or("Anonymous".to_string())
        ))
        .to_owned();

      ctx
        .send(|f| {
          f.embeds = vec![embed];

          f
        })
        .await?;
    }
  }

  Ok(())
}