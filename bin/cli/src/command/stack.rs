use anyhow::Context;
use colored::Colorize;
use comfy_table::{Attribute, Cell, Color, Table};
use komodo_client::{
  api::read::{
    GetStack, GetStackLog, ListStackServices, ListUpdates,
  },
  entities::{
    Operation,
    config::cli::{
      CliTableBorders,
      args::{
        CliFormat,
        stack::{Stack, StackCommand, StackLogsOptions},
      },
    },
  },
};

use crate::config::cli_config;

pub async fn handle(stack: &Stack) -> anyhow::Result<()> {
  match &stack.command {
    StackCommand::Status { format } => {
      show_status(&stack.stack, *format).await
    }
    StackCommand::Logs(opts) => show_logs(&stack.stack, opts).await,
    StackCommand::Services { format } => {
      list_services(&stack.stack, *format).await
    }
    StackCommand::Deploys { limit, format } => {
      show_deploys(&stack.stack, *limit, *format).await
    }
    StackCommand::DeployLog { id, format } => {
      show_deploy_log(id, *format).await
    }
  }
}

async fn show_status(
  name: &str,
  format: CliFormat,
) -> anyhow::Result<()> {
  let client = super::komodo_client().await?;

  let stack = client
    .read(GetStack {
      stack: name.to_string(),
    })
    .await
    .context("Failed to get stack")?;

  if matches!(format, CliFormat::Json) {
    return super::print_json(&stack);
  }

  println!("\n{}: {}", "Name".dimmed(), stack.name.bold());
  println!("{}: {}", "ID".dimmed(), stack.id);

  if !stack.config.server_id.is_empty() {
    println!("{}: {}", "Server".dimmed(), stack.config.server_id);
  }

  if !stack.config.repo.is_empty() {
    println!("{}: {}", "Repo".dimmed(), stack.config.repo);
    println!("{}: {}", "Branch".dimmed(), stack.config.branch);
  }

  if let Some(hash) = &stack.info.deployed_hash {
    println!("{}: {}", "Deployed Hash".dimmed(), hash);
  }

  if let Some(message) = &stack.info.deployed_message {
    println!("{}: {}", "Deployed Message".dimmed(), message);
  }

  if let Some(services) = &stack.info.deployed_services {
    println!("{}: {}", "Services".dimmed(), services.len());
    for svc in services {
      println!("  - {}: {}", svc.service_name, svc.image);
    }
  }

  if !stack.description.is_empty() {
    println!("\n{}: {}", "Description".dimmed(), stack.description);
  }

  Ok(())
}

async fn list_services(
  stack: &str,
  format: CliFormat,
) -> anyhow::Result<()> {
  let client = super::komodo_client().await?;

  let services = client
    .read(ListStackServices {
      stack: stack.to_string(),
    })
    .await
    .context("Failed to list stack services")?;

  if matches!(format, CliFormat::Json) {
    return super::print_json(&services);
  }

  if services.is_empty() {
    println!(
      "{}: No services found for stack '{}'",
      "INFO".green(),
      stack.bold()
    );
    return Ok(());
  }

  let preset = {
    use comfy_table::presets::*;
    use komodo_client::entities::config::cli::CliTableBorders;
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
    ["Service", "Image", "Container", "State"]
      .map(|h| Cell::new(h).add_attribute(Attribute::Bold)),
  );

  for service in services {
    let (container_name, state, color) = if let Some(container) =
      &service.container
    {
      let state = container.state.to_string();
      let color = match container.state {
          komodo_client::entities::docker::container::ContainerStateStatusEnum::Running => {
            Color::Green
          }
          komodo_client::entities::docker::container::ContainerStateStatusEnum::Paused => {
            Color::DarkYellow
          }
          komodo_client::entities::docker::container::ContainerStateStatusEnum::Empty => {
            Color::Grey
          }
          _ => Color::Red,
        };
      (container.name.clone(), state, color)
    } else {
      (String::from("-"), String::from("Not running"), Color::Grey)
    };

    table.add_row([
      Cell::new(&service.service).add_attribute(Attribute::Bold),
      Cell::new(&service.image),
      Cell::new(&container_name),
      Cell::new(&state).fg(color).add_attribute(Attribute::Bold),
    ]);
  }

  println!("{table}");
  Ok(())
}

async fn show_logs(
  stack: &str,
  opts: &StackLogsOptions,
) -> anyhow::Result<()> {
  let client = super::komodo_client().await?;

  // Cap tail at 5000
  let tail = opts.tail.min(5000);

  let log = client
    .read(GetStackLog {
      stack: stack.to_string(),
      services: opts.services.clone(),
      tail,
      timestamps: opts.timestamps,
    })
    .await
    .context("Failed to get stack logs")?;

  if matches!(opts.format, CliFormat::Json) {
    return super::print_json(&log);
  }

  if log.stdout.is_empty() && log.stderr.is_empty() {
    println!(
      "{}: No logs found for stack '{}'",
      "INFO".green(),
      stack.bold()
    );
    return Ok(());
  }

  // Print stdout
  if !log.stdout.is_empty() {
    print!("{}", log.stdout);
  }

  // Print stderr if present
  if !log.stderr.is_empty() {
    eprint!("{}", log.stderr.red());
  }

  Ok(())
}

async fn show_deploys(
  name: &str,
  limit: u32,
  format: CliFormat,
) -> anyhow::Result<()> {
  let client = super::komodo_client().await?;

  // First get the stack to get its ID
  let stack = client
    .read(GetStack {
      stack: name.to_string(),
    })
    .await
    .context("Failed to get stack")?;

  // Query updates for this stack
  let updates = client
    .read(ListUpdates {
      query: Some(bson::doc! {
        "target.type": "Stack",
        "target.id": &stack.id,
        "operation": { "$in": [
          Operation::DeployStack.to_string(),
          Operation::StartStack.to_string(),
          Operation::RestartStack.to_string(),
          Operation::StopStack.to_string(),
          Operation::DestroyStack.to_string(),
          Operation::PauseStack.to_string(),
          Operation::UnpauseStack.to_string()
        ]}
      }),
      page: 0,
    })
    .await
    .context("Failed to list stack deployments")?;

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
      "{}: No deployments found for stack '{}'",
      "INFO".green(),
      name.bold()
    );
    return Ok(());
  }

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

async fn show_deploy_log(
  id: &str,
  format: CliFormat,
) -> anyhow::Result<()> {
  use komodo_client::api::read::GetUpdate;

  let client = super::komodo_client().await?;

  let update = client
    .read(GetUpdate { id: id.to_string() })
    .await
    .context("Failed to get deployment log")?;

  if matches!(format, CliFormat::Json) {
    return super::print_json(&update);
  }

  println!("\n{}: {}", "Deployment".dimmed(), update.id.bold());
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
