use std::process::Command;

use anyhow::Result;
use serde_json::Value;

use super::{AgentAdapter, AgentConfig, AgentEvent, RunRequest, resolve_executable};

pub(super) struct OpenCodeAdapter;

impl AgentAdapter for OpenCodeAdapter {
    fn command(&self, config: &AgentConfig, request: &RunRequest) -> Command {
        let mut command = Command::new(resolve_executable(&config.executable));
        command
            .args(["run", "--format", "json", "--dir"])
            .arg(&request.directory);
        if let Some(id) = &request.session_id {
            command.args(["--session", id]);
        }
        if !request.model.is_empty() {
            command.args(["--model", &request.model]);
        }
        command
    }

    fn input(&self, request: &RunRequest) -> Result<Vec<u8>> {
        Ok(request.prompt.as_bytes().to_vec())
    }

    fn decode(&self, line: &str) -> Result<Vec<AgentEvent>> {
        let value: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => {
                return Ok(vec![AgentEvent::Notice {
                    message: line.to_owned(),
                }]);
            }
        };
        let mut events = vec![];
        if let Some(id) = value["sessionID"].as_str().filter(|s| !s.is_empty()) {
            events.push(AgentEvent::Session { id: id.into() });
        }
        let part = &value["part"];
        match value["type"].as_str().unwrap_or_default() {
            "text" => {
                let text = part["text"].as_str().unwrap_or_default();
                if !text.is_empty() {
                    events.push(AgentEvent::Text {
                        id: part["id"]
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(crate::model::new_id),
                        text: text.into(),
                        delta: false,
                    });
                }
            }
            "tool_use" => {
                let state = &part["state"];
                let name = part["tool"].as_str().unwrap_or("tool");
                let title = state["title"]
                    .as_str()
                    .or_else(|| state["input"]["command"].as_str())
                    .unwrap_or(name);
                let input = serde_json::to_string_pretty(&state["input"])?;
                let output = state["output"]
                    .as_str()
                    .or_else(|| state["error"].as_str())
                    .unwrap_or_default();
                events.push(AgentEvent::Tool {
                    id: part["callID"]
                        .as_str()
                        .or_else(|| part["id"].as_str())
                        .map(str::to_owned)
                        .unwrap_or_else(crate::model::new_id),
                    name: format!("{name} · {title}"),
                    details: format!("{input}\n\n{output}"),
                    failed: state["status"] == "error",
                });
            }
            "error" => {
                let error = &value["error"];
                let message = error["data"]["message"]
                    .as_str()
                    .or_else(|| error["message"].as_str())
                    .or_else(|| error["name"].as_str())
                    .map(str::to_owned)
                    .unwrap_or_else(|| error.to_string());
                events.push(AgentEvent::Error { message });
            }
            _ => {}
        }
        Ok(events)
    }
}
