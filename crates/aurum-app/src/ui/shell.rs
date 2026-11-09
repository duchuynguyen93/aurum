//! Window shell: sidebar, main area and the Now Playing bar. `AppState` is the root view.

use gpui_kit::prelude::*;
use gpui_kit::*;

use super::widgets::{icon, spaced, view_title};
use crate::state::app_state::{AppState, View};
use crate::theme;

actions!(aurum, [TogglePlay, NextTrack, PrevTrack, OpenSettings]);

const SIDEBAR_WIDTH: f32 = 220.;

impl Render for AppState {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_roots = !self.settings.roots.is_empty();
        let main = match self.view {
            View::Settings => self.render_settings(cx).into_any_element(),
            _ if !has_roots => self.render_empty(cx).into_any_element(),
            View::Tracks => self.render_tracks(cx).into_any_element(),
            View::Albums => self.render_placeholder("Albums").into_any_element(),
            View::Artists => self.render_placeholder("Artists").into_any_element(),
        };

        div()
            .key_context("Aurum")
            .track_focus(&self.focus)
            .on_action(cx.listener(|s, _: &TogglePlay, _, cx| s.toggle_play(cx)))
            .on_action(cx.listener(|s, _: &NextTrack, _, cx| s.next(cx)))
            .on_action(cx.listener(|s, _: &PrevTrack, _, cx| s.prev(cx)))
            .on_action(cx.listener(|s, _: &OpenSettings, _, cx| s.set_view(View::Settings, cx)))
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
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            .child(main),
                    ),
            )
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
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(rgb(theme::OBSIDIAN_900))
            .px(px(16.))
            .py(px(24.))
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

    fn render_placeholder(&self, title: &str) -> impl IntoElement {
        div()
            .size_full()
            .p(px(32.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(view_title(title))
            .child(
                div()
                    .text_color(rgb(theme::IVORY_500))
                    .child("This view arrives in the next step of milestone 2."),
            )
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
