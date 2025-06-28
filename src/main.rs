#![warn(clippy::pedantic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::too_many_lines)]

#[macro_use(query)]
extern crate sqlx;

use std::env;

use anyhow::{Context as ErrorContext, Error, Result};
use dotenvy::dotenv;
use poise::serenity_prelude::{Client, GatewayIntents, GuildId};
use poise::{Framework, FrameworkOptions, builtins};
use tracing::info;

use crate::commands::{
  add, add_bookmark, bookmark, challenge, coffee, community_sit, complete, course, courses,
  customize, erase, erase_message, extract, extract_text, glossary, hello, help, import, keys,
  mahabharata, manage, pick_winner, ping, quote, quotes, recent, remove_entry, report_message,
  stats, streak, suggest, sutta, terms, uptime, whatis,
};
use crate::config::MEDITATION_MIND;
use crate::data::bloom::{Context, Data, MinimalCommand};
use crate::data::term::Term;
use crate::database::DatabaseHandler;
use crate::handlers::{database, errors, events as Events};

mod charts;
mod commands;
mod config;
mod data;
mod events;
mod handlers;
mod images;

#[tokio::main]
async fn main() -> Result<()> {
  dotenv().ok();

  tracing_subscriber::fmt::init();

  let token =
    env::var("DISCORD_TOKEN").with_context(|| "Missing DISCORD_TOKEN environment variable")?;
  let test_guild = env::var("TEST_GUILD_ID");

  let intents = GatewayIntents::GUILDS
    | GatewayIntents::GUILD_MODERATION
    | GatewayIntents::GUILD_MESSAGES
    | GatewayIntents::GUILD_MESSAGE_REACTIONS
    | GatewayIntents::DIRECT_MESSAGES
    | GatewayIntents::GUILD_MEMBERS
    | GatewayIntents::GUILD_VOICE_STATES;

  let framework = Framework::builder()
    .options(FrameworkOptions {
      commands: vec![
        keys(),
        courses(),
        pick_winner(),
        erase(),
        extract(),
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
        sutta(),
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
        extract_text(),
        report_message(),
        community_sit(),
        mahabharata(),
      ],
      event_handler: |ctx, event, _framework, data| Box::pin(Events::listen(ctx, event, data)),
      on_error: |error| Box::pin(async move { errors::handle(error).await }),
      ..Default::default()
    })
    .setup(|ctx, _ready, framework| {
      Box::pin(async move {
        let commands = if let Ok(test_guild) = test_guild {
          info!("Registering commands in test guild {test_guild}");
          let guild_id = GuildId::new(test_guild.parse::<u64>()?);
          builtins::register_in_guild(ctx, &framework.options().commands, guild_id).await?;
          ctx.http.as_ref().get_guild_commands(guild_id).await?
        } else {
          info!("Registering commands globally");
          builtins::register_globally(ctx, &framework.options().commands).await?;
          ctx.http.as_ref().get_global_commands().await?
        };

        let db = DatabaseHandler::new().await?;
        let term_names = if let Ok(mut transaction) = db.start_transaction_with_retry(5).await {
          let terms = DatabaseHandler::get_term_list(&mut transaction, &MEDITATION_MIND)
            .await
            .unwrap_or_else(|_| vec![Term::default()]);
          Term::names_and_aliases(terms)
        } else {
          vec![String::new()]
        };
        let commands: Vec<MinimalCommand> = commands
          .iter()
          .map(|c| MinimalCommand {
            name: c.name.clone(),
            id: c.id.get(),
          })
          .collect();

        Data::new(db, term_names, commands)
      })
    })
    .build();

  let mut client = Client::builder(&token, intents)
    .framework(framework)
    .await?;

  let shard_manager = client.shard_manager.clone();

  tokio::spawn(async move {
    wait_until_shutdown().await;

    info!("Received shutdown request. Until next time!");
    shard_manager.shutdown_all().await;
  });

  client.start().await.map_err(Into::into)
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
