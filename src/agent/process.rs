use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::Duration,
};

use anyhow::{Context, Result};

use super::{AgentConfig, AgentEvent, RunRequest, adapter};

const MAX_LINE_BYTES: u64 = 1024 * 1024;
const MAX_DIAGNOSTIC_CHARS: usize = 8000;

pub struct AgentRun {
    pub events: Receiver<AgentEvent>,
    cancelled: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl AgentRun {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl Drop for AgentRun {
    fn drop(&mut self) {
        self.cancel();
        // Release backpressure before waiting for the process supervisor to exit.
        let (_, empty) = mpsc::channel();
        drop(std::mem::replace(&mut self.events, empty));
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn launch(config: AgentConfig, request: RunRequest) -> AgentRun {
    let (sender, events) = mpsc::sync_channel(256);
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancellation = cancelled.clone();
    let worker = thread::spawn(move || {
        let result = execute(&config, request, &sender, &cancellation);
        let success = matches!(&result, Ok(true));
        if let Err(error) = result {
            let _ = sender.send(AgentEvent::Error {
                message: format!("{error:#}"),
            });
        }
        let _ = sender.send(AgentEvent::Finished {
            cancelled: cancellation.load(Ordering::Relaxed),
            success,
        });
    });
    AgentRun {
        events,
        cancelled,
        worker: Some(worker),
    }
}

struct ManagedChild(Child);

impl ManagedChild {
    fn terminate(&mut self) {
        #[cfg(unix)]
        {
            // Every child starts in its own process group, including tools it spawns.
            unsafe {
                libc::kill(-(self.0.id() as i32), libc::SIGKILL);
            }
        }
        let _ = self.0.kill();
    }
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        self.terminate();
        let _ = self.0.wait();
    }
}

fn execute(
    config: &AgentConfig,
    request: RunRequest,
    sender: &SyncSender<AgentEvent>,
    cancelled: &Arc<AtomicBool>,
) -> Result<bool> {
    if cancelled.load(Ordering::Relaxed) {
        return Ok(false);
    }
    config.validate()?;
    anyhow::ensure!(
        request.directory.is_dir(),
        "项目目录不存在：{}",
        request.directory.display()
    );
    let adapter = adapter(&config.kind);
    let input = adapter.input(&request)?;
    let mut command = adapter.command(config, &request);
    command
        .current_dir(&request.directory)
        .env("PWD", &request.directory)
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = ManagedChild(command.spawn().with_context(|| {
        format!(
            "无法启动 {}。请检查安装、可执行文件路径和 Agent 设置",
            config.executable
        )
    })?);
    let mut stdin = child.0.stdin.take().context("无法打开 Agent 输入")?;
    let stdout = child.0.stdout.take().context("无法打开 Agent 输出")?;
    let stderr = child.0.stderr.take().context("无法打开 Agent 日志")?;
    let failed = Arc::new(AtomicBool::new(false));
    let write_failed = failed.clone();
    let write_sender = sender.clone();
    let writer = thread::spawn(move || {
        if let Err(error) = stdin.write_all(&input) {
            write_failed.store(true, Ordering::Relaxed);
            let _ = write_sender.send(AgentEvent::Error {
                message: format!("无法发送消息：{error}"),
            });
        }
        // EOF is part of the one-request-per-process protocol.
    });
    let read_failed = failed.clone();
    let read_sender = sender.clone();
    let reader = thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut saw_content = false;
        loop {
            let mut line = String::new();
            let read = reader
                .by_ref()
                .take(MAX_LINE_BYTES + 1)
                .read_line(&mut line);
            match read {
                Ok(0) => break,
                Ok(_) if line.len() as u64 <= MAX_LINE_BYTES => {}
                other => {
                    read_failed.store(true, Ordering::Relaxed);
                    let message = match other {
                        Err(error) => format!("Agent 输出读取失败：{error}"),
                        _ => "Agent 单条事件超过 1 MiB，已停止运行".into(),
                    };
                    let _ = read_sender.send(AgentEvent::Error { message });
                    break;
                }
            }
            if line.trim().is_empty() {
                continue;
            }
            match adapter.decode(line.trim_end()) {
                Ok(events) => {
                    for event in events {
                        saw_content |=
                            matches!(event, AgentEvent::Text { .. } | AgentEvent::Tool { .. });
                        if matches!(event, AgentEvent::Error { .. }) {
                            read_failed.store(true, Ordering::Relaxed);
                        }
                        if read_sender.send(event).is_err() {
                            return false;
                        }
                    }
                }
                Err(error) => {
                    read_failed.store(true, Ordering::Relaxed);
                    let _ = read_sender.send(AgentEvent::Error {
                        message: format!("Agent 返回了无效事件：{error}"),
                    });
                    break;
                }
            }
        }
        saw_content
    });
    let diagnostics = thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut tail = Vec::new();
        let mut buffer = [0; 4096];
        while let Ok(count) = reader.read(&mut buffer) {
            if count == 0 {
                break;
            }
            tail.extend_from_slice(&buffer[..count]);
            if tail.len() > MAX_DIAGNOSTIC_CHARS * 4 {
                tail.drain(..tail.len() - MAX_DIAGNOSTIC_CHARS * 4);
            }
        }
        String::from_utf8_lossy(&tail)
            .chars()
            .take(MAX_DIAGNOSTIC_CHARS)
            .collect::<String>()
    });

    let status = loop {
        if cancelled.load(Ordering::Relaxed) || failed.load(Ordering::Relaxed) {
            child.terminate();
        }
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        thread::sleep(Duration::from_millis(40));
    };
    // Close inherited pipes even if a tool outlives the Agent process.
    child.terminate();
    let _ = writer.join();
    let saw_content = reader.join().unwrap_or(false);
    let diagnostic = diagnostics.join().unwrap_or_default();
    if cancelled.load(Ordering::Relaxed) {
        return Ok(false);
    }
    if !status.success() && !failed.load(Ordering::Relaxed) {
        let _ = sender.send(AgentEvent::Error {
            message: format!("Agent 退出（{status}）\n{}", diagnostic.trim()),
        });
    } else if !diagnostic.trim().is_empty() {
        let _ = sender.send(AgentEvent::Notice {
            message: diagnostic.trim().to_owned(),
        });
    }
    if status.success() && !saw_content && !failed.load(Ordering::Relaxed) {
        let _ = sender.send(AgentEvent::Error {
            message: "Agent 未返回回复或工具事件。请检查登录、模型及输出协议。".into(),
        });
        return Ok(false);
    }
    Ok(status.success() && !failed.load(Ordering::Relaxed))
}
