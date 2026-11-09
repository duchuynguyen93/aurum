//! Window shell: sidebar, main area and the Now Playing bar. `AppState` is the root view.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::component::{InteractiveElementExt as _, TitleBar};
use gpui_kit::prelude::*;
use gpui_kit::*;

use super::albums::SIDEBAR_WIDTH;
use super::widgets::{icon, spaced, tip};
use crate::state::app_state::{AppState, Detail, View};
use crate::theme;

actions!(
    aurum,
    [
        TogglePlay,
        NextTrack,
        PrevTrack,
        OpenSettings,
        FocusSearch,
        ClearSearch
    ]
);

const TITLE_BAR_HEIGHT: f32 = 34.;

/// Invisible strip that moves the window. The window owns no system title bar (see
/// `main.rs`), so it does the same as gpui-component's `TitleBar`: a press arms the move, the
/// first mouse move starts it, a double click zooms.
fn drag_strip(armed: Rc<Cell<bool>>) -> Stateful<Div> {
    let (down, moved, up) = (armed.clone(), armed.clone(), armed);
    div()
        .id("drag-strip")
        .window_control_area(WindowControlArea::Drag)
        .on_double_click(|_, window, _| {
            if cfg!(target_os = "macos") {
                window.titlebar_double_click();
            } else {
                window.zoom_window();
            }
        })
        .on_mouse_down(MouseButton::Left, move |_, _, _| down.set(true))
        .on_mouse_up(MouseButton::Left, move |_, _, _| up.set(false))
        .on_mouse_move(move |_, window, _| {
            if moved.replace(false) {
                window.start_window_move();
            }
        })
}

impl Render for AppState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_roots = !self.settings.roots.is_empty();
        let top = self.detail.last().cloned();
        let main = if self.view == View::Settings {
            self.render_settings(cx).into_any_element()
        } else if !has_roots {
            self.render_empty(cx).into_any_element()
        } else {
            match (top, self.view) {
                (Some(Detail::Album(d)), _) => self.render_album_detail(d, cx).into_any_element(),
                (Some(Detail::Artist(name)), _) => self
                    .render_artist_detail(&name, window, cx)
                    .into_any_element(),
                (None, View::Albums) => self.render_albums(window, cx).into_any_element(),
                (None, View::Artists) => self.render_artists(cx).into_any_element(),
                (None, View::Tracks) | (None, View::Settings) => {
                    self.render_tracks(cx).into_any_element()
                }
            }
        };
        // The album page paints its own blurred backdrop up to the top edge.
        let bar_bg = if self.view != View::Settings
            && has_roots
            && matches!(self.detail.last(), Some(Detail::Album(_)))
        {
            transparent_black()
        } else {
            theme::color(theme::OBSIDIAN_950)
        };

        div()
            .key_context("Aurum")
            .track_focus(&self.focus)
            // Shortcuts are bound on the "Aurum" context, which also contains the search box.
            // While the box has focus they must not act, so the keystroke reaches the text.
            .on_action(cx.listener(|s, _: &TogglePlay, window, cx| {
                if s.focus.is_focused(window) {
                    s.toggle_play(cx);
                } else {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|s, _: &NextTrack, window, cx| {
                if s.focus.is_focused(window) {
                    s.next(cx);
                } else {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|s, _: &PrevTrack, window, cx| {
                if s.focus.is_focused(window) {
                    s.prev(cx);
                } else {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|s, _: &OpenSettings, _, cx| s.set_view(View::Settings, cx)))
            .on_action(cx.listener(|s, _: &FocusSearch, window, cx| s.focus_search(window, cx)))
            .on_action(cx.listener(|s, _: &ClearSearch, window, cx| s.clear_search(window, cx)))
            // Clicks on plain divs do not move focus, which would silence the shortcuts.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|s, _, window, cx| window.focus(&s.focus, cx)),
            )
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(theme::OBSIDIAN_950))
            .text_color(rgb(theme::IVORY_100))
            .font_family(theme::FONT_UI)
            .text_size(px(13.))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .child(self.render_sidebar(cx))
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(div().size_full().flex().flex_col().child(main))
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .w_full()
                                    .child(TitleBar::new().bg(bar_bg).border_color(bar_bg)),
                            ),
                    ),
            )
            .children(self.render_engine_banner(cx))
            .child(self.render_now_playing(cx))
    }
}

impl AppState {
    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let item = |id: &'static str, label: &'static str, glyph: &'static str, view: View| {
            let selected = self.view == view;
            div()
                .id(id)
                .h(px(36.))
                .px(px(12.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.))
                .rounded(px(10.))
                .cursor_pointer()
                .text_size(px(13.))
                .text_color(rgb(if selected {
                    theme::IVORY_50
                } else {
                    theme::IVORY_300
                }))
                .hover(|s| s.bg(rgb(theme::OBSIDIAN_800)))
                .on_click(cx.listener(move |s, _, _, cx| s.set_view(view, cx)))
                .child(icon(
                    glyph,
                    16.,
                    if selected {
                        theme::IVORY_50
                    } else {
                        theme::IVORY_500
                    },
                ))
                .child(div().flex_1().child(label))
                .child(
                    div()
                        .size(px(6.))
                        .rounded_full()
                        .when(selected, |d| d.bg(rgb(theme::GOLD_500))),
                )
        };

        div()
            .relative()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(rgb(theme::OBSIDIAN_900))
            .px(px(16.))
            .pt(px(TITLE_BAR_HEIGHT + 20.))
            .pb(px(24.))
            // The traffic lights sit on the sidebar; the strip above lets it drag the window.
            .child(
                drag_strip(self.drag_armed.clone())
                    .absolute()
                    .top_0()
                    .left_0()
                    .w_full()
                    .h(px(TITLE_BAR_HEIGHT)),
            )
            .child(
                div()
                    .px(px(12.))
                    .pb(px(28.))
                    .font_family(theme::FONT_DISPLAY)
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::IVORY_50))
                    .child(spaced("AURUM", 6.)),
            )
            .child(
                div()
                    .px(px(12.))
                    .pb(px(8.))
                    .text_size(px(11.))
                    .text_color(rgb(theme::IVORY_500))
                    .child(spaced("LIBRARY", 2.)),
            )
            .child(item("nav-albums", "Albums", "disc-3", View::Albums))
            .child(item("nav-artists", "Artists", "user", View::Artists))
            .child(item("nav-tracks", "Tracks", "list-music", View::Tracks))
            .child(div().mt_auto().child(item(
                "nav-settings",
                "Settings",
                "settings",
                View::Settings,
            )))
    }

    /// Shown instead of the library views while no music folder is configured.
    fn render_empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.))
            .child(
                div()
                    .font_family(theme::FONT_DISPLAY)
                    .text_size(px(34.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::IVORY_50))
                    .child("Your library is empty"),
            )
            .child(
                div()
                    .max_w(px(420.))
                    .text_center()
                    .text_size(px(15.))
                    .text_color(rgb(theme::IVORY_300))
                    .child("Point Aurum at a folder of music and it will find your albums."),
            )
            .child(
                div()
                    .id("empty-add")
                    .mt(px(16.))
                    .h(px(44.))
                    .px(px(24.))
                    .flex()
                    .items_center()
                    .rounded(px(10.))
                    .bg(rgb(theme::GOLD_500))
                    .text_color(rgb(theme::OBSIDIAN_950))
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::GOLD_400)))
                    .active(|s| s.bg(rgb(theme::GOLD_700)))
                    .on_click(cx.listener(|s, _, _, cx| s.prompt_add_folder(cx)))
                    .child("Add a music folder"),
            )
    }
}

impl AppState {
    /// Danger strip above the bar while the audio engine reports a problem.
    fn render_engine_banner(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let message = self.engine_error.clone()?;
        Some(
            div()
                .w_full()
                .h(px(40.))
                .flex_none()
                .px(px(24.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .bg(theme::color(theme::DANGER).opacity(0.14))
                .border_t_1()
                .border_color(theme::color(theme::DANGER).opacity(0.4))
                .text_size(px(13.))
                .text_color(rgb(theme::DANGER))
                .child(icon("triangle-alert", 16., theme::DANGER))
                .child(div().flex_1().min_w_0().truncate().child(message))
                .child(
                    div()
                        .id("banner-close")
                        .size(px(28.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.))
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::color(theme::DANGER).opacity(0.2)))
                        .tooltip(tip("Dismiss"))
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.engine_error = None;
                            cx.notify();
                        }))
                        .child(icon("x", 16., theme::DANGER)),
                ),
        )
    }
}
