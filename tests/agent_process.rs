#![cfg(unix)]

use std::{
    fs,
    path::PathBuf,
    sync::mpsc::RecvTimeoutError,
    time::{Duration, Instant},
};

use agentdesk::{
    agent::{AgentConfig, AgentEvent, AgentKind, AgentRun, RunRequest, launch},
    model::new_id,
};

struct Fixture {
    path: PathBuf,
}

impl Fixture {
    fn new(script: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!("agentdesk-process-{}", new_id()));
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("agent"), format!("#!/bin/sh\n{script}\n")).unwrap();
        fs::set_permissions(path.join("agent"), fs::Permissions::from_mode(0o700)).unwrap();
        Self { path }
    }

    fn run(&self, kind: AgentKind, session_id: Option<String>) -> AgentRun {
        launch(
            AgentConfig {
                kind,
                executable: self.path.join("agent").to_string_lossy().into(),
                ..AgentConfig::default()
            },
            RunRequest {
                directory: self.path.clone(),
                model: "provider/model".into(),
                prompt: "--flag `literal` $(literal) 中文\nnext line".into(),
                session_id,
            },
        )
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn collect(run: &AgentRun) -> Vec<AgentEvent> {
    let deadline = Instant::now() + Duration::from_secs(6);
    let mut events = vec![];
    loop {
        match run
            .events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            Ok(event) => {
                let done = matches!(event, AgentEvent::Finished { .. });
                events.push(event);
                if done {
                    break;
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                run.cancel();
                panic!("Agent failed to finish within deadline");
            }
            Err(error) => panic!("Agent disconnected before its terminal event: {error}"),
        }
    }
    events
}

#[test]
fn opencode_stream_continuation_and_prompt_are_lossless() {
    let fixture = Fixture::new(
        r#"
printf '%s\n' "$@" > args.txt
cat > input.txt
printf '%s\n' '{"type":"step_start","sessionID":"ses_123","part":{}}'
printf '%s\n' '{"type":"text","sessionID":"ses_123","part":{"id":"text1","text":"Hello 世界"}}'
printf '%s\n' '{"type":"tool_use","sessionID":"ses_123","part":{"id":"tool1","callID":"call1","tool":"bash","state":{"status":"completed","input":{"command":"pwd"},"output":"/project"}}}'
printf '%s\n' '{"type":"step_finish","sessionID":"ses_123","part":{"reason":"stop"}}'
"#,
    );
    let events = collect(&fixture.run(AgentKind::OpenCode, Some("ses_previous".into())));
    assert!(matches!(
        events.last(),
        Some(AgentEvent::Finished { success: true, .. })
    ));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::Text { text, .. } if text == "Hello 世界"))
    );
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Tool { id, details, .. } if id == "call1" && details.contains("/project"))));
    assert_eq!(
        fs::read_to_string(fixture.path.join("input.txt")).unwrap(),
        "--flag `literal` $(literal) 中文\nnext line"
    );
    let args = fs::read_to_string(fixture.path.join("args.txt")).unwrap();
    assert!(args.contains("--session\nses_previous\n"));
    assert!(args.contains("--model\nprovider/model\n"));
    assert!(!args.contains("--auto"));
}

#[test]
fn custom_agent_receives_json_and_emits_deltas() {
    let fixture = Fixture::new(
        r#"
cat > input.json
printf '%s\n' '{"type":"session","id":"private-123"}'
printf '%s\n' '{"type":"text","id":"a","text":"hello","delta":true}'
printf '%s\n' '{"type":"text","id":"a","text":" world","delta":true}'
"#,
    );
    let events = collect(&fixture.run(AgentKind::Custom, Some("private-123".into())));
    let request: RunRequest =
        serde_json::from_slice(&fs::read(fixture.path.join("input.json")).unwrap()).unwrap();
    assert_eq!(request.session_id.as_deref(), Some("private-123"));
    assert_eq!(request.directory, fixture.path);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::Text { delta: true, .. }))
    );
    assert!(matches!(
        events.last(),
        Some(AgentEvent::Finished { success: true, .. })
    ));
}

#[test]
fn malformed_and_empty_output_fail_instead_of_claiming_success() {
    for script in ["cat > /dev/null\nprintf 'not json\\n'", "cat > /dev/null"] {
        let fixture = Fixture::new(script);
        let events = collect(&fixture.run(AgentKind::Custom, None));
        assert!(events.iter().any(|e| matches!(e, AgentEvent::Error { .. })));
        assert!(matches!(
            events.last(),
            Some(AgentEvent::Finished { success: false, .. })
        ));
    }
}

#[test]
fn stderr_and_nonzero_exit_are_reported() {
    let fixture = Fixture::new("cat > /dev/null\nprintf 'authentication required\\n' >&2\nexit 7");
    let events = collect(&fixture.run(AgentKind::OpenCode, None));
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Error { message } if message.contains("authentication required"))));
    assert!(matches!(
        events.last(),
        Some(AgentEvent::Finished { success: false, .. })
    ));
}

#[test]
fn cancellation_terminates_descendants_and_does_not_wait_for_pipe_eof() {
    let fixture = Fixture::new(
        "cat > /dev/null\nsleep 30 &\necho $! > descendant.pid\nprintf '%s\\n' '{\"type\":\"session\",\"id\":\"ready\"}'\nwait",
    );
    let run = fixture.run(AgentKind::Custom, None);
    assert!(matches!(
        run.events.recv_timeout(Duration::from_secs(2)).unwrap(),
        AgentEvent::Session { .. }
    ));
    run.cancel();
    let events = collect(&run);
    assert!(matches!(
        events.last(),
        Some(AgentEvent::Finished {
            cancelled: true,
            ..
        })
    ));
    let pid: i32 = fs::read_to_string(fixture.path.join("descendant.pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while unsafe { libc::kill(pid, 0) } == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_ne!(
        unsafe { libc::kill(pid, 0) },
        0,
        "tool child survived cancellation"
    );
}

#[test]
fn structured_provider_errors_override_a_zero_exit_status() {
    let fixture = Fixture::new(
        r#"cat > /dev/null
printf '%s\n' '{"type":"error","sessionID":"ses_failed","error":{"name":"APIError","data":{"message":"model unavailable"}}}'
"#,
    );
    let events = collect(&fixture.run(AgentKind::OpenCode, None));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::Error { message } if message == "model unavailable"))
    );
    assert!(matches!(
        events.last(),
        Some(AgentEvent::Finished { success: false, .. })
    ));
}

#[test]
fn dropping_a_run_reaps_the_process_before_returning() {
    let fixture = Fixture::new(
        "cat > /dev/null\necho $$ > parent.pid\nprintf '%s\\n' '{\"type\":\"session\",\"id\":\"ready\"}'\nsleep 30",
    );
    let run = fixture.run(AgentKind::Custom, None);
    assert!(matches!(
        run.events.recv_timeout(Duration::from_secs(2)).unwrap(),
        AgentEvent::Session { .. }
    ));
    let pid: i32 = fs::read_to_string(fixture.path.join("parent.pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    drop(run);
    assert_ne!(
        unsafe { libc::kill(pid, 0) },
        0,
        "Agent survived removal of its desktop run handle"
    );
}
