//! `grok models` subcommand.

use anyhow::Result;
use tokio_util::sync::CancellationToken;
use xai_grok_shell::agent::config::Config as AgentConfig;
use xai_grok_shell::cli_models::{AuthStatus, list_models};

use crate::client_identity::{PAGER_CLIENT_TYPE, PAGER_CLIENT_VERSION};

pub async fn list_available_models(agent_config: &AgentConfig) -> Result<()> {
    match AuthStatus::resolve(agent_config) {
        AuthStatus::ApiKey => println!("You are using XAI_API_KEY."),
        AuthStatus::LoggedIn(host) => println!("You are logged in with {}.", host),
        AuthStatus::ModelCredentials(model) => {
            println!("Model '{model}' is using its own API key.");
        }
        AuthStatus::DeploymentKey => println!("You are authenticated via deployment key."),
        AuthStatus::NotAuthenticated => println!("You are not authenticated."),
    }
    println!();

    let cancel = CancellationToken::new();
    xai_grok_telemetry::startup::mark_utility_process();
    let spawned = crate::acp::spawn::spawn_grok_shell(agent_config.clone(), &cancel, None).await?;
    // Cancel and join on every return path, including the `?` below
    let _agent_guard =
        crate::acp::spawn::AgentShutdownGuard::new(cancel.clone(), Some(spawned.thread_handle));

    let state = list_models(&spawned.channel.tx, PAGER_CLIENT_TYPE, PAGER_CLIENT_VERSION).await?;

    println!("Default model: {}", state.current_model_id.0);
    println!();
    println!("Available models:");
    let hidden = grog_providers::visibility::load_hidden_from_grog_home();
    let show_hidden = grog_providers::visibility::show_hidden();
    for m in state.available_models {
        let key = m.model_id.0.as_ref();
        if !grog_providers::visibility::is_picker_visible(key, &hidden) && !show_hidden {
            continue;
        }
        let source = grog_providers::source_label(key);
        let mark = if m.model_id == state.current_model_id {
            "*"
        } else {
            "-"
        };
        let hidden_tag = if hidden.contains(key) {
            " (hidden)"
        } else {
            ""
        };
        println!("  {mark} {key}  {source}{hidden_tag}");
    }

    Ok(())
}
