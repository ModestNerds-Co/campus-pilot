//! Internal-only durable Agent execution worker.

use std::{env, sync::Arc, time::Duration};

use anyhow::{Context, Result, bail};
use campus_pilot::{
    config::Config,
    services::agent::{AgentWorkerReadinessOps, AgentWorkerSupervisor, ProviderAgentRunExecutor},
    state::AppState,
};
use dotenv::dotenv;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::watch;

const WORKER_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(500);

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init()
        .ok();

    let config = Config::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database.url)
        .await
        .context("connect Agent worker database")?;
    let state = Arc::new(AppState::init(pool.clone(), config.clone()));
    let artifact_keyring = config
        .agent
        .artifact_keyring
        .clone()
        .context("AGENT_ARTIFACT_KEYS_JSON is required by the Agent worker")?;
    let readiness = AgentWorkerReadinessOps::new(pool);
    readiness.cleanup_workers().await?;

    let worker_key = env::var("AGENT_WORKER_KEY").unwrap_or_else(|_| "agent-worker".to_owned());
    let batch_size = env::var("AGENT_WORKER_BATCH_SIZE")
        .unwrap_or_else(|_| "4".to_owned())
        .parse::<u16>()
        .context("AGENT_WORKER_BATCH_SIZE must be an integer")?;
    if !(1..=25).contains(&batch_size) {
        bail!("AGENT_WORKER_BATCH_SIZE must be between 1 and 25");
    }

    let coverage = readiness
        .startup_coverage(
            &artifact_keyring,
            config.ai_providers.credential_keyring.as_ref(),
        )
        .await?;
    let mut instance = readiness.register_worker(&worker_key).await?;
    readiness.mark_ready(&mut instance, coverage).await?;

    let executor = Arc::new(ProviderAgentRunExecutor::new(
        (*state.agent_session_ops).clone(),
        (*state.agent_usage_runtime).clone(),
        (*state.ai_provider_ops).clone(),
        (*state.ai_routing_ops).clone(),
        state.agent_authority_loader.clone(),
        state.agent_capability_broker.clone(),
        artifact_keyring.clone(),
    ));
    let supervisor = AgentWorkerSupervisor::new(
        (*state.agent_session_ops).clone(),
        (*state.agent_usage_runtime).clone(),
        (*state.agent_worker_readiness).clone(),
        executor,
        &worker_key,
        batch_size,
    )?;
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let mut worker_loop = tokio::spawn(async move {
        loop {
            if *shutdown_rx.borrow() {
                break;
            }
            match supervisor.run_once().await {
                Ok(tick) => {
                    if tick.claimed == 0 {
                        tokio::select! {
                            _ = tokio::time::sleep(IDLE_POLL_INTERVAL) => {},
                            changed = shutdown_rx.changed() => {
                                if changed.is_err() || *shutdown_rx.borrow() {
                                    break;
                                }
                            }
                        }
                    }
                }
                Err(error) => {
                    log::error!("Agent worker tick failed: {error}");
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(1)) => {},
                        changed = shutdown_rx.changed() => {
                            if changed.is_err() || *shutdown_rx.borrow() {
                                break;
                            }
                        }
                    }
                }
            }
        }
    });

    let mut heartbeat = tokio::time::interval(WORKER_HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    heartbeat.tick().await;
    let mut heartbeat_count = 0_u16;
    let mut worker_loop_finished = false;
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);
    let worker_result = loop {
        tokio::select! {
            signal = &mut shutdown => {
                signal?;
                break Ok(());
            }
            result = &mut worker_loop => {
                worker_loop_finished = true;
                match result {
                    Ok(()) => break Err(anyhow::anyhow!("Agent worker execution loop exited")),
                    Err(error) => break Err(anyhow::Error::new(error).context("Agent worker execution loop panicked")),
                }
            }
            _ = heartbeat.tick() => {
                let current = readiness.startup_coverage(
                    &artifact_keyring,
                    config.ai_providers.credential_keyring.as_ref(),
                ).await?;
                if let Err(error) = readiness.heartbeat_ready(&mut instance, current).await {
                    break Err(anyhow::Error::new(error).context("refresh Agent worker readiness"));
                }
                heartbeat_count += 1;
                if heartbeat_count >= 240 {
                    readiness.cleanup_workers().await?;
                    heartbeat_count = 0;
                }
            }
        }
    };

    let _ = readiness
        .mark_draining(&mut instance, "worker_shutdown")
        .await;
    let _ = shutdown_tx.send(true);
    if !worker_loop_finished {
        let _ = worker_loop.await;
    }
    let _ = readiness
        .mark_unavailable(&mut instance, "worker_shutdown")
        .await;
    worker_result
}

async fn shutdown_signal() -> Result<()> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .context("listen for Agent worker SIGTERM")?;
        tokio::select! {
            signal = tokio::signal::ctrl_c() => {
                signal.context("listen for Agent worker interrupt")?;
            }
            _ = terminate.recv() => {}
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .context("listen for Agent worker shutdown")
    }
}
