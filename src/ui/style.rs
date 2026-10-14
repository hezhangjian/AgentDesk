use gpui_kit::assets::IconName;
use gpui_kit::{component::Icon, *};

pub fn accent() -> Rgba {
    rgb(0x25675b)
}
pub fn border() -> Rgba {
    rgb(0xe8e9e5)
}
pub fn canvas() -> Rgba {
    rgb(0xfdfdfb)
}
pub fn ink() -> Rgba {
    rgb(0x272b29)
}
pub fn muted() -> Rgba {
    rgb(0x8b908a)
}
pub fn sidebar() -> Rgba {
    rgb(0xf3f4f0)
}

pub fn icon(name: IconName) -> Icon {
    Icon::new(name).size(px(17.))
}

pub fn row() -> Div {
    div().flex().items_center().gap_2()
}

pub fn label(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(11.))
        .text_color(muted())
        .child(text.into())
}
