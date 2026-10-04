use anyhow::Context;
use colored::Colorize;
use comfy_table::{Attribute, Cell, Color, Table};
use komodo_client::{
  api::read::{GetResourceSync, GetUpdate, ListUpdates},
  entities::{
    Operation,
    config::cli::{
      CliTableBorders,
      args::{
        CliFormat,
        sync::{Sync, SyncCommand},
      },
    },
    sync::DiffData,
  },
};

use crate::config::cli_config;

pub async fn handle(sync: &Sync) -> anyhow::Result<()> {
  match &sync.command {
    SyncCommand::Status { format } => {
      show_status(&sync.sync, *format).await
    }
    SyncCommand::Logs { limit, format } => {
      show_logs(&sync.sync, *limit, *format).await
    }
    SyncCommand::RunLog { id, format } => {
      show_run_log(id, *format).await
    }
    SyncCommand::Diff { format } => {
      show_diff(&sync.sync, *format).await
    }
  }
}

async fn show_status(
  name: &str,
  format: CliFormat,
) -> anyhow::Result<()> {
  let client = super::komodo_client().await?;

  let sync = client
    .read(GetResourceSync {
      sync: name.to_string(),
    })
    .await
    .context("Failed to get resource sync")?;

  if matches!(format, CliFormat::Json) {
    return super::print_json(&sync);
  }

  println!("\n{}: {}", "Name".dimmed(), sync.name.bold());
  println!("{}: {}", "ID".dimmed(), sync.id);

  if !sync.config.repo.is_empty() {
    println!("{}: {}", "Repo".dimmed(), sync.config.repo);
    println!("{}: {}", "Branch".dimmed(), sync.config.branch);
  }

  if sync.info.last_sync_ts > 0 {
    let last_sync = super::format_timetamp(sync.info.last_sync_ts)
      .unwrap_or_else(|_| "-".to_string());
    println!("{}: {}", "Last Sync".dimmed(), last_sync);
  }

  if let Some(hash) = &sync.info.last_sync_hash {
    println!("{}: {}", "Last Sync Hash".dimmed(), hash);
  }

  if let Some(message) = &sync.info.last_sync_message {
    println!("{}: {}", "Last Sync Message".dimmed(), message);
  }

  // Show pending updates count
  let pending_resources = sync.info.resource_updates.len();
  let pending_variables = sync.info.variable_updates.len();
  let pending_user_groups = sync.info.user_group_updates.len();
  let total_pending =
    pending_resources + pending_variables + pending_user_groups;

  if total_pending > 0 {
    println!(
      "\n{}: {} pending updates",
      "Pending".yellow().bold(),
      total_pending
    );
    if pending_resources > 0 {
      println!("  - Resources: {}", pending_resources);
    }
    if pending_variables > 0 {
      println!("  - Variables: {}", pending_variables);
    }
    if pending_user_groups > 0 {
      println!("  - User Groups: {}", pending_user_groups);
    }
  } else {
    println!("\n{}: No pending updates", "Status".green().bold());
  }

  if let Some(error) = &sync.info.pending_error {
    println!("\n{}: {}", "Error".red().bold(), error);
  }

  if !sync.description.is_empty() {
    println!("\n{}: {}", "Description".dimmed(), sync.description);
  }

  Ok(())
}

async fn show_logs(
  name: &str,
  limit: u32,
  format: CliFormat,
) -> anyhow::Result<()> {
  let client = super::komodo_client().await?;

  // First get the sync to get its ID
  let sync = client
    .read(GetResourceSync {
      sync: name.to_string(),
    })
    .await
    .context("Failed to get resource sync")?;

  // Query updates for this sync
  let updates = client
    .read(ListUpdates {
      query: Some(bson::doc! {
        "target.type": "ResourceSync",
        "target.id": &sync.id,
        "operation": { "$in": [
          Operation::RunSync.to_string(),
          Operation::CommitSync.to_string()
        ]}
      }),
      page: 0,
    })
    .await
    .context("Failed to list sync runs")?;

  if matches!(format, CliFormat::Json) {
    let updates = updates
      .updates
      .iter()
      .take(limit as usize)
      .collect::<Vec<_>>();
    return super::print_json(&updates);
  }

  if updates.updates.is_empty() {
    println!(
      "{}: No runs found for sync '{}'",
      "INFO".green(),
      name.bold()
    );
    return Ok(());
  }

  println!(
    "\n{}: Showing {} most recent runs. Use `{}` to see detailed logs.\n",
    "TIP".blue(),
    limit.min(updates.updates.len() as u32),
    "km sync <name> run-log <ID>".bold()
  );

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
    ["ID", "Operation", "Status", "Started", "Operator"]
      .map(|h| Cell::new(h).add_attribute(Attribute::Bold)),
  );

  for (i, update) in updates.updates.iter().enumerate() {
    if i >= limit as usize {
      break;
    }

    let status_color = if update.success {
      Color::Green
    } else {
      Color::Red
    };

    let status = if update.success { "Success" } else { "Failed" };

    let started = super::format_timetamp(update.start_ts)
      .unwrap_or_else(|_| "-".to_string());

    table.add_row([
      Cell::new(&update.id),
      Cell::new(update.operation.to_string()),
      Cell::new(status)
        .fg(status_color)
        .add_attribute(Attribute::Bold),
      Cell::new(&started),
      Cell::new(&update.operator),
    ]);
  }

  println!("{table}");
  Ok(())
}

async fn show_run_log(
  id: &str,
  format: CliFormat,
) -> anyhow::Result<()> {
  let client = super::komodo_client().await?;

  let update = client
    .read(GetUpdate { id: id.to_string() })
    .await
    .context("Failed to get sync run log")?;

  if matches!(format, CliFormat::Json) {
    return super::print_json(&update);
  }

  println!("\n{}: {}", "Run ID".dimmed(), update.id.bold());
  println!("{}: {}", "Operation".dimmed(), update.operation);

  let status = if update.success {
    "Success".green()
  } else {
    "Failed".red()
  };
  println!("{}: {}", "Status".dimmed(), status);

  let started = super::format_timetamp(update.start_ts)
    .unwrap_or_else(|_| "-".to_string());
  println!("{}: {}", "Started".dimmed(), started);

  if let Some(end_ts) = update.end_ts {
    let ended = super::format_timetamp(end_ts)
      .unwrap_or_else(|_| "-".to_string());
    println!("{}: {}", "Ended".dimmed(), ended);
  }

  println!("{}: {}", "Operator".dimmed(), update.operator);

  if !update.logs.is_empty() {
    println!("\n{}", "=== Logs ===".bold());
    for log in &update.logs {
      let stage = if log.success {
        log.stage.green()
      } else {
        log.stage.red()
      };
      println!("\n[{}]", stage);
      if !log.stdout.is_empty() {
        print!("{}", log.stdout);
      }
      if !log.stderr.is_empty() {
        eprint!("{}", log.stderr.red());
      }
    }
  }

  Ok(())
}

async fn show_diff(
  name: &str,
  format: CliFormat,
) -> anyhow::Result<()> {
  let client = super::komodo_client().await?;

  let sync = client
    .read(GetResourceSync {
      sync: name.to_string(),
    })
    .await
    .context("Failed to get resource sync")?;

  if matches!(format, CliFormat::Json) {
    return super::print_json(&sync.info);
  }

  let has_resource_updates = !sync.info.resource_updates.is_empty();
  let has_variable_updates = !sync.info.variable_updates.is_empty();
  let has_user_group_updates =
    !sync.info.user_group_updates.is_empty();

  if !has_resource_updates
    && !has_variable_updates
    && !has_user_group_updates
  {
    println!(
      "{}: No pending diffs for sync '{}'",
      "INFO".green(),
      name.bold()
    );
    return Ok(());
  }

  println!(
    "\n{}: Pending changes for sync '{}'\n",
    "DIFF".yellow().bold(),
    name.bold()
  );

  // Show resource updates
  if has_resource_updates {
    println!("{}", "=== Resource Updates ===".bold());
    for diff in &sync.info.resource_updates {
      let (resource_type, resource_id) =
        diff.target.extract_variant_id();
      match &diff.data {
        DiffData::Create { name, proposed } => {
          println!(
            "\n{} {} ({})",
            "+".green().bold(),
            name.green().bold(),
            resource_type
          );
          println!("{}", "Proposed:".dimmed());
          for line in proposed.lines() {
            println!("  {}", line.green());
          }
        }
        DiffData::Update { proposed, current } => {
          println!(
            "\n{} {} ({})",
            "~".yellow().bold(),
            resource_id.yellow().bold(),
            resource_type
          );
          println!("{}", "Current:".dimmed());
          for line in current.lines() {
            println!("  {}", line.red());
          }
          println!("{}", "Proposed:".dimmed());
          for line in proposed.lines() {
            println!("  {}", line.green());
          }
        }
        DiffData::Delete { current } => {
          println!(
            "\n{} {} ({})",
            "-".red().bold(),
            resource_id.red().bold(),
            resource_type
          );
          println!("{}", "Current:".dimmed());
          for line in current.lines() {
            println!("  {}", line.red());
          }
        }
      }
    }
    println!();
  }

  // Show variable updates
  if has_variable_updates {
    println!("{}", "=== Variable Updates ===".bold());
    for diff in &sync.info.variable_updates {
      match diff {
        DiffData::Create { name, proposed } => {
          println!(
            "\n{} {}",
            "+".green().bold(),
            name.green().bold()
          );
          println!("{}", "Proposed:".dimmed());
          for line in proposed.lines() {
            println!("  {}", line.green());
          }
        }
        DiffData::Update { proposed, current } => {
          println!("\n{}", "~ Variable Update".yellow().bold());
          println!("{}", "Current:".dimmed());
          for line in current.lines() {
            println!("  {}", line.red());
          }
          println!("{}", "Proposed:".dimmed());
          for line in proposed.lines() {
            println!("  {}", line.green());
          }
        }
        DiffData::Delete { current } => {
          println!("\n{}", "- Variable Delete".red().bold());
          println!("{}", "Current:".dimmed());
          for line in current.lines() {
            println!("  {}", line.red());
          }
        }
      }
    }
    println!();
  }

  // Show user group updates
  if has_user_group_updates {
    println!("{}", "=== User Group Updates ===".bold());
    for diff in &sync.info.user_group_updates {
      match diff {
        DiffData::Create { name, proposed } => {
          println!(
            "\n{} {}",
            "+".green().bold(),
            name.green().bold()
          );
          println!("{}", "Proposed:".dimmed());
          for line in proposed.lines() {
            println!("  {}", line.green());
          }
        }
        DiffData::Update { proposed, current } => {
          println!("\n{}", "~ User Group Update".yellow().bold());
          println!("{}", "Current:".dimmed());
          for line in current.lines() {
            println!("  {}", line.red());
          }
          println!("{}", "Proposed:".dimmed());
          for line in proposed.lines() {
            println!("  {}", line.green());
          }
        }
        DiffData::Delete { current } => {
          println!("\n{}", "- User Group Delete".red().bold());
          println!("{}", "Current:".dimmed());
          for line in current.lines() {
            println!("  {}", line.red());
          }
        }
      }
    }
  }

  Ok(())
}
