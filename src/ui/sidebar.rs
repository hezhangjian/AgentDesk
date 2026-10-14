use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        Sizable,
        button::{Button, ButtonVariants},
    },
    *,
};

use super::{Desktop, style::*};

impl Desktop {
    pub(super) fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut sessions = self.workspace.sessions.iter().collect::<Vec<_>>();
        sessions.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        div()
            .w(px(248.))
            .flex_shrink_0()
            .h_full()
            .bg(sidebar())
            .border_r_1()
            .border_color(border())
            .flex()
            .flex_col()
            .px_3()
            .pt_5()
            .pb_3()
            .child(
                row()
                    .px_3()
                    .mb_6()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(29.))
                            .rounded_lg()
                            .bg(ink())
                            .text_color(rgb(0xffffff))
                            .child(icon(IconName::Command)),
                    )
                    .child(
                        div()
                            .text_size(px(19.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("AgentDesk"),
                    )
                    .child(label("BETA")),
            )
            .child(
                Button::new("new-chat")
                    .ghost()
                    .w_full()
                    .h(px(42.))
                    .justify_start()
                    .icon(IconName::SquarePen)
                    .label("新建会话")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.select(this.project_id.clone(), None, window, cx)
                    })),
            )
            .child(
                div()
                    .id("sidebar-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .mt_7()
                    .child(
                        row()
                            .justify_between()
                            .px_3()
                            .mb_2()
                            .child(label("项目"))
                            .child(
                                Button::new("add-project")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Plus)
                                    .accessibility_label("添加项目文件夹")
                                    .tooltip("添加项目文件夹")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.add_project(window, cx)
                                    })),
                            ),
                    )
                    .when(self.workspace.projects.is_empty(), |view| {
                        view.child(
                            div()
                                .px_3()
                                .py_2()
                                .text_size(px(12.))
                                .text_color(muted())
                                .child("添加一个文件夹，开始工作。"),
                        )
                    })
                    .children(self.workspace.projects.iter().map(|project| {
                        let id = project.id.clone();
                        let selected = self.project_id.as_ref() == Some(&id);
                        div()
                            .id(SharedString::from(format!("project-{id}")))
                            .role(Role::Button)
                            .aria_label(project.name.clone())
                            .mx_1()
                            .px_3()
                            .py_2()
                            .rounded_lg()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0xe9ebe5)))
                            .when(selected && self.session_id.is_none(), |s| {
                                s.bg(rgb(0xe6e9e1))
                            })
                            .child(
                                row()
                                    .gap_3()
                                    .child(icon(if selected {
                                        IconName::FolderOpen
                                    } else {
                                        IconName::Folder
                                    }))
                                    .child(div().min_w_0().truncate().child(project.name.clone())),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select(Some(id.clone()), None, window, cx)
                            }))
                    }))
                    .child(
                        row()
                            .px_3()
                            .mt_8()
                            .mb_3()
                            .justify_between()
                            .child(label("最近会话"))
                            .child(label(format!("{}", sessions.len()))),
                    )
                    .when(sessions.is_empty(), |view| {
                        view.child(
                            div()
                                .px_3()
                                .text_size(px(12.))
                                .text_color(muted())
                                .child("你的会话会保存在这里"),
                        )
                    })
                    .children(sessions.into_iter().map(|session| {
                        let id = session.id.clone();
                        let project_id = session.project_id.clone();
                        let selected = self.session_id.as_ref() == Some(&id);
                        let running = self.runs.contains_key(&id);
                        let project = self
                            .workspace
                            .projects
                            .iter()
                            .find(|p| p.id == project_id)
                            .map(|p| p.name.clone())
                            .unwrap_or_default();
                        div()
                            .id(SharedString::from(format!("session-{id}")))
                            .role(Role::Button)
                            .aria_label(session.title.clone())
                            .mx_1()
                            .px_3()
                            .py_3()
                            .rounded_lg()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0xe9ebe5)))
                            .when(selected, |s| s.bg(rgb(0xe4e8df)))
                            .child(
                                row()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_size(px(13.))
                                            .child(session.title.clone()),
                                    )
                                    .when(running, |s| {
                                        s.child(div().size(px(6.)).rounded_full().bg(accent()))
                                    }),
                            )
                            .child(div().mt_1().truncate().child(label(project)))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select(Some(project_id.clone()), Some(id.clone()), window, cx)
                            }))
                    })),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(border())
                    .pt_3()
                    .mt_3()
                    .child(
                        Button::new("agent-settings")
                            .ghost()
                            .w_full()
                            .justify_start()
                            .icon(IconName::Settings)
                            .label("Agent 设置")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_settings(window, cx)),
                            ),
                    )
                    .child(
                        row()
                            .px_3()
                            .pt_3()
                            .pb_1()
                            .gap_2()
                            .child(div().size(px(5.)).rounded_full().bg(muted()))
                            .child(label("本地工作区"))
                            .child(div().flex_1())
                            .child(label("v0.0.1")),
                    ),
            )
    }
}
