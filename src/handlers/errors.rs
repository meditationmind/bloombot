use anyhow::Error;
use poise::{CreateReply, FrameworkError, builtins, serenity_prelude::Channel};
use tracing::error;

use crate::{Data, config::EMOJI};

pub async fn handle(error: FrameworkError<'_, Data, Error>) {
  match error {
    FrameworkError::Command { ctx, error, .. } => {
      if error.to_string() == "Unknown Member" {
        let msg = format!(
          "{} The specified user is not a member of this server.",
          EMOJI.mminfo
        );
        match ctx
          .send(CreateReply::default().content(msg).ephemeral(true))
          .await
        {
          Ok(_) => return,
          Err(e) => error!("While handling error, could not send message: {e}"),
        }
      } else {
        let msg = format!(
          "{} An error occurred. Please try again or contact server staff for assistance.",
          EMOJI.mminfo
        );
        if let Err(e) = ctx
          .send(CreateReply::default().content(msg).ephemeral(true))
          .await
        {
          error!("While handling error, could not send message: {e}");
        }
      }

      let command_name = ctx.command().qualified_name.as_str();
      let channel_id = ctx.channel_id();
      let channel = if let Ok(channel) = channel_id.to_channel(ctx).await {
        Some(channel)
      } else {
        error!("While handling error, could not get channel: {channel_id}");
        None
      };

      // Check whether the error originated from a guild channel or DM.
      let source = channel
        .as_ref()
        .map_or("unknown".to_owned(), |channel| match channel {
          Channel::Guild(_) => {
            let guild_name = ctx
              .guild()
              .map_or("unknown".to_owned(), |guild| guild.name.clone());
            format!("{guild_name} ({})", channel.id())
          }
          Channel::Private(_) => "DM".to_owned(),
          _ => "unknown".to_owned(),
        });

      error!("\x1B[1m/{command_name}\x1B[0m failed with error: {error}");
      error!("\tSource: {source}");
      if let Some(channel) = channel {
        error!("\tChannel: {}", channel.id());
      }
      error!("\tUser: {} ({})", ctx.author().name, ctx.author().id);
    }
    FrameworkError::ArgumentParse {
      error, input, ctx, ..
    } => {
      let response = if let Some(input) = input {
        format!("**Cannot parse `{input}` as argument: {error}**")
      } else {
        format!("**{error}**")
      };
      if let Err(e) = ctx
        .send(CreateReply::default().content(response).ephemeral(true))
        .await
      {
        error!("While handling error, could not send message: {e}");
      }
    }
    FrameworkError::CommandCheckFailed { error, ctx, .. } => {
      error!(
        "A command check failed in command \x1B[1m/{}\x1B[0m for user {}: {}",
        ctx.command().qualified_name,
        ctx.author().name,
        error.map_or(String::from("Conditions not met"), |e| e.to_string()),
      );
    }
    error => {
      if let Err(e) = builtins::on_error(error).await {
        error!("Error while handling error: {e}");
      }
    }
  }
}
