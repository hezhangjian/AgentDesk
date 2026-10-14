use std::{fs, path::PathBuf};

use agentdesk::{
    agent::{AgentConfig, AgentEvent},
    model::{EntryKind, Session, SessionStatus, Workspace, new_id},
    storage::Storage,
};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("agentdesk-test-{}", new_id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn projects_are_canonical_deduplicated_and_sorted() {
    let directory = TestDirectory::new();
    let mut workspace = Workspace::default();
    for name in ["Zebra", "alpha"] {
        fs::create_dir(directory.0.join(name)).unwrap();
        workspace.add_project(directory.0.join(name)).unwrap();
    }
    let id = workspace
        .add_project(directory.0.join("Zebra/../alpha"))
        .unwrap();
    assert_eq!(workspace.projects.len(), 2);
    assert_eq!(workspace.projects[0].id, id);
    assert_eq!(workspace.projects[0].name, "alpha");
    fs::write(directory.0.join("file"), "text").unwrap();
    assert!(workspace.add_project(directory.0.join("file")).is_err());
}

#[test]
fn persistence_restores_context_and_marks_incomplete_runs_interrupted() {
    let directory = TestDirectory::new();
    let storage = Storage {
        directory: directory.0.join("state"),
    };
    let mut workspace = storage.load().unwrap();
    workspace.add_project(directory.0.clone()).unwrap();
    let mut session = Session::new(
        &workspace.projects[0],
        AgentConfig::default(),
        "中文任务 🦀",
    );
    session.begin("中文任务 🦀".into());
    session.apply(AgentEvent::Session {
        id: "remote-123".into(),
    });
    session.draft = "下一个问题".into();
    workspace.sessions.push(session);
    storage.save(&workspace).unwrap();
    let loaded = storage.load().unwrap();
    assert_eq!(
        loaded.sessions[0].remote_session_id.as_deref(),
        Some("remote-123")
    );
    assert_eq!(loaded.sessions[0].status, SessionStatus::Interrupted);
    assert_eq!(loaded.sessions[0].draft, "下一个问题");
    assert_eq!(loaded.sessions[0].entries[0].text, "中文任务 🦀");
    assert_eq!(
        loaded.sessions[0].directory,
        directory.0.canonicalize().unwrap()
    );
    storage.save(&loaded).unwrap();
    assert_eq!(storage.load().unwrap().sessions[0].entries.len(), 2);
}

#[test]
fn corrupt_and_future_workspaces_are_not_silently_reset() {
    let directory = TestDirectory::new();
    let storage = Storage {
        directory: directory.0.clone(),
    };
    let path = directory.0.join("workspace.json");
    fs::write(&path, "{broken").unwrap();
    assert!(storage.load().is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "{broken");
    let workspace = Workspace {
        version: 99,
        ..Workspace::default()
    };
    storage.save(&workspace).unwrap();
    assert!(storage.load().is_err());
}

#[test]
fn event_reduction_preserves_order_updates_tools_and_never_masks_errors() {
    let directory = TestDirectory::new();
    let mut workspace = Workspace::default();
    workspace.add_project(directory.0.clone()).unwrap();
    let mut session = Session::new(&workspace.projects[0], AgentConfig::default(), "test");
    session.begin("test".into());
    session.apply(AgentEvent::Text {
        id: "part1".into(),
        text: "你好".into(),
        delta: true,
    });
    session.apply(AgentEvent::Text {
        id: "part1".into(),
        text: "世界".into(),
        delta: true,
    });
    for output in ["running", "done"] {
        session.apply(AgentEvent::Tool {
            id: "call1".into(),
            name: "read".into(),
            details: output.into(),
            failed: false,
        });
    }
    session.apply(AgentEvent::Text {
        id: "part2".into(),
        text: "完成".into(),
        delta: false,
    });
    session.apply(AgentEvent::Error {
        message: "provider failure".into(),
    });
    session.apply(AgentEvent::Finished {
        cancelled: false,
        success: true,
    });
    assert_eq!(session.entries.len(), 5);
    assert_eq!(session.entries[1].text, "你好世界");
    assert_eq!(session.entries[2].kind, EntryKind::Tool);
    assert_eq!(session.entries[2].details, "done");
    assert_eq!(session.status, SessionStatus::Failed);
    session.begin("retry".into());
    session.apply(AgentEvent::Finished {
        cancelled: true,
        success: false,
    });
    assert_eq!(session.status, SessionStatus::Interrupted);
}
