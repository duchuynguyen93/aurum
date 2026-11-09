//! Small building blocks shared by the views.

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::theme;

/// Lucide icon from `assets/icons/<name>.svg`, tinted with `color`.
pub fn icon(name: &str, size: f32, color: u32) -> Svg {
    svg()
        .path(format!("icons/{name}.svg"))
        .size(px(size))
        .flex_none()
        .text_color(theme::color(color))
}

/// Quiet button: Obsidian800 fill with a hairline border.
pub fn secondary_button(
    id: impl Into<ElementId>,
    label: &str,
    glyph: &str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(36.))
        .px(px(14.))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .rounded(px(10.))
        .bg(rgb(theme::OBSIDIAN_800))
        .border_1()
        .border_color(rgb(theme::OBSIDIAN_700))
        .text_size(px(13.))
        .text_color(rgb(theme::IVORY_100))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(theme::OBSIDIAN_700)))
        .on_click(on_click)
        .child(icon(glyph, 16., theme::IVORY_300))
        .child(label.to_string())
}

/// Letter-spaced text. GPUI has no letter-spacing, so each letter is its own child.
pub fn spaced(text: &str, gap: f32) -> Div {
    div()
        .flex()
        .flex_row()
        .gap(px(gap))
        .children(text.chars().map(|c| div().child(c.to_string())))
}

/// View title in the display face.
pub fn view_title(text: &str) -> Div {
    div()
        .font_family(theme::FONT_DISPLAY)
        .text_size(px(22.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(theme::IVORY_50))
        .child(text.to_string())
}
