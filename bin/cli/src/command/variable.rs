use std::process::Command;

use anyhow::{Context, anyhow};
use colored::Colorize;
use comfy_table::{Attribute, Cell, Color, Table};
use komodo_client::{
  api::{
    read::{GetVariable, ListVariables},
    write::{CreateVariable, DeleteVariable},
  },
  entities::config::cli::{
    CliTableBorders,
    args::{
      CliFormat,
      variable::{Variable, VariableCommand},
    },
  },
};

use crate::config::cli_config;

/// Check if showing secrets is allowed via environment variable.
/// Returns true if KM_SHOW_SECRETS=true is set.
fn secrets_display_allowed() -> bool {
  std::env::var("KM_SHOW_SECRETS")
    .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
    .unwrap_or(false)
}

fn should_mask_secret_value(
  is_secret: bool,
  value: &str,
  show_secrets: bool,
) -> bool {
  is_secret && !value.is_empty() && !show_secrets
}

pub async fn handle(variable: &Variable) -> anyhow::Result<()> {
  match &variable.command {
    VariableCommand::List { format } => list_variables(*format).await,
    VariableCommand::Get { name } => get_variable(name).await,
    VariableCommand::Create {
      name,
      value,
      from_command,
      secret,
      description,
      yes,
    } => {
      create_variable(
        name,
        value.as_deref(),
        from_command.as_deref(),
        *secret,
        description,
        *yes,
      )
      .await
    }
    VariableCommand::Delete { name, yes } => {
      delete_variable(name, *yes).await
    }
  }
}

async fn list_variables(format: CliFormat) -> anyhow::Result<()> {
  let client = super::komodo_client().await?;
  let show_secrets = secrets_display_allowed();

  let variables = client
    .read(ListVariables {})
    .await
    .context("Failed to list variables")?;

  if variables.is_empty() {
    println!("{}: No variables found", "INFO".green());
    return Ok(());
  }

  match format {
    CliFormat::Table => {
      let preset = {
        use comfy_table::presets::*;
        match cli_config().table_borders {
          None | Some(CliTableBorders::Horizontal) => {
            UTF8_HORIZONTAL_ONLY
          }
          Some(CliTableBorders::Vertical) => UTF8_FULL_CONDENSED,
          Some(CliTableBorders::Inside) => UTF8_NO_BORDERS,
          Some(CliTableBorders::Outside) => UTF8_BORDERS_ONLY,
          Some(CliTableBorders::All) => UTF8_FULL,
        }
      };

      let mut table = Table::new();
      table.load_style(preset).set_header(
        ["Name", "Value", "Secret", "Description"]
          .map(|h| Cell::new(h).add_attribute(Attribute::Bold)),
      );

      for var in variables {
        let value_cell = if should_mask_secret_value(
          var.is_secret,
          &var.value,
          show_secrets,
        ) {
          Cell::new("********").fg(Color::DarkYellow)
        } else {
          Cell::new(&var.value)
        };

        let secret_cell = if var.is_secret {
          Cell::new("Yes").fg(Color::DarkYellow)
        } else {
          Cell::new("No")
        };

        table.add_row([
          Cell::new(&var.name).add_attribute(Attribute::Bold),
          value_cell,
          secret_cell,
          Cell::new(&var.description),
        ]);
      }

      println!("{table}");
    }
    CliFormat::Json => {
      let masked: Vec<_> = variables
        .into_iter()
        .map(|mut var| {
          if should_mask_secret_value(
            var.is_secret,
            &var.value,
            show_secrets,
          ) {
            var.value = "********".to_string();
          }
          var
        })
        .collect();
      println!(
        "{}",
        serde_json::to_string_pretty(&masked)
          .context("Failed to serialize variables to JSON")?
      );
    }
  }

  Ok(())
}

async fn get_variable(name: &str) -> anyhow::Result<()> {
  let client = super::komodo_client().await?;
  let show_secrets = secrets_display_allowed();

  let variable = client
    .read(GetVariable {
      name: name.to_string(),
    })
    .await
    .context("Failed to get variable")?;

  // For secret variables, require explicit opt-in via environment variable
  // Don't reveal the mechanism in the error message for security
  if should_mask_secret_value(
    variable.is_secret,
    &variable.value,
    show_secrets,
  ) {
    println!("\n{}: {}", "Name".dimmed(), variable.name.bold());
    println!("{}: {}", "Value".dimmed(), "********".yellow());
    println!("{}: {}", "Secret".dimmed(), "Yes".yellow());
    if !variable.description.is_empty() {
      println!(
        "{}: {}",
        "Description".dimmed(),
        variable.description
      );
    }
    println!(
      "\n{}: Secret variable values are hidden for security.",
      "INFO".blue()
    );
    return Ok(());
  }

  println!("\n{}: {}", "Name".dimmed(), variable.name.bold());
  println!("{}: {}", "Value".dimmed(), variable.value);
  if variable.is_secret {
    println!("{}: {}", "Secret".dimmed(), "Yes".yellow());
  }
  if !variable.description.is_empty() {
    println!("{}: {}", "Description".dimmed(), variable.description);
  }

  Ok(())
}

/// Execute a shell command and return its stdout output.
/// Handles special characters properly by using shell execution.
fn execute_command(cmd: &str) -> anyhow::Result<String> {
  let output = if cfg!(target_os = "windows") {
    Command::new("cmd").args(["/C", cmd]).output()
  } else {
    Command::new("sh").args(["-c", cmd]).output()
  }
  .context("Failed to execute command")?;

  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    return Err(anyhow!(
      "Command failed with exit code {}: {}",
      output.status.code().unwrap_or(-1),
      stderr.trim()
    ));
  }

  let stdout = String::from_utf8(output.stdout)
    .context("Command output contains invalid UTF-8")?;

  // Trim trailing newline that most commands add
  Ok(stdout.trim_end_matches('\n').to_string())
}

async fn create_variable(
  name: &str,
  value: Option<&str>,
  from_command: Option<&str>,
  secret: bool,
  description: &str,
  yes: bool,
) -> anyhow::Result<()> {
  // Get the value either from direct input or by executing a command
  let final_value = if let Some(cmd) = from_command {
    println!(
      "\n{}: Executing command to get value...",
      "INFO".blue()
    );
    execute_command(cmd)?
  } else {
    value
      .ok_or_else(|| {
        anyhow!("Either value or --from-command must be provided")
      })?
      .to_string()
  };

  println!("\n{}: Create Variable\n", "Mode".dimmed());
  println!(" - {}:  {name}", "Name".dimmed());

  // Don't show the actual value if it's a secret
  if secret && !final_value.is_empty() {
    println!(" - {}: {}", "Value".dimmed(), "********".yellow());
  } else {
    println!(" - {}: {final_value}", "Value".dimmed());
  }

  println!(" - {}: {secret}", "Is Secret".dimmed());
  if !description.is_empty() {
    println!(" - {}: {description}", "Description".dimmed());
  }

  crate::command::wait_for_enter("create variable", yes)?;

  let client = super::komodo_client().await?;

  let created = client
    .write(CreateVariable {
      name: name.to_string(),
      value: final_value,
      is_secret: secret,
      description: description.to_string(),
    })
    .await
    .context("Failed to create variable")?;

  println!(
    "\n{}: Variable '{}' created successfully ✅",
    "SUCCESS".green(),
    created.name.bold()
  );

  Ok(())
}

async fn delete_variable(
  name: &str,
  yes: bool,
) -> anyhow::Result<()> {
  println!("\n{}: Delete Variable\n", "Mode".dimmed());
  println!(" - {}:  {name}", "Name".dimmed());

  crate::command::wait_for_enter("delete variable", yes)?;

  let client = super::komodo_client().await?;

  let deleted = client
    .write(DeleteVariable {
      name: name.to_string(),
    })
    .await
    .context("Failed to delete variable")?;

  println!(
    "\n{}: Variable '{}' deleted successfully ✅",
    "SUCCESS".green(),
    deleted.name.bold()
  );

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::should_mask_secret_value;

  #[test]
  fn masks_non_empty_secret_when_hidden() {
    assert!(should_mask_secret_value(true, "value", false));
  }

  #[test]
  fn does_not_mask_empty_secret_when_hidden() {
    assert!(!should_mask_secret_value(true, "", false));
  }

  #[test]
  fn does_not_mask_secret_when_allowed() {
    assert!(!should_mask_secret_value(true, "value", true));
  }

  #[test]
  fn does_not_mask_non_secret() {
    assert!(!should_mask_secret_value(false, "value", false));
  }
}
