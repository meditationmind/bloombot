use std::fmt::Write as _;

use anyhow::Result;
use indexmap::IndexMap;
use poise::serenity_prelude::builder::*;
use poise::{Command, Context as PoiseContext, ContextMenuCommandAction, CreateReply};

use crate::config::{EMOJI, ROLES, SECRET_CATEGORY};
use crate::Context;

struct Help<'a> {
  /// Extra text displayed in the footer of your main help menu.
  footer_text: &'a str,
  /// Whether to make the response ephemeral, if possible.
  ephemeral: bool,
  /// Whether to include context menu commands.
  show_context_menu_commands: bool,
  /// Whether the user should be able to see all commands.
  elevated_permissions: bool,
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
  let is_staff = match ctx.guild_id() {
    Some(guild_id) => ctx.author().has_role(ctx, guild_id, ROLES.staff).await?,
    None => false,
  };
  let footer_text = "For more info about a command or its subcommands, use: /help command";
  let help = Help::new(footer_text, true, true, is_staff, SECRET_CATEGORY);

  match command {
    Some(command) => help.single_command(ctx, command.as_str()).await,
    None => help.all_commands(ctx).await,
  }
}

impl<'a> Help<'a> {
  /// Initializes the help menu configuration.
  fn new(
    footer_text: &'a str,
    ephemeral: bool,
    show_context_menu_commands: bool,
    elevated_permissions: bool,
    secret_category: &'a str,
  ) -> Self {
    Self {
      footer_text,
      ephemeral,
      show_context_menu_commands,
      elevated_permissions,
      secret_category,
    }
  }

  /// Displays the help menu for a single command and any subcommands it may have.
  async fn single_command<U, E>(
    &self,
    ctx: PoiseContext<'_, U, E>,
    command_name: &str,
  ) -> Result<()> {
    let command = ctx.framework().options().commands.iter().find(|command| {
      command.name.eq_ignore_ascii_case(command_name)
        || command
          .context_menu_name
          .as_deref()
          .is_some_and(|name| name.eq_ignore_ascii_case(command_name))
    });

    let command_not_found = format!("{} Command not found: `{command_name}`", EMOJI.mminfo);

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

    let is_secret = command
      .category
      .as_ref()
      .is_some_and(|category| category == self.secret_category);
    let missing_permissions =
      !self.elevated_permissions && !command.required_permissions.is_empty();
    let is_context_menu_command = command.context_menu_action.is_some();
    let disabled_context_menu_command = is_context_menu_command && !self.show_context_menu_commands;

    if is_secret || missing_permissions || disabled_context_menu_command {
      ctx
        .send(
          CreateReply::default()
            .content(command_not_found)
            .ephemeral(self.ephemeral),
        )
        .await?;
      return Ok(());
    }

    let (prefix, command_name) = if is_context_menu_command {
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

    let mut subcommands = IndexMap::<&String, &str>::new();

    let help_text = if command.subcommands.is_empty() {
      help_text
    } else {
      for subcmd in &command.subcommands {
        let subcmd_help = subcmd
          .help_text
          .as_deref()
          .unwrap_or(subcmd.description.as_deref().unwrap_or("No help available"));
        subcommands.insert(&subcmd.name, subcmd_help);
      }
      &format!("{help_text}\n\nSubcommands:")
    };

    let fields = subcommands.into_iter().map(|(subcommand_name, help_text)| {
      let field_name = format!("{prefix}{command_name} {subcommand_name}");
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

  /// Displays the help menu for all commands.
  async fn all_commands<U, E>(&self, ctx: PoiseContext<'_, U, E>) -> Result<()> {
    let mut categories = IndexMap::<Option<&str>, Vec<&Command<U, E>>>::new();
    for cmd in &ctx.framework().options().commands {
      let missing_permissions = !self.elevated_permissions && !cmd.required_permissions.is_empty();
      let is_context_menu_command = cmd.context_menu_action.is_some();
      let not_usable_here = ctx.guild_id().is_none() && cmd.guild_only;
      let is_secret = cmd
        .category
        .as_ref()
        .is_some_and(|category| category == self.secret_category);

      if missing_permissions || is_context_menu_command || is_secret || not_usable_here {
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

    let mut embed = CreateEmbed::new()
      .fields(fields)
      .footer(CreateEmbedFooter::new(self.footer_text));

    if self.show_context_menu_commands {
      let mut context_categories = IndexMap::<Option<&str>, Vec<&Command<U, E>>>::new();
      for cmd in &ctx.framework().options().commands {
        let not_context_menu_command = cmd.context_menu_action.is_none();
        let missing_permissions =
          !self.elevated_permissions && !cmd.required_permissions.is_empty();
        let not_usable_here = ctx.guild_id().is_none() && cmd.guild_only;

        if not_context_menu_command || cmd.hide_in_help || missing_permissions || not_usable_here {
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
        embed = embed.field("Context Menu Commands", category_content, false);
      }
    }

    ctx
      .send(
        CreateReply::default()
          .embed(embed)
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
      let has_permissions = cmd.required_permissions.is_empty() || is_staff;
      let not_secret = cmd
        .category
        .as_ref()
        .is_some_and(|category| category != SECRET_CATEGORY);
      let is_usable_here = (ctx.guild_id().is_some() && cmd.guild_only)
        || (ctx.guild_id().is_none() && !cmd.guild_only);
      let not_context_menu_command = cmd.context_menu_action.is_none();

      has_permissions
        && not_secret
        && is_usable_here
        && not_context_menu_command
        && cmd.name.starts_with(partial)
    })
    .take(25)
    .map(|cmd| AutocompleteChoice::from(&cmd.name))
}
