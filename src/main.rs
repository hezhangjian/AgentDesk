mod ui;

use gpui_kit::{
    assets::AllAssets,
    component::{Theme, ThemeMode},
    *,
};

actions!(agentdesk, [Quit]);

fn main() {
    gpui_kit::application().with_assets(AllAssets).run(|cx| {
        gpui_kit::init(cx);
        Theme::change(ThemeMode::Light, None, cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(1220.), px(820.)), cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(840.), px(620.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("AgentDesk".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| ui::Desktop::new(window, cx)),
        )
        .expect("failed to open AgentDesk");
        cx.activate(true);
    });
}
