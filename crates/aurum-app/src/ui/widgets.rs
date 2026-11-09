//! Small building blocks shared by the views.

use gpui_kit::component::input::Input;
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::state::app_state::AppState;
use crate::state::cover_cache::Lookup;
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

/// Tooltip builder for `.tooltip(...)`.
pub fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text: SharedString = text.into();
    move |window, cx| Tooltip::new(text.clone()).build(window, cx)
}

/// Soft drop shadow of a cover; `scale` 1.0 is the large header cover (0 24 48, 45%).
pub fn cover_shadow(scale: f32) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: theme::color(0x000000).opacity(0.45),
        offset: point(px(0.), px(24. * scale)),
        blur_radius: px(48. * scale),
        spread_radius: px(0.),
        inset: false,
    }]
}

/// Fills its parent with the cover, a quiet disc icon when the album has none, or a shimmer
/// while the picture is still being read.
pub fn cover_fill(lookup: Lookup, icon_size: f32) -> AnyElement {
    match lookup {
        Lookup::Ready(art) => img(art.thumb)
            .size_full()
            .object_fit(ObjectFit::Cover)
            .into_any_element(),
        Lookup::Missing => div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(icon("disc-3", icon_size, theme::IVORY_500))
            .into_any_element(),
        Lookup::Loading => Skeleton::new().size_full().into_any_element(),
    }
}

/// Round back button of the detail pages.
pub fn back_button(cx: &mut Context<AppState>) -> Stateful<Div> {
    div()
        .id("back")
        .size(px(36.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .cursor_pointer()
        .hover(|s| s.bg(rgb(theme::OBSIDIAN_800)))
        .tooltip(tip("Back"))
        .on_click(cx.listener(|s, _, _, cx| s.back(cx)))
        .child(icon("arrow-left", 18., theme::IVORY_300))
}

/// Gold call to action (play, add folder style): the only filled button.
pub fn gold_button(id: impl Into<ElementId>, label: &str, glyph: &str) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(40.))
        .px(px(20.))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .rounded(px(10.))
        .bg(rgb(theme::GOLD_500))
        .text_color(rgb(theme::OBSIDIAN_950))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(|s| s.bg(rgb(theme::GOLD_400)))
        .active(|s| s.bg(rgb(theme::GOLD_700)))
        .child(icon(glyph, 16., theme::OBSIDIAN_950))
        .child(label.to_string())
}

/// Outlined button next to a gold one.
pub fn outline_button(id: impl Into<ElementId>, label: &str, glyph: &str) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(40.))
        .px(px(20.))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .rounded(px(10.))
        .border_1()
        .border_color(rgb(theme::OBSIDIAN_600))
        .text_color(rgb(theme::IVORY_100))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(theme::OBSIDIAN_800)))
        .child(icon(glyph, 16., theme::IVORY_300))
        .child(label.to_string())
}

impl AppState {
    /// Search box (gpui-component input) for the top right of a view header.
    pub(super) fn search_box(&self) -> impl IntoElement {
        div().w(px(260.)).flex_none().child(
            Input::new(&self.search_input)
                .cleanable(true)
                .prefix(icon("search", 16., theme::IVORY_500))
                .h(px(36.)),
        )
    }

    /// Header row of a list view: `left` on the left, the search box on the right.
    pub(super) fn view_header(&self, left: impl IntoElement) -> Div {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(16.))
            .child(left)
            .child(self.search_box())
    }
}
