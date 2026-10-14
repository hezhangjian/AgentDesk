use agentdesk::model::{Entry, EntryKind};
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        input::Textarea,
        text::TextView,
    },
    *,
};

use super::{Desktop, style::*};

impl Desktop {
    pub(super) fn chat(&self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session = self.current_session();
        let project = self
            .workspace
            .projects
            .iter()
            .find(|p| Some(&p.id) == self.project_id.as_ref());
        let running = self
            .session_id
            .as_ref()
            .is_some_and(|id| self.runs.contains_key(id));
        let agent = session.map(|s| &s.agent).unwrap_or(&self.workspace.agent);
        let has_messages = session.is_some_and(|s| !s.entries.is_empty());
        let status = if running {
            "正在执行"
        } else {
            session.map(|s| s.status.label()).unwrap_or("准备就绪")
        };
        let session_id = self.session_id.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                row()
                    .h(px(72.))
                    .flex_shrink_0()
                    .px_7()
                    .justify_between()
                    .border_b_1()
                    .border_color(border())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(
                                        session
                                            .map(|s| s.title.clone())
                                            .unwrap_or_else(|| "新会话".into()),
                                    ),
                            )
                            .child(
                                div().mt_1().truncate().child(label(
                                    project
                                        .map(|p| p.path.display().to_string())
                                        .unwrap_or_else(|| {
                                            "选择一个项目，给任务一个工作空间".into()
                                        }),
                                )),
                            ),
                    )
                    .child(
                        row()
                            .ml_4()
                            .gap_2()
                            .text_color(muted())
                            .child(icon(IconName::Cpu))
                            .child(div().text_size(px(12.)).child(agent.label())),
                    ),
            )
            .child(
                div()
                    .id("conversation")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .px_8()
                    .child(
                        div()
                            .w_full()
                            .max_w(px(820.))
                            .mx_auto()
                            .when(!has_messages, |view| view.child(self.welcome(cx)))
                            .when(has_messages, |view| {
                                view.py_8()
                                    .flex()
                                    .flex_col()
                                    .gap_7()
                                    .children(
                                        session
                                            .into_iter()
                                            .flat_map(|s| s.entries.iter())
                                            .map(|entry| self.message(entry, cx)),
                                    )
                                    .when(running, |view| {
                                        view.child(
                                            row()
                                                .gap_3()
                                                .text_color(accent())
                                                .child(
                                                    div().size(px(7.)).rounded_full().bg(accent()),
                                                )
                                                .child(div().text_size(px(13.)).child(
                                                    "Agent 正在工作，回复和工具结果会显示在这里…",
                                                )),
                                        )
                                    })
                            }),
                    ),
            )
            .child(
                div().flex_shrink_0().px_8().pt_3().pb_5().child(
                    div()
                        .w_full()
                        .max_w(px(820.))
                        .mx_auto()
                        .child(
                            row()
                                .justify_between()
                                .mb_3()
                                .child(
                                    row()
                                        .text_size(px(11.))
                                        .text_color(muted())
                                        .gap_2()
                                        .child(div().size(px(5.)).rounded_full().bg(if running {
                                            accent()
                                        } else {
                                            muted()
                                        }))
                                        .child(status),
                                )
                                .when(has_messages, |view| {
                                    view.child(
                                        Button::new("scroll-bottom")
                                            .ghost()
                                            .xsmall()
                                            .icon(IconName::ArrowDown)
                                            .accessibility_label("回到最新消息")
                                            .tooltip("回到最新消息")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.scroll.scroll_to_bottom();
                                                cx.notify();
                                            })),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .rounded_2xl()
                                .border_1()
                                .border_color(rgb(0xdfe3da))
                                .bg(rgb(0xffffff))
                                .px_4()
                                .pt_3()
                                .pb_3()
                                .child(
                                    Textarea::new(&self.composer)
                                        .accessibility_id("composer")
                                        .appearance(false)
                                        .bordered(false)
                                        .disabled(self.storage_blocked)
                                        .w_full(),
                                )
                                .child(
                                    row()
                                        .mt_2()
                                        .justify_between()
                                        .child(
                                            row()
                                                .gap_2()
                                                .text_size(px(12.))
                                                .text_color(muted())
                                                .child(icon(IconName::Folder))
                                                .child(
                                                    project
                                                        .map(|p| p.name.clone())
                                                        .unwrap_or_else(|| "发送时选择项目".into()),
                                                ),
                                        )
                                        .child(if running {
                                            Button::new("stop-run")
                                                .label("停止")
                                                .icon(IconName::Square)
                                                .small()
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    if let Some(run) = session_id
                                                        .as_ref()
                                                        .and_then(|id| this.runs.get(id))
                                                    {
                                                        run.cancel();
                                                    }
                                                    cx.notify();
                                                }))
                                                .into_any_element()
                                        } else {
                                            Button::new("send-message")
                                                .accessibility_label("发送消息")
                                                .primary()
                                                .icon(IconName::ArrowUp)
                                                .small()
                                                .tooltip(if project.is_none() {
                                                    "选择项目并发送"
                                                } else {
                                                    "发送消息"
                                                })
                                                .disabled(
                                                    self.storage_blocked
                                                        || self
                                                            .composer
                                                            .read(cx)
                                                            .value()
                                                            .trim()
                                                            .is_empty(),
                                                )
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.send(window, cx)
                                                }))
                                                .into_any_element()
                                        }),
                                ),
                        )
                        .child(
                            row()
                                .justify_between()
                                .mt_3()
                                .child(label("Enter 发送 · Shift + Enter 换行"))
                                .child(label("由你的 Agent 执行 · 会话保存在本机")),
                        ),
                ),
            )
    }

    fn welcome(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let has_project = self.project_id.is_some();
        div().pt(px(100.)).pb_8().flex().flex_col().items_center()
            .child(div().size(px(54.)).rounded_2xl().bg(rgb(0xe9eee5)).text_color(accent()).flex().items_center().justify_center()
                .child(gpui_kit::component::Icon::new(IconName::Command).size(px(28.))))
            .child(div().mt_7().text_size(px(30.)).font_weight(FontWeight::MEDIUM).child("想一起完成什么？"))
            .child(div().mt_3().text_size(px(14.)).text_color(muted()).child("一个桌面，连接你的项目与 Agent。"))
            .when(!has_project, |view| view.child(Button::new("welcome-add-project").mt_7().label("添加项目文件夹").icon(IconName::Plus)
                .on_click(cx.listener(|this, _, window, cx| this.add_project(window, cx)))))
            .when(has_project, |view| view.child(div().flex().gap_3().mt_9().w_full().max_w(px(650.))
                .children([
                    ("understand", IconName::Folder, "了解项目", "梳理结构与关键模块", "请先阅读这个项目，梳理目录结构、技术栈和关键模块，不要修改文件。"),
                    ("review", IconName::MessageSquare, "检查代码", "发现潜在问题", "请检查这个项目的代码，列出值得优先修复的问题和依据，先不要修改文件。"),
                    ("build", IconName::Terminal, "运行项目", "确认启动与验证方式", "请阅读项目文档和配置，告诉我如何启动、构建和测试这个项目。"),
                ].into_iter().map(|(id, name, title, subtitle, prompt)| {
                    div().id(id).flex_1().min_w_0().p_4().rounded_xl().border_1().border_color(border()).cursor_pointer()
                        .hover(|s| s.bg(rgb(0xf3f5ef)).border_color(rgb(0xd6ddd0)))
                        .child(div().text_color(accent()).child(icon(name)))
                        .child(div().mt_3().text_size(px(13.)).child(title))
                        .child(div().mt_1().child(label(subtitle)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.composer.update(cx, |input, cx| { input.set_value(prompt, window, cx); input.focus(window, cx); });
                        }))
                }))))
    }

    fn message(&self, entry: &Entry, cx: &mut Context<Self>) -> AnyElement {
        match entry.kind {
            EntryKind::User => row()
                .justify_end()
                .child(
                    div()
                        .id(SharedString::from(format!("user-{}", entry.id)))
                        .role(Role::Group)
                        .aria_label(entry.text.clone())
                        .max_w(relative(0.85))
                        .rounded_2xl()
                        .px_5()
                        .py_3()
                        .bg(rgb(0xedf0e9))
                        .child(
                            TextView::markdown(
                                SharedString::from(entry.id.clone()),
                                entry.text.clone(),
                            )
                            .selectable(true),
                        ),
                )
                .into_any_element(),
            EntryKind::Assistant => div()
                .id(SharedString::from(format!("assistant-{}", entry.id)))
                .role(Role::Group)
                .aria_label(entry.text.clone())
                .w_full()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    row()
                        .text_color(accent())
                        .child(icon(IconName::Command))
                        .child(
                            div()
                                .text_size(px(12.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(
                                    self.current_session()
                                        .map(|s| s.agent.label())
                                        .unwrap_or("Agent"),
                                ),
                        ),
                )
                .child(
                    TextView::markdown(SharedString::from(entry.id.clone()), entry.text.clone())
                        .selectable(true),
                )
                .child(
                    Button::new(SharedString::from(format!("copy-{}", entry.id)))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Copy)
                        .accessibility_label("复制回复")
                        .tooltip("复制回复")
                        .on_click({
                            let text = entry.text.clone();
                            move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))
                            }
                        }),
                )
                .into_any_element(),
            EntryKind::Tool => {
                let id = entry.id.clone();
                let expanded = self.expanded.contains(&id);
                div()
                    .rounded_lg()
                    .border_1()
                    .border_color(border())
                    .overflow_hidden()
                    .child(
                        div()
                            .id(SharedString::from(format!("tool-{id}")))
                            .role(Role::Button)
                            .aria_label(entry.text.clone())
                            .px_4()
                            .py_3()
                            .cursor_pointer()
                            .bg(rgb(0xf5f6f2))
                            .child(
                                row()
                                    .text_color(if entry.failed { rgb(0xb34a37) } else { muted() })
                                    .child(icon(if expanded {
                                        IconName::ChevronDown
                                    } else {
                                        IconName::ChevronRight
                                    }))
                                    .child(icon(IconName::Terminal))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_size(px(12.))
                                            .child(entry.text.clone()),
                                    )
                                    .child(icon(if entry.failed {
                                        IconName::X
                                    } else {
                                        IconName::Check
                                    })),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !this.expanded.remove(&id) {
                                    this.expanded.insert(id.clone());
                                }
                                cx.notify();
                            })),
                    )
                    .when(expanded, |view| {
                        view.child(
                            div()
                                .id(SharedString::from(format!("detail-{}", entry.id)))
                                .max_h(px(320.))
                                .overflow_y_scroll()
                                .p_4()
                                .text_size(px(12.))
                                .child(
                                    TextView::markdown(
                                        SharedString::from(format!("detail-text-{}", entry.id)),
                                        fenced(&entry.details),
                                    )
                                    .selectable(true),
                                ),
                        )
                    })
                    .into_any_element()
            }
            EntryKind::Error | EntryKind::Notice => div()
                .id(SharedString::from(format!("notice-{}", entry.id)))
                .role(Role::Group)
                .aria_label(entry.text.clone())
                .px_4()
                .py_3()
                .rounded_lg()
                .bg(if entry.kind == EntryKind::Error {
                    rgb(0xfff1ed)
                } else {
                    rgb(0xf3f4ef)
                })
                .text_color(if entry.kind == EntryKind::Error {
                    rgb(0xa23e2c)
                } else {
                    muted()
                })
                .text_size(px(12.))
                .child(
                    TextView::markdown(SharedString::from(entry.id.clone()), fenced(&entry.text))
                        .selectable(true),
                )
                .into_any_element(),
        }
    }
}

fn fenced(text: &str) -> String {
    let fence = "`".repeat(
        text.split(|c| c != '`')
            .map(str::len)
            .max()
            .unwrap_or(0)
            .max(2)
            + 1,
    );
    format!("{fence}text\n{text}\n{fence}")
}
