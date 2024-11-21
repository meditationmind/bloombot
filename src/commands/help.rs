use std::fmt::Write as _;

use anyhow::Result;
use indexmap::IndexMap;
use poise::serenity_prelude::builder::*;
use poise::{Command, Context as PoiseContext, ContextMenuCommandAction, CreateReply};

use crate::config::{ROLES, SECRET_CATEGORY};
use crate::Context;

struct Help<'a> {
  /// Extra text displayed in the footer of your main help menu.
  footer_text: &'a str,
  /// Whether to make the response ephemeral, if possible.
  ephemeral: bool,
  /// Whether to include context menu commands.
  show_context_menu_commands: bool,
  /// Optionally specify a secret category to exclude from help.
  secret_category: &'a str,
}

/// Show the help menu
///
/// Shows the help menu.
#[poise::command(slash_command, category = "Utilities")]
pub async fn help(
  ctx: Context<'_>,
  #[description = "Specific command to show help about"]
  #[autocomplete = "autocomplete_command"]
  command: Option<String>,
) -> Result<()> {
  // Determine who should see all available commands.
  let staff = match ctx.guild_id() {
    Some(guild_id) => ctx.author().has_role(ctx, guild_id, ROLES.staff).await?,
    None => false,
  };

  let footer_text = "For more info about a command or its subcommands, use: /help command";
  let help = Help::with_config(footer_text, true, true, SECRET_CATEGORY);

  match command {
    Some(command) => help.single_command(ctx, command.as_str(), staff).await,
    None => help.all_commands(ctx, staff).await,
  }
}

impl<'a> Help<'a> {
  fn with_config(
    footer_text: &'a str,
    ephemeral: bool,
    show_context_menu_commands: bool,
    secret_category: &'a str,
  ) -> Self {
    Self {
      footer_text,
      ephemeral,
      show_context_menu_commands,
      secret_category,
    }
  }

  async fn single_command<U, E>(
    &self,
    ctx: PoiseContext<'_, U, E>,
    command_name: &str,
    elevated_permissions: bool,
  ) -> Result<()> {
    let command = ctx.framework().options().commands.iter().find(|command| {
      command.name.eq_ignore_ascii_case(command_name)
        || command
          .context_menu_name
          .as_deref()
          .is_some_and(|name| name.eq_ignore_ascii_case(command_name))
    });

    let command_not_found = format!("Command not found: `{command_name}`");

    let Some(command) = command else {
      ctx
        .send(
          CreateReply::default()
            .content(command_not_found)
            .ephemeral(self.ephemeral),
        )
        .await?;
      return Ok(());
    };

    if command
      .category
      .as_ref()
      .is_some_and(|category| category == self.secret_category)
      || (command.context_menu_action.is_some() && !self.show_context_menu_commands)
      || (!elevated_permissions && !command.required_permissions.is_empty())
    {
      ctx
        .send(
          CreateReply::default()
            .content(command_not_found)
            .ephemeral(self.ephemeral),
        )
        .await?;
      return Ok(());
    }

    let (prefix, command_name) = if command.context_menu_action.is_some() {
      (
        String::new(),
        command
          .context_menu_name
          .as_deref()
          .map_or(command.name.as_str(), |name| name),
      )
    } else {
      (String::from("/"), command.name.as_str())
    };

    let help_text = command.help_text.as_deref().unwrap_or(
      command
        .description
        .as_deref()
        .unwrap_or("No help available"),
    );

    let mut subcommands = IndexMap::<&String, String>::new();

    let help_text = if command.subcommands.is_empty() {
      help_text
    } else {
      for subcmd in &command.subcommands {
        let subcmd_help = match subcmd.help_text.as_deref() {
          Some(f) => f.to_owned(),
          None => subcmd
            .description
            .as_deref()
            .unwrap_or("No help available")
            .to_owned(),
        };
        subcommands.insert(&subcmd.name, subcmd_help);
      }
      &format!("{help_text}\n\nSubcommands:")
    };

    let fields = subcommands.into_iter().map(|(subcommand_name, help_text)| {
      let field_name = format!("{prefix}{} {subcommand_name}", command.name);
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
          .ephemeral(self.ephemeral),
      )
      .await?;

    Ok(())
  }

  async fn all_commands<U, E>(
    &self,
    ctx: PoiseContext<'_, U, E>,
    elevated_permissions: bool,
  ) -> Result<()> {
    let mut categories = IndexMap::<Option<&str>, Vec<&Command<U, E>>>::new();
    for cmd in &ctx.framework().options().commands {
      if !elevated_permissions && !cmd.required_permissions.is_empty()
        || cmd.context_menu_action.is_some()
        || cmd
          .category
          .as_ref()
          .is_some_and(|category| category == self.secret_category)
        || ctx.guild_id().is_none() && cmd.guild_only
      {
        continue;
      }
      categories
        .entry(cmd.category.as_deref())
        .or_default()
        .push(cmd);
    }

    let fields = categories
      .into_iter()
      .filter(|(_, commands)| !commands.is_empty())
      .map(|(category_name, commands)| {
        let mut category_content = String::from("```");
        for command in commands {
          if command.hide_in_help || ctx.guild_id().is_none() && command.guild_only {
            continue;
          }
          let prefix = String::from("/");
          let total_command_name_length = prefix.chars().count() + command.name.chars().count();
          let padding = 12_usize.saturating_sub(total_command_name_length) + 1;
          let _ = writeln!(
            category_content,
            "{prefix}{}{}{}",
            command.name,
            " ".repeat(padding),
            command.description.as_deref().unwrap_or("")
          );
        }
        category_content += "```";

        (category_name.unwrap_or("Other"), category_content, false)
      });

    if self.show_context_menu_commands {
      let mut context_categories = IndexMap::<Option<&str>, Vec<&Command<U, E>>>::new();
      for cmd in &ctx.framework().options().commands {
        if cmd.context_menu_action.is_none()
          || cmd.hide_in_help
          || (ctx.guild_id().is_none() && cmd.guild_only)
        {
          continue;
        }
        context_categories
          .entry(cmd.category.as_deref())
          .or_default()
          .push(cmd);
      }

      let mut category_content = String::from("```");

      for (_, commands) in context_categories {
        for command in commands {
          let kind = match command.context_menu_action {
            Some(ContextMenuCommandAction::User(_)) => "user",
            Some(ContextMenuCommandAction::Message(_)) => "message",
            _ => continue,
          };
          let name = command
            .context_menu_name
            .as_deref()
            .map_or(command.name.as_str(), |name| name);
          let _ = writeln!(
            category_content,
            "{name} (on {kind})\n>> {}",
            command.description.as_deref().unwrap_or("")
          );
        }
      }

      category_content += "```";

      if category_content != "``````" {
        ctx
          .send(
            CreateReply::default()
              .embed(
                CreateEmbed::new()
                  .fields(fields)
                  .field("Context Menu Commands", category_content, false)
                  .footer(CreateEmbedFooter::new(self.footer_text)),
              )
              .ephemeral(self.ephemeral),
          )
          .await?;
        return Ok(());
      };
    }

    ctx
      .send(
        CreateReply::default()
          .embed(
            CreateEmbed::new()
              .fields(fields)
              .footer(CreateEmbedFooter::new(self.footer_text)),
          )
          .ephemeral(self.ephemeral),
      )
      .await?;

    Ok(())
  }
}

pub async fn autocomplete_command<'a>(
  ctx: Context<'a>,
  partial: &'a str,
) -> impl Iterator<Item = AutocompleteChoice> + 'a {
  let is_staff = match ctx.guild_id() {
    Some(guild_id) => ctx
      .author()
      .has_role(ctx, guild_id, ROLES.staff)
      .await
      .unwrap_or_default(),
    None => false,
  };

  ctx
    .framework()
    .options()
    .commands
    .iter()
    .filter(move |cmd| {
      (cmd.required_permissions.is_empty() || is_staff)
        && cmd.context_menu_action.is_none()
        && cmd
          .category
          .as_ref()
          .is_some_and(|category| category != SECRET_CATEGORY)
        && ((ctx.guild_id().is_some() && cmd.guild_only)
          || (ctx.guild_id().is_none() && !cmd.guild_only))
        && cmd.name.starts_with(partial)
    })
    .take(25)
    .map(|cmd| AutocompleteChoice::from(&cmd.name))
}
