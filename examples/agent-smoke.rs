//! Exercise the real OpenCode adapter against an explicitly chosen directory.
use agentdesk::agent::{AgentConfig, AgentEvent, RunRequest, launch};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let directory = PathBuf::from(
        args.next()
            .ok_or_else(|| anyhow::anyhow!("usage: agent-smoke DIRECTORY PROMPT [SESSION_ID]"))?,
    )
    .canonicalize()?;
    let prompt = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing prompt"))?;
    let session_id = args.next();
    let run = launch(
        AgentConfig::default(),
        RunRequest {
            directory,
            model: String::new(),
            prompt,
            session_id,
        },
    );
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let event = run
            .events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))?;
        match event {
            AgentEvent::Finished { cancelled, success } => {
                anyhow::ensure!(success && !cancelled, "Agent run failed");
                break;
            }
            event => println!("{}", serde_json::to_string(&event)?),
        }
    }
    Ok(())
}
