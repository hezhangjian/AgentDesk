use agentdesk::agent::{AgentConfig, AgentKind};
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        input::{Input, InputState},
    },
    *,
};

use super::{Desktop, style::*};

pub(super) struct SettingsForm {
    kind: AgentKind,
    executable: Entity<InputState>,
    arguments: Entity<InputState>,
    model: Entity<InputState>,
}

impl Desktop {
    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let config = self.workspace.agent.clone();
        self.settings = Some(SettingsForm {
            kind: config.kind,
            executable: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(config.executable)
                    .placeholder("opencode 或可执行文件的绝对路径")
            }),
            arguments: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(
                        serde_json::to_string(&config.arguments).unwrap_or_else(|_| "[]".into()),
                    )
                    .placeholder("[]")
            }),
            model: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(config.model)
                    .placeholder("留空使用 Agent 默认模型")
            }),
        });
        cx.notify();
    }

    fn save_settings(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = &self.settings else {
            return;
        };
        let arguments = if form.kind == AgentKind::Custom {
            match serde_json::from_str::<Vec<String>>(&form.arguments.read(cx).value()) {
                Ok(arguments) => arguments,
                Err(_) => {
                    self.error = Some("参数必须是 JSON 字符串数组，例如 [\"--stdio\"]。".into());
                    cx.notify();
                    return;
                }
            }
        } else {
            vec![]
        };
        let config = AgentConfig {
            kind: form.kind.clone(),
            executable: form.executable.read(cx).value().trim().into(),
            arguments,
            model: form.model.read(cx).value().trim().into(),
        };
        if let Err(error) = config.validate() {
            self.error = Some(error.to_string());
            cx.notify();
            return;
        }
        let previous = self.workspace.agent.clone();
        self.workspace.agent = config;
        if self.save() {
            self.error = None;
            self.settings = None;
        } else {
            self.workspace.agent = previous;
        }
        cx.notify();
    }

    pub(super) fn settings_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let form = self.settings.as_ref().unwrap();
        let custom = form.kind == AgentKind::Custom;
        div().id("settings-content").size_full().overflow_y_scroll().px_8().py_8()
            .child(div().w_full().max_w(px(620.)).mx_auto()
                .child(row().justify_between()
                    .child(div().text_size(px(25.)).font_weight(FontWeight::MEDIUM).child("连接你的 Agent"))
                    .child(Button::new("close-settings").ghost().small().icon(IconName::X).on_click(cx.listener(|this, _, _, cx| { this.settings = None; cx.notify(); }))))
                .child(div().mt_3().text_color(muted()).child("AgentDesk 管理工作空间，Agent 负责执行任务。"))
                .child(row().mt_8().gap_3().children([
                    ("opencode-provider", AgentKind::OpenCode, "OpenCode"),
                    ("custom-provider", AgentKind::Custom, "企业私有 Agent"),
                ].into_iter().map(|(id, kind, title)| {
                    let selected = form.kind == kind;
                    Button::new(id).label(title).when(selected, |button| button.primary())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if let Some(form) = &mut this.settings
                                && form.kind != kind {
                                    form.kind = kind.clone();
                                    form.executable.update(cx, |input, cx| input.set_value(if kind == AgentKind::OpenCode { "opencode" } else { "" }, window, cx));
                            }
                            cx.notify();
                        }))
                })))
                .child(div().mt_7().mb_2().child("可执行文件"))
                .child(Input::new(&form.executable).id("agent-executable").w_full())
                .child(div().mt_2().child(label("使用命令名称或绝对路径；不需要填写 shell 命令。")))
                .when(custom, |view| view
                    .child(div().mt_6().mb_2().child("启动参数"))
                    .child(Input::new(&form.arguments).id("agent-arguments").w_full())
                    .child(div().mt_2().child(label("JSON 数组，例如 [\"--stdio\"]。每项作为一个独立参数传入。"))))
                .child(div().mt_6().mb_2().child("模型（可选）"))
                .child(Input::new(&form.model).id("agent-model").w_full())
                .child(div().mt_6().p_4().rounded_lg().bg(sidebar()).text_size(px(12.)).text_color(muted())
                    .child(if custom {
                        "企业 Agent 通过本地适配程序连接：从标准输入接收请求，向标准输出逐行返回 JSON 事件。协议与示例见 docs/agent-protocol.md。"
                    } else {
                        "复用本机 OpenCode 的登录、模型与权限配置。请先在终端完成 opencode auth login。需要交互确认的操作会由 OpenCode 拒绝，可在其权限配置中明确允许需要的操作。"
                    }))
                .child(div().mt_5().child(label("保存后用于新会话；已有会话继续使用创建时的 Agent 配置。")))
                .child(row().mt_8().justify_end()
                    .child(Button::new("cancel-settings").ghost().label("取消").on_click(cx.listener(|this, _, _, cx| { this.settings = None; cx.notify(); })))
                    .child(Button::new("save-settings").primary().label("保存设置").disabled(self.storage_blocked)
                        .on_click(cx.listener(|this, _, window, cx| this.save_settings(window, cx))))))
    }
}
