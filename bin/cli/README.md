# Komodo CLI (Agentic Fork)

Komodo CLI is a shell-friendly client for running Komodo actions from scripts and terminals.

This fork tracks upstream Komodo and adds extra, agent-friendly commands on top:

- `stack` — status, logs, services, deploys, deploy-log
- `procedure` — status, logs, run-log
- `sync` — status, logs, run-log, diff
- `variable` — list, get, create (incl. `--from-command`), delete

Everything else matches upstream behavior exactly, including the interactive terminal commands (`connect`/`exec`/`attach`).

## Running commands on servers and in containers

> **This CLI is not the right tool for running shell commands on servers or inside containers.**
> Its terminal commands (`connect`, `exec`, `attach`) open **interactive** terminal sessions over
> WebSocket and are meant for a human at a keyboard.

For one-off or scripted commands on a host or in a container:

1. **SSH directly into the server** using your normal SSH client (`ssh user@server`).
2. From there, run host commands directly, or run a command inside a container with
   `docker exec <container> <cmd>` / `docker compose exec <service> <cmd>`.

Keep `km` for the Komodo REST/WebSocket surface (deploys, status, logs, variables, syncs,
procedures) and use real SSH for everything that is just "run this command over there".

## Install

### Homebrew on Linux

Install this agentic fork on Linux x86_64 or ARM64 with Homebrew 6 or newer:

```sh
brew update
brew tap fariszr/tap
brew trust fariszr/tap
brew install --cask komodo-agentic-cli
km --help
```

Review the [tap](https://github.com/FarisZR/homebrew-tap) before granting trust.
Once tapped and trusted, use the short package name `komodo-agentic-cli`.
The installed command is `km`.

```sh
brew update
brew upgrade --cask komodo-agentic-cli
```

The tap tracks published releases automatically. These are the existing
GNU/Linux release binaries with their glibc/OpenSSL runtime requirements.

### Cargo

```sh
cargo install komodo_cli
```

On Ubuntu, also install:

```sh
apt install build-essential pkg-config libssl-dev
```

## Configure

Set credentials with environment variables:

```sh
export KM_KOMODO_URL="https://your.komodo.address"
export KM_KOMODO_API_KEY="YOUR-API-KEY"
export KM_KOMODO_API_SECRET="YOUR-API-SECRET"
```

Or create `~/.config/komodo/komodo.cli.toml`:

```toml
host = "https://your.komodo.address"
cli_key = "YOUR-API-KEY"
cli_secret = "YOUR-API-SECRET"
```

Inspect the resolved config with:

```sh
km config
```

## Interactive terminal commands

These open a live, interactive terminal session (upstream behavior). They are for humans,
not for scripting one-shot commands — see the note above.

```sh
# Interactive server shell (alias: ssh)
km connect my-server
km connect my-server bash

# Interactive shell inside a container (`docker exec` analogue)
km exec nginx --server my-server bash

# Attach to a container's main process (`docker attach` analogue)
km attach nginx --server my-server
```

## Listing Resources

```sh
km list
km list stacks
km list procedures
km list syncs
km list -d
km list -a
```

## Stack Operations

```sh
km stack my-stack status
km stack my-stack status -f json
km stk my-stack s

km stack my-stack logs
km stack my-stack logs -f json
km stack my-stack logs -s nginx
km stack my-stack logs -s nginx -s redis
km stack my-stack logs -n 200
km stack my-stack logs -t

km stack my-stack services
km stack my-stack services -f json

km stack my-stack deploys
km stack my-stack deploys -f json
km stack my-stack deploys -n 20
km stack my-stack deploy-log <ID>
km stack my-stack deploy-log <ID> -f json
```

## Procedure Operations

```sh
km procedure my-proc status
km procedure my-proc status -f json
km proc my-proc s

km procedure my-proc logs
km procedure my-proc logs -f json
km procedure my-proc logs -n 20
km procedure my-proc run-log <ID>
km procedure my-proc run-log <ID> -f json
```

## Resource Sync Operations

```sh
km sync my-sync status
km sync my-sync status -f json
km sn my-sync s
km sync my-sync diff
km sync my-sync diff -f json

km sync my-sync logs
km sync my-sync logs -f json
km sync my-sync logs -n 20
km sync my-sync run-log <ID>
km sync my-sync run-log <ID> -f json
```

## Variable Management

Secret values stay masked by default in this fork.

```sh
km variable list
km var ls
km var get MY_VAR
```

Enable secret display only when needed:

```sh
KM_SHOW_SECRETS=true km var get MY_SECRET
KM_SHOW_SECRETS=1 km var get MY_SECRET
```

Create and delete variables:

```sh
km var create MY_VAR "my-value"
km var create API_KEY "secret" -s
km var create API_KEY -c "openssl rand -hex 32" -s
km var delete MY_VAR
km var delete MY_VAR -y
```

## Run Executions

Execute commands require confirmation by default. Use `-y` or `--yes` for automation.

```sh
km execute deploy-stack my-stack
km execute run-sync my-sync
km execute run-procedure my-procedure

km execute deploy-stack my-stack -y
km execute run-sync my-sync --yes
km execute run-procedure my-procedure -y

km execute run-build test_build -y
km execute destroy-stack my-stack -y
```

## Other Commands

```sh
km config
km core-info
km container
km database
km create
km update
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `KM_KOMODO_URL` | Komodo server URL (alias: `KOMODO_CLI_HOST`) |
| `KM_KOMODO_API_KEY` | API key (alias: `KOMODO_CLI_KEY`) |
| `KM_KOMODO_API_SECRET` | API secret (alias: `KOMODO_CLI_SECRET`) |
| `KM_SHOW_SECRETS` | Show secret variable values when set to `true` or `1` |

## Full Command Reference

Use `km --help` for the current command tree.
