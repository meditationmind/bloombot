#![warn(clippy::pedantic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::too_many_lines)]

use anyhow::{Context as ErrorContext, Error, Result};
use commands::{
  add, add_bookmark, bookmark, challenge, coffee, community_sit, complete, course, courses,
  customize, erase, erase_message, glossary, hello, help, import, keys, manage, pick_winner, ping,
  quote, quotes, recent, remove_entry, report_message, stats, streak, suggest, terms, uptime,
  whatis,
};
use dotenvy::dotenv;
use log::{error, info};
use poise::serenity_prelude::{self as serenity, model::channel};
use rand::rngs::SmallRng;
use rand::SeedableRng;
use serenity::FullEvent as Event;
use std::sync::Arc;
use tokio::sync::Mutex;

mod charts;
mod commands;
mod config;
mod data;
mod events;
mod handlers;

use handlers::{database, embeddings};

pub struct Data {
  pub db: Arc<database::DatabaseHandler>,
  pub rng: Arc<Mutex<SmallRng>>,
  pub embeddings: Arc<embeddings::OpenAIHandler>,
  pub bloom_start_time: std::time::Instant,
}
pub type Context<'a> = poise::Context<'a, Data, Error>;

#[tokio::main]
async fn main() -> Result<()> {
  dotenv().ok();

  pretty_env_logger::init();

  let token =
    std::env::var("DISCORD_TOKEN").with_context(|| "Missing DISCORD_TOKEN environment variable")?;
  let test_guild = std::env::var("TEST_GUILD_ID");

  let intents = serenity::GatewayIntents::GUILDS
    | serenity::GatewayIntents::GUILD_MODERATION
    | serenity::GatewayIntents::GUILD_MESSAGES
    | serenity::GatewayIntents::GUILD_MESSAGE_REACTIONS
    | serenity::GatewayIntents::DIRECT_MESSAGES
    | serenity::GatewayIntents::GUILD_MEMBERS;

  let framework = poise::Framework::builder()
    .options(poise::FrameworkOptions {
      commands: vec![
        keys(),
        courses(),
        pick_winner(),
        erase(),
        manage(),
        quotes(),
        terms(),
        challenge(),
        customize(),
        add(),
        import(),
        recent(),
        remove_entry(),
        stats(),
        streak(),
        whatis(),
        glossary(),
        bookmark(),
        quote(),
        coffee(),
        hello(),
        help(),
        ping(),
        uptime(),
        course(),
        suggest(),
        complete(),
        add_bookmark(),
        erase_message(),
        report_message(),
        community_sit(),
      ],
      event_handler: |ctx, event, _framework, data| Box::pin(event_handler(ctx, event, data)),
      on_error: |error| {
        Box::pin(async move {
          error_handler(error).await;
        })
      },
      ..Default::default()
    })
    .setup(|ctx, _ready, framework| {
      Box::pin(async move {
        if let Ok(test_guild) = test_guild {
          info!("Registering commands in test guild {test_guild}");

          let guild_id = serenity::GuildId::new(test_guild.parse::<u64>()?);
          poise::builtins::register_in_guild(ctx, &framework.options().commands, guild_id).await?;
        } else {
          info!("Registering commands globally");
          poise::builtins::register_globally(ctx, &framework.options().commands).await?;
        }
        Ok(Data {
          db: Arc::new(database::DatabaseHandler::new().await?),
          rng: Arc::new(Mutex::new(SmallRng::from_entropy())),
          embeddings: Arc::new(embeddings::OpenAIHandler::new()?),
          bloom_start_time: std::time::Instant::now(),
        })
      })
    })
    .build();

  let mut client = serenity::Client::builder(&token, intents)
    .framework(framework)
    .await
    .map_err(|e| anyhow::anyhow!(e))?;

  let shard_manager = client.shard_manager.clone();

  tokio::spawn(async move {
    wait_until_shutdown().await;

    info!("Received shutdown request. Until next time!");
    shard_manager.shutdown_all().await;
  });

  client
    .start()
    .await
    .map_err(|e| anyhow::anyhow!("Error starting client: {e}"))
}

async fn error_handler(error: poise::FrameworkError<'_, Data, Error>) {
  match error {
    poise::FrameworkError::Command { ctx, error, .. } => {
      match ctx.say("An error occurred while running the command").await {
        Ok(_) => {}
        Err(e) => {
          error!("While handling error, could not send message: {e}");
        }
      };

      let command = ctx.command();
      let channel_id = ctx.channel_id();
      let channel = if let Ok(channel) = channel_id.to_channel(ctx).await {
        Some(channel)
      } else {
        error!("While handling error, could not get channel {channel_id}");
        None
      };

      // Whether it's a guild or DM channel
      let source = match &channel {
        Some(channel) => match channel {
          channel::Channel::Guild(_) => {
            let guild_name = match ctx.guild() {
              Some(guild) => guild.name.clone(),
              None => "unknown".to_owned(),
            };
            format!("{} ({})", guild_name, channel.id())
          }
          channel::Channel::Private(_) => "DM".to_owned(),
          _ => "unknown".to_owned(),
        },
        None => "unknown".to_owned(),
      };
      let user = ctx.author();

      error!(
        "\x1B[1m/{}\x1B[0m failed with error: {:?}",
        command.name, error
      );
      error!("\tSource: {source}");

      if let Some(channel) = channel {
        error!("\tChannel: {}", channel.id());
      }

      error!("\tUser: {} ({})", user.name, user.id);
    }
    poise::FrameworkError::ArgumentParse {
      error, input, ctx, ..
    } => {
      let response = if let Some(input) = input {
        format!("**Cannot parse `{input}` as argument: {error}**")
      } else {
        format!("**{error}**")
      };

      match ctx
        .send(
          poise::CreateReply::default()
            .content(response)
            .ephemeral(true),
        )
        .await
      {
        Ok(_) => {}
        Err(e) => {
          error!("While handling error, could not send message: {e}");
        }
      };
    }
    error => {
      if let Err(e) = poise::builtins::on_error(error).await {
        error!("Error while handling error: {e}");
      }
    }
  }
}

async fn event_handler(ctx: &serenity::Context, event: &Event, data: &Data) -> Result<(), Error> {
  let database = &data.db;

  match event {
    Event::GuildCreate { guild, .. } => {
      events::guild_create(ctx, database, &guild.id).await?;
    }
    Event::GuildMemberRemoval { user, .. } => {
      events::guild_member_removal(ctx, user).await?;
    }
    Event::GuildMemberUpdate {
      old_if_available,
      new,
      ..
    } => {
      events::guild_member_update(ctx, old_if_available, new).await?;
    }
    Event::MessageDelete {
      deleted_message_id, ..
    } => {
      events::message_delete(database, deleted_message_id).await?;
    }
    Event::ReactionAdd { add_reaction } => {
      events::reaction_add(ctx, database, add_reaction).await?;
    }
    Event::ReactionRemove { removed_reaction } => {
      events::reaction_remove(ctx, database, removed_reaction).await?;
    }
    Event::Ready { .. } => {
      info!("Connected!");

      let default_activity_text = "Tracking your meditations";
      info!(
        "Setting default activity text: \"{}\"",
        default_activity_text
      );
      ctx.set_activity(Some(serenity::ActivityData::custom(default_activity_text)));
    }
    _ => {}
  }
  Ok(())
}

#[allow(clippy::unwrap_used)]
#[cfg(unix)]
async fn wait_until_shutdown() {
  use tokio::signal::unix as signal;

  let [mut s1, mut s2, mut s3] = [
    signal::signal(signal::SignalKind::hangup()).unwrap(),
    signal::signal(signal::SignalKind::interrupt()).unwrap(),
    signal::signal(signal::SignalKind::terminate()).unwrap(),
  ];

  tokio::select!(
      v = s1.recv() => v.unwrap(),
      v = s2.recv() => v.unwrap(),
      v = s3.recv() => v.unwrap(),
  );
}

#[allow(clippy::unwrap_used)]
#[cfg(windows)]
async fn wait_until_shutdown() {
  let (mut s1, mut s2) = (
    tokio::signal::windows::ctrl_c().unwrap(),
    tokio::signal::windows::ctrl_break().unwrap(),
  );

  tokio::select!(
      v = s1.recv() => v.unwrap(),
      v = s2.recv() => v.unwrap(),
  );
}
