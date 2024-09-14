use crate::config::{BloomBotEmbed, ROLES};
use crate::database::DatabaseHandler;
use crate::Context;
use anyhow::{Context as AnyhowContext, Result};
use poise::serenity_prelude::RoleId;

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
  let data = ctx.data();
  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  if let Some(keyword) = keyword {
    let supporter = {
      if let Some(member) = ctx.author_member().await {
        member.roles.contains(&RoleId::from(ROLES.patreon))
          || member.roles.contains(&RoleId::from(ROLES.kofi))
          || member.roles.contains(&RoleId::from(ROLES.staff))
      } else {
        false
      }
    };

    if supporter {
      match DatabaseHandler::get_random_quote_with_keyword(&mut transaction, &guild_id, &keyword)
        .await?
      {
        None => {
          ctx
            .send(
              poise::CreateReply::default()
                .content("No quotes found. Fetching random quote.")
                .ephemeral(true),
            )
            .await?;

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

          return Ok(());
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

      return Ok(());
    }

    ctx
      .send(
        poise::CreateReply::default()
          .content("<:mminfo:1279517292455264359> The keyword option is only available to [subscription-based donators](<https://discord.com/channels/244917432383176705/1030424719138246667/1031137243345211413>).")
          .ephemeral(true),
      )
      .await?;
  }

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
