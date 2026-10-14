use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::{AgentConfig, AgentEvent};

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    #[default]
    Idle,
    Running,
    Completed,
    Failed,
    Interrupted,
}

impl SessionStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "准备就绪",
            Self::Running => "正在执行",
            Self::Completed => "已完成",
            Self::Failed => "执行失败",
            Self::Interrupted => "已停止",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    Assistant,
    Error,
    Notice,
    Tool,
    User,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Entry {
    pub id: String,
    pub kind: EntryKind,
    pub text: String,
    #[serde(default)]
    pub details: String,
    #[serde(default)]
    pub failed: bool,
}

impl Entry {
    pub fn new(kind: EntryKind, text: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            kind,
            text: text.into(),
            details: String::new(),
            failed: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Session {
    pub id: String,
    pub project_id: String,
    pub directory: PathBuf,
    pub title: String,
    pub agent: AgentConfig,
    pub remote_session_id: Option<String>,
    pub updated_at: u64,
    pub status: SessionStatus,
    pub entries: Vec<Entry>,
    #[serde(default)]
    pub draft: String,
}

impl Session {
    pub fn new(project: &Project, agent: AgentConfig, prompt: &str) -> Self {
        Self {
            id: new_id(),
            project_id: project.id.clone(),
            directory: project.path.clone(),
            title: prompt
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(36)
                .collect(),
            agent,
            remote_session_id: None,
            updated_at: now(),
            status: SessionStatus::Idle,
            entries: vec![],
            draft: String::new(),
        }
    }

    pub fn begin(&mut self, prompt: String) {
        self.entries.push(Entry::new(EntryKind::User, prompt));
        self.status = SessionStatus::Running;
        self.updated_at = now();
        self.draft.clear();
    }

    pub fn apply(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::Session { id } => self.remote_session_id = Some(id),
            AgentEvent::Text { id, text, delta } => {
                if let Some(entry) = self
                    .entries
                    .iter_mut()
                    .find(|e| e.id == id && e.kind == EntryKind::Assistant)
                {
                    if delta {
                        entry.text.push_str(&text);
                    } else {
                        entry.text = text;
                    }
                } else {
                    self.entries.push(Entry {
                        id,
                        kind: EntryKind::Assistant,
                        text,
                        details: String::new(),
                        failed: false,
                    });
                }
            }
            AgentEvent::Tool {
                id,
                name,
                details,
                failed,
            } => {
                let entry = Entry {
                    id: id.clone(),
                    kind: EntryKind::Tool,
                    text: name,
                    details,
                    failed,
                };
                if let Some(existing) = self
                    .entries
                    .iter_mut()
                    .find(|e| e.id == id && e.kind == EntryKind::Tool)
                {
                    *existing = entry;
                } else {
                    self.entries.push(entry);
                }
            }
            AgentEvent::Error { message } => {
                self.entries.push(Entry::new(EntryKind::Error, message));
                self.status = SessionStatus::Failed;
            }
            AgentEvent::Notice { message } => {
                self.entries.push(Entry::new(EntryKind::Notice, message))
            }
            AgentEvent::Finished { cancelled, success } => {
                self.status = if cancelled {
                    SessionStatus::Interrupted
                } else if success && self.status != SessionStatus::Failed {
                    SessionStatus::Completed
                } else {
                    SessionStatus::Failed
                };
            }
        }
        self.updated_at = now();
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Workspace {
    pub version: u32,
    pub projects: Vec<Project>,
    pub sessions: Vec<Session>,
    pub agent: AgentConfig,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            version: 1,
            projects: vec![],
            sessions: vec![],
            agent: AgentConfig::default(),
        }
    }
}

impl Workspace {
    pub fn add_project(&mut self, path: PathBuf) -> anyhow::Result<String> {
        let path = path.canonicalize()?;
        anyhow::ensure!(path.is_dir(), "请选择一个文件夹");
        if let Some(existing) = self.projects.iter().find(|p| p.path == path) {
            return Ok(existing.id.clone());
        }
        let id = new_id();
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        self.projects.push(Project {
            id: id.clone(),
            name,
            path,
        });
        self.projects.sort_by_key(|p| p.name.to_lowercase());
        Ok(id)
    }

    pub fn recover(&mut self) {
        for session in &mut self.sessions {
            if session.status == SessionStatus::Running {
                session.status = SessionStatus::Interrupted;
                session.entries.push(Entry::new(
                    EntryKind::Notice,
                    "上次运行随应用关闭而中断，可继续发送消息。",
                ));
            }
        }
    }
}
