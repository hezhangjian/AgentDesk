use gpui_kit::prelude::FluentBuilder;
mod chat;
mod settings;
mod sidebar;
mod style;

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use agentdesk::{
    agent::{self, AgentEvent, AgentRun, RunRequest},
    model::{Session, SessionStatus, Workspace},
    storage::Storage,
};
use gpui_kit::{
    component::input::{InputEvent, TextareaState},
    *,
};

use settings::SettingsForm;

pub struct Desktop {
    workspace: Workspace,
    storage: Option<Storage>,
    storage_blocked: bool,
    error: Option<String>,
    project_id: Option<String>,
    session_id: Option<String>,
    composer: Entity<TextareaState>,
    new_drafts: HashMap<Option<String>, String>,
    runs: HashMap<String, AgentRun>,
    expanded: HashSet<String>,
    scroll: ScrollHandle,
    settings: Option<SettingsForm>,
    dirty: bool,
    last_save: Instant,
    subscriptions: Vec<Subscription>,
}

impl Desktop {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let storage = Storage::discover();
        let (storage, workspace, error) = match storage {
            Ok(storage) => match storage.load() {
                Ok(workspace) => (Some(storage), workspace, None),
                Err(error) => (
                    Some(storage),
                    Workspace::default(),
                    Some(format!("{error:#}。请修复数据文件后重启。")),
                ),
            },
            Err(error) => (None, Workspace::default(), Some(error.to_string())),
        };
        let composer = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("描述你想完成的任务…")
                .auto_grow(2, 7)
                .submit_on_enter(true)
        });
        let input_subscription = cx.subscribe_in(
            &composer,
            window,
            |this, _, event, window, cx| match event {
                InputEvent::PressEnter { shift: false, .. } if this.settings.is_none() => {
                    this.send(window, cx)
                }
                InputEvent::Change => {
                    this.capture_draft(cx);
                    this.dirty = true;
                    cx.notify();
                }
                _ => {}
            },
        );
        let shutdown = cx.on_app_quit(|this, cx| {
            this.capture_draft(cx);
            for run in this.runs.values() {
                run.cancel();
            }
            for session in &mut this.workspace.sessions {
                if this.runs.contains_key(&session.id) {
                    session.status = SessionStatus::Interrupted;
                }
            }
            this.runs.clear();
            this.save();
            let timer = cx.background_executor().timer(Duration::from_millis(200));
            async move {
                timer.await;
            }
        });
        cx.spawn(async |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        let project_id = workspace.projects.first().map(|p| p.id.clone());
        let storage_blocked = error.is_some();
        Self {
            workspace,
            storage,
            storage_blocked,
            error,
            project_id,
            session_id: None,
            composer,
            new_drafts: HashMap::new(),
            runs: HashMap::new(),
            expanded: HashSet::new(),
            scroll: ScrollHandle::new(),
            settings: None,
            dirty: false,
            last_save: Instant::now(),
            subscriptions: vec![input_subscription, shutdown],
        }
    }

    fn current_session(&self) -> Option<&Session> {
        self.session_id
            .as_ref()
            .and_then(|id| self.workspace.sessions.iter().find(|s| &s.id == id))
    }

    fn save(&mut self) -> bool {
        if self.storage_blocked {
            return false;
        }
        self.last_save = Instant::now();
        if let Some(storage) = &self.storage
            && let Err(error) = storage.save(&self.workspace)
        {
            self.error = Some(format!("无法保存会话：{error:#}"));
            return false;
        }
        self.dirty = false;
        self.last_save = Instant::now();
        true
    }

    fn capture_draft(&mut self, cx: &App) {
        let value = self.composer.read(cx).value().to_string();
        if let Some(id) = &self.session_id {
            if let Some(session) = self.workspace.sessions.iter_mut().find(|s| &s.id == id) {
                session.draft = value;
            }
        } else {
            self.new_drafts.insert(self.project_id.clone(), value);
        }
    }

    fn select(
        &mut self,
        project_id: Option<String>,
        session_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.capture_draft(cx);
        if self.project_id.is_none()
            && project_id.is_some()
            && session_id.is_none()
            && let Some(draft) = self.new_drafts.remove(&None)
        {
            self.new_drafts.insert(project_id.clone(), draft);
        }
        self.project_id = project_id;
        self.session_id = session_id;
        self.settings = None;
        let draft = self
            .current_session()
            .map(|s| s.draft.clone())
            .unwrap_or_else(|| {
                self.new_drafts
                    .get(&self.project_id)
                    .cloned()
                    .unwrap_or_default()
            });
        self.composer.update(cx, |input, cx| {
            input.set_value(draft, window, cx);
            input.focus(window, cx);
        });
        self.scroll.scroll_to_bottom();
        self.dirty = true;
        cx.notify();
    }

    fn add_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.choose_project(false, window, cx);
    }

    fn choose_project(
        &mut self,
        send_after_selection: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.storage_blocked {
            return;
        }
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("选择项目文件夹".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = paths.await;
            let _ = this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.first() {
                            match this.workspace.add_project(path.clone()) {
                                Ok(id) => {
                                    this.select(Some(id), None, window, cx);
                                    if this.save() && send_after_selection {
                                        this.send(window, cx);
                                    }
                                }
                                Err(error) => this.error = Some(error.to_string()),
                            }
                        }
                    }
                    Ok(Err(error)) => this.error = Some(error.to_string()),
                    _ => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.storage_blocked || self.settings.is_some() {
            return;
        }
        let prompt = self.composer.read(cx).value().trim().to_owned();
        if prompt.is_empty() {
            return;
        }
        if self
            .session_id
            .as_ref()
            .is_some_and(|id| self.runs.contains_key(id))
        {
            return;
        }
        if prompt.len() > 128 * 1024 {
            self.error = Some("消息过长，请控制在 128 KiB 以内。".into());
            cx.notify();
            return;
        }
        if self.session_id.is_none() {
            let Some(project) = self
                .workspace
                .projects
                .iter()
                .find(|p| Some(&p.id) == self.project_id.as_ref())
            else {
                self.choose_project(true, window, cx);
                return;
            };
            let session = Session::new(project, self.workspace.agent.clone(), &prompt);
            self.session_id = Some(session.id.clone());
            self.workspace.sessions.push(session);
        }
        let id = self.session_id.clone().unwrap();
        let session = self
            .workspace
            .sessions
            .iter_mut()
            .find(|s| s.id == id)
            .unwrap();
        if let Err(error) = session.agent.validate() {
            self.error = Some(error.to_string());
            cx.notify();
            return;
        }
        if !session.directory.is_dir() {
            self.error = Some(format!("项目目录不存在：{}", session.directory.display()));
            cx.notify();
            return;
        }
        // A missing remote ID after a previous response must not silently lose context.
        if session.remote_session_id.is_none()
            && session.entries.iter().any(|e| {
                matches!(
                    e.kind,
                    agentdesk::model::EntryKind::Assistant | agentdesk::model::EntryKind::Tool
                )
            })
        {
            self.error = Some(
                "Agent 未提供会话 ID，无法可靠续接。请新建会话，并检查 Agent 的会话事件。".into(),
            );
            cx.notify();
            return;
        }
        let request = RunRequest {
            directory: session.directory.clone(),
            model: session.agent.model.clone(),
            prompt: prompt.clone(),
            session_id: session.remote_session_id.clone(),
        };
        let config = session.agent.clone();
        let previous = session.clone();
        session.begin(prompt);
        if !self.save() {
            *self
                .workspace
                .sessions
                .iter_mut()
                .find(|s| s.id == id)
                .unwrap() = previous;
            cx.notify();
            return;
        }
        self.error = None;
        self.runs.insert(id, agent::launch(config, request));
        self.new_drafts.remove(&self.project_id);
        self.composer
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.scroll.scroll_to_bottom();
        cx.notify();
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let follow = (self.scroll.max_offset().y + self.scroll.offset().y).abs() < px(70.);
        let mut events = vec![];
        for (id, run) in &self.runs {
            for _ in 0..128 {
                match run.events.try_recv() {
                    Ok(event) => events.push((id.clone(), event)),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        if !events.iter().any(|(session_id, event)| {
                            session_id == id && matches!(event, AgentEvent::Finished { .. })
                        }) {
                            events.push((
                                id.clone(),
                                AgentEvent::Error {
                                    message: "Agent 连接意外断开".into(),
                                },
                            ));
                            events.push((
                                id.clone(),
                                AgentEvent::Finished {
                                    cancelled: false,
                                    success: false,
                                },
                            ));
                        }
                        break;
                    }
                }
            }
        }
        let changed = !events.is_empty();
        let mut finished = false;
        for (id, event) in events {
            if matches!(event, AgentEvent::Finished { .. }) {
                self.runs.remove(&id);
                finished = true;
            }
            if let Some(session) = self.workspace.sessions.iter_mut().find(|s| s.id == id) {
                session.apply(event);
            }
            self.dirty = true;
        }
        if self.dirty
            && (finished || self.last_save.elapsed() > Duration::from_millis(750))
            && !self.save()
        {
            cx.notify();
        }
        if changed {
            if follow {
                self.scroll.scroll_to_bottom();
            }
            cx.notify();
        }
    }
}

impl Render for Desktop {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = &self.subscriptions;
        div()
            .size_full()
            .flex()
            .bg(style::canvas())
            .text_color(style::ink())
            .text_size(px(14.))
            .child(self.sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .when_some(self.error.clone(), |view, error| {
                        view.child(
                            div()
                                .px_6()
                                .py_3()
                                .bg(rgb(0xfff1ed))
                                .text_color(rgb(0xa23e2c))
                                .text_size(px(12.))
                                .child(error),
                        )
                    })
                    .child(if self.settings.is_some() {
                        self.settings_view(cx).into_any_element()
                    } else {
                        self.chat(window, cx).into_any_element()
                    }),
            )
    }
}
