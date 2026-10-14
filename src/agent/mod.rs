mod opencode;
mod process;

use std::{path::PathBuf, process::Command};

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub use process::{AgentRun, launch};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    Custom,
    #[default]
    OpenCode,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AgentConfig {
    pub kind: AgentKind,
    pub executable: String,
    pub arguments: Vec<String>,
    pub model: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            kind: AgentKind::OpenCode,
            executable: "opencode".into(),
            arguments: vec![],
            model: String::new(),
        }
    }
}

impl AgentConfig {
    pub fn label(&self) -> &'static str {
        match self.kind {
            AgentKind::Custom => "企业 Agent",
            AgentKind::OpenCode => "OpenCode",
        }
    }

    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            !self.executable.trim().is_empty(),
            "请填写 Agent 可执行文件路径"
        );
        let path = std::path::Path::new(&self.executable);
        anyhow::ensure!(
            path.is_absolute() || path.components().count() == 1,
            "可执行文件请使用绝对路径或 PATH 中的命令名称"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RunRequest {
    pub directory: PathBuf,
    pub model: String,
    pub prompt: String,
    pub session_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    Error {
        message: String,
    },
    Notice {
        message: String,
    },
    Session {
        id: String,
    },
    Text {
        id: String,
        text: String,
        #[serde(default)]
        delta: bool,
    },
    Tool {
        id: String,
        name: String,
        details: String,
        #[serde(default)]
        failed: bool,
    },
    #[serde(skip)]
    Finished {
        cancelled: bool,
        success: bool,
    },
}

/// Adapters translate a transport into UI-independent conversation events.
trait AgentAdapter: Send + Sync {
    fn command(&self, config: &AgentConfig, request: &RunRequest) -> Command;
    fn input(&self, request: &RunRequest) -> Result<Vec<u8>>;
    fn decode(&self, line: &str) -> Result<Vec<AgentEvent>>;
}

struct CustomAdapter;

impl AgentAdapter for CustomAdapter {
    fn command(&self, config: &AgentConfig, _: &RunRequest) -> Command {
        let mut command = Command::new(resolve_executable(&config.executable));
        command.args(&config.arguments);
        command
    }

    fn input(&self, request: &RunRequest) -> Result<Vec<u8>> {
        let mut data = serde_json::to_vec(request)?;
        data.push(b'\n');
        Ok(data)
    }

    fn decode(&self, line: &str) -> Result<Vec<AgentEvent>> {
        Ok(vec![serde_json::from_str(line)?])
    }
}

fn adapter(kind: &AgentKind) -> Box<dyn AgentAdapter> {
    match kind {
        AgentKind::Custom => Box::new(CustomAdapter),
        AgentKind::OpenCode => Box::new(opencode::OpenCodeAdapter),
    }
}

fn resolve_executable(executable: &str) -> PathBuf {
    let path = PathBuf::from(executable);
    if path.is_absolute() {
        return path;
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&paths) {
            if directory.is_absolute() && directory.join(&path).is_file() {
                return directory.join(path);
            }
        }
    }
    // Finder launches may not inherit the user's shell PATH.
    for directory in [
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ]
    .into_iter()
    .chain(dirs::home_dir().map(|home| home.join(".opencode/bin")))
    {
        if directory.join(&path).is_file() {
            return directory.join(path);
        }
    }
    path
}
