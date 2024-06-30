use crate::config::ROLES;
use crate::Context;
use anyhow::Result;
use poise::{
  serenity_prelude::{self as serenity, builder::*},
  CreateReply,
};
use std::fmt::Write as _;

pub struct HelpConfiguration<'a> {
  /// Extra text displayed at the bottom of your message. Can be used for help and tips specific to your bot.
  pub extra_text_at_bottom: &'a str,
  /// Whether to make the response ephemeral if possible. Can be nice to reduce clutter.
  pub ephemeral: bool,
  /// Whether to list context menu commands as well.
  pub show_context_menu_commands: bool,
  /// Optionally specify a secret category to completely prevent from being accessible via the command.
  pub secret_category: &'a str,
}

impl Default for HelpConfiguration<'_> {
  fn default() -> Self {
    Self {
      extra_text_at_bottom: "",
      ephemeral: true,
      show_context_menu_commands: false,
      secret_category: "",
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OrderedMap<K, V>(pub Vec<(K, V)>);

impl<K, V> Default for OrderedMap<K, V> {
  fn default() -> Self {
    Self(Vec::default())
  }
}

impl<K: Eq, V> OrderedMap<K, V> {
  /// Creates a new [`OrderedMap`]
  pub fn new() -> Self {
    Self(Vec::new())
  }

  /// Finds a value in the map by the given key
  #[allow(dead_code)]
  pub fn get(&self, k: &K) -> Option<&V> {
    self
      .0
      .iter()
      .find(|entry| &entry.0 == k)
      .map(|entry| &entry.1)
  }

  /// Inserts a key value pair into the map
  #[allow(dead_code)]
  pub fn insert(&mut self, k: K, v: V) {
    match self.0.iter_mut().find(|entry| entry.0 == k) {
      Some(entry) => entry.1 = v,
      None => self.0.push((k, v)),
    }
  }

  /// Finds a value in the map by the given key, or inserts it if it doesn't exist
  pub fn get_or_insert_with(&mut self, k: K, v: impl FnOnce() -> V) -> &mut V {
    if let Some(i) = self.0.iter().position(|entry| entry.0 == k) {
      &mut self.0[i].1
    } else {
      self.0.push((k, v()));
      &mut self.0.last_mut().expect("we just inserted").1
    }
  }
}

impl<K, V> IntoIterator for OrderedMap<K, V> {
  type Item = (K, V);
  type IntoIter = std::vec::IntoIter<(K, V)>;
  fn into_iter(self) -> Self::IntoIter {
    self.0.into_iter()
  }
}

async fn help_single_command<U, E>(
  ctx: poise::Context<'_, U, E>,
  command_name: &str,
  config: HelpConfiguration<'_>,
  elevated_permissions: bool,
) -> Result<(), serenity::Error> {
  let command = ctx.framework().options().commands.iter().find(|command| {
    if command.name.eq_ignore_ascii_case(command_name) {
      return true;
    }
    if let Some(context_menu_name) = &command.context_menu_name {
      if context_menu_name.eq_ignore_ascii_case(command_name) {
        return true;
      }
    }
    false
  });

  let command_not_found = format!("Command not found: `{command_name}`");

  if command.is_none() {
    ctx
      .send(
        CreateReply::default()
          .content(command_not_found)
          .ephemeral(config.ephemeral),
      )
      .await?;
    return Ok(());
  }

  let command = command.unwrap();

  if command.category.clone().unwrap_or_default() == config.secret_category
    || (command.context_menu_action.is_some() && !config.show_context_menu_commands)
    || (!elevated_permissions && !command.required_permissions.is_empty())
  {
    ctx
      .send(
        CreateReply::default()
          .content(command_not_found)
          .ephemeral(config.ephemeral),
      )
      .await?;
    return Ok(());
  }

  let prefix = if command.context_menu_action.is_some() {
    String::new()
  } else {
    String::from("/")
  };

  let command_name = if command.context_menu_action.is_some() {
    command
      .context_menu_name
      .clone()
      .unwrap_or(command.name.clone())
      .to_string()
  } else {
    command.name.clone()
  };

  let mut help_text = match command.help_text.clone() {
    Some(f) => f,
    None => command
      .description
      .as_deref()
      .unwrap_or("No help available")
      .to_owned(),
  };

  let mut subcommands = OrderedMap::<&String, String>::new();

  if !command.subcommands.is_empty() {
    help_text += "\n\nSubcommands:";
    for subcmd in &command.subcommands {
      let subcmd_help = match subcmd.help_text.clone() {
        Some(f) => f,
        None => subcmd
          .description
          .as_deref()
          .unwrap_or("No help available")
          .to_owned(),
      };
      subcommands.insert(&subcmd.name, subcmd_help);
    }
  }

  let fields = subcommands.into_iter().map(|(subcommand_name, help_text)| {
    let field_name = format!("{}{} {}", prefix, command.name, subcommand_name);
    let field_text = format!("```{help_text}```");
    (field_name, field_text, false)
  });

  ctx
    .send(
      CreateReply::default()
        .embed(
          CreateEmbed::new()
            .title(format!("{prefix}{command_name}"))
            .description(help_text)
            .fields(fields),
        )
        .ephemeral(config.ephemeral),
    )
    .await?;

  Ok(())
}

async fn help_all_commands<U, E>(
  ctx: poise::Context<'_, U, E>,
  config: HelpConfiguration<'_>,
  elevated_permissions: bool,
) -> Result<(), serenity::Error> {
  let mut categories = OrderedMap::<Option<&str>, Vec<&poise::Command<U, E>>>::new();
  for cmd in &ctx.framework().options().commands {
    if !elevated_permissions && !cmd.required_permissions.is_empty() {
      continue;
    }
    if cmd.category.clone().unwrap_or_default() == config.secret_category {
      continue;
    }
    if cmd.context_menu_action.is_some() {
      continue;
    }
    categories
      .get_or_insert_with(cmd.category.as_deref(), Vec::new)
      .push(cmd);
  }

  let fields = categories
    .into_iter()
    .filter(|(_, commands)| !commands.is_empty())
    .map(|(category_name, commands)| {
      let mut category_content = String::from("```");
      for command in commands {
        if command.hide_in_help {
          continue;
        }

        let prefix = String::from("/");
        let total_command_name_length = prefix.chars().count() + command.name.chars().count();
        let padding = 12_usize.saturating_sub(total_command_name_length) + 1;
        let _ = writeln!(
          category_content,
          "{}{}{}{}",
          prefix,
          command.name,
          " ".repeat(padding),
          command.description.as_deref().unwrap_or("")
        );
      }

      category_content += "```";

      (category_name.unwrap_or("Other"), category_content, false)
    });

  if config.show_context_menu_commands {
    let mut context_categories = OrderedMap::<Option<&str>, Vec<&poise::Command<U, E>>>::new();
    for cmd in &ctx.framework().options().commands {
      if cmd.context_menu_action.is_none() || cmd.hide_in_help {
        continue;
      }
      context_categories
        .get_or_insert_with(cmd.category.as_deref(), Vec::new)
        .push(cmd);
    }

    let mut category_content = String::from("```");

    for (_, commands) in context_categories {
      for command in commands {
        let kind = match command.context_menu_action {
          Some(poise::ContextMenuCommandAction::User(_)) => "user",
          Some(poise::ContextMenuCommandAction::Message(_)) => "message",
          _ => continue,
        };
        let name = command
          .context_menu_name
          .clone()
          .unwrap_or(command.name.clone());
        let _ = writeln!(
          category_content,
          "{} (on {})\n>> {}",
          name,
          kind,
          command.description.as_deref().unwrap_or("")
        );
      }
    }

    category_content += "```";

    if category_content == "``````" {
      ctx
        .send(
          CreateReply::default()
            .embed(
              CreateEmbed::new()
                .fields(fields)
                .footer(CreateEmbedFooter::new(
                  config.extra_text_at_bottom.to_string(),
                )),
            )
            .ephemeral(config.ephemeral),
        )
        .await?;
    } else {
      ctx
        .send(
          CreateReply::default()
            .embed(
              CreateEmbed::new()
                .fields(fields)
                .field("Context Menu Commands", category_content, false)
                .footer(CreateEmbedFooter::new(
                  config.extra_text_at_bottom.to_string(),
                )),
            )
            .ephemeral(config.ephemeral),
        )
        .await?;
    };
  } else {
    ctx
      .send(
        CreateReply::default()
          .embed(
            CreateEmbed::new()
              .fields(fields)
              .footer(CreateEmbedFooter::new(
                config.extra_text_at_bottom.to_string(),
              )),
          )
          .ephemeral(config.ephemeral),
      )
      .await?;
  };

  Ok(())
}

pub async fn help_menu<U, E>(
  ctx: poise::Context<'_, U, E>,
  command: Option<&str>,
  config: HelpConfiguration<'_>,
  elevated_permissions: bool,
) -> Result<(), serenity::Error> {
  match command {
    Some(command) => help_single_command(ctx, command, config, elevated_permissions).await,
    None => help_all_commands(ctx, config, elevated_permissions).await,
  }
}

/// Show the help menu
///
/// Shows the help menu.
#[poise::command(slash_command, category = "Utilities")]
pub async fn help(
  ctx: Context<'_>,
  #[description = "Specific command to show help about"]
  // Disabling autocomplete until menu is displayed dynamically based on permissions.
  // #[autocomplete = "poise::builtins::autocomplete_command"]
  command: Option<String>,
) -> Result<()> {
  //Determine who should see all available commands
  let elevated_permissions = match ctx.guild_id() {
    Some(guild_id) => ctx.author().has_role(ctx, guild_id, ROLES.staff).await?,
    None => false,
  };

  help_menu(
    ctx,
    command.as_deref(),
    HelpConfiguration {
      ephemeral: true,
      secret_category: "Secret",
      show_context_menu_commands: true,
      extra_text_at_bottom: "For more info about a command or its subcommands, use: /help command",
    },
    elevated_permissions,
  )
  .await?;

  Ok(())
}
