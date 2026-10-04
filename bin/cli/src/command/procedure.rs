use anyhow::Context;
use colored::Colorize;
use comfy_table::{Attribute, Cell, Color, Table};
use komodo_client::{
  api::read::{GetProcedure, GetUpdate, ListUpdates},
  entities::{
    Operation,
    config::cli::{
      CliTableBorders,
      args::{
        CliFormat,
        procedure::{Procedure, ProcedureCommand},
      },
    },
  },
};

use crate::config::cli_config;

pub async fn handle(procedure: &Procedure) -> anyhow::Result<()> {
  match &procedure.command {
    ProcedureCommand::Status { format } => {
      show_status(&procedure.procedure, *format).await
    }
    ProcedureCommand::Logs { limit, format } => {
      show_logs(&procedure.procedure, *limit, *format).await
    }
    ProcedureCommand::RunLog { id, format } => {
      show_run_log(id, *format).await
    }
  }
}

async fn show_status(
  name: &str,
  format: CliFormat,
) -> anyhow::Result<()> {
  let client = super::komodo_client().await?;

  let procedure = client
    .read(GetProcedure {
      procedure: name.to_string(),
    })
    .await
    .context("Failed to get procedure")?;

  if matches!(format, CliFormat::Json) {
    return super::print_json(&procedure);
  }

  println!("\n{}: {}", "Name".dimmed(), procedure.name.bold());
  println!("{}: {}", "ID".dimmed(), procedure.id);

  let schedule_enabled = if procedure.config.schedule_enabled {
    "true".green()
  } else {
    "false".dimmed()
  };
  println!("{}: {}", "Schedule Enabled".dimmed(), schedule_enabled);

  if !procedure.config.schedule.is_empty() {
    println!(
      "{}: {}",
      "Schedule".dimmed(),
      procedure.config.schedule
    );
  }

  println!(
    "{}: {}",
    "Stages".dimmed(),
    procedure.config.stages.len()
  );

  if !procedure.description.is_empty() {
    println!("{}: {}", "Description".dimmed(), procedure.description);
  }

  Ok(())
}

async fn show_logs(
  name: &str,
  limit: u32,
  format: CliFormat,
) -> anyhow::Result<()> {
  let client = super::komodo_client().await?;

  // First get the procedure to get its ID
  let procedure = client
    .read(GetProcedure {
      procedure: name.to_string(),
    })
    .await
    .context("Failed to get procedure")?;

  // Query updates for this procedure
  let updates = client
    .read(ListUpdates {
      query: Some(bson::doc! {
        "target.type": "Procedure",
        "target.id": &procedure.id,
        "operation": { "$in": [
          Operation::RunProcedure.to_string()
        ]}
      }),
      page: 0,
    })
    .await
    .context("Failed to list procedure runs")?;

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
      "{}: No runs found for procedure '{}'",
      "INFO".green(),
      name.bold()
    );
    return Ok(());
  }

  println!(
    "\n{}: Showing {} most recent runs. Use `{}` to see detailed logs.\n",
    "TIP".blue(),
    limit.min(updates.updates.len() as u32),
    "km procedure <name> run-log <ID>".bold()
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
    ["ID", "Status", "Started", "Operator"]
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
    .context("Failed to get procedure run log")?;

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
