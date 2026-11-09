//! The 88px Now Playing bar.

use std::rc::Rc;

use aurum_core::Repeat;
use gpui_kit::prelude::*;
use gpui_kit::*;

use super::format::clock;
use super::slider::Slider;
use super::widgets::icon;
use crate::state::app_state::AppState;
use crate::state::cover_cache;
use crate::theme;

const BAR_HEIGHT: f32 = 88.;
const PLAY_SIZE: f32 = 44.;

/// Drag payload types: one per slider so their drag-move events never cross.
#[derive(Clone, Copy)]
struct ProgressDrag;
#[derive(Clone, Copy)]
struct VolumeDrag;

impl AppState {
    pub(super) fn render_now_playing(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let left = self.now_playing_info();
        let center = self.now_playing_center(cx);
        let right = self.now_playing_volume(cx);

        div()
            .h(px(BAR_HEIGHT))
            .w_full()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(24.))
            .px(px(24.))
            .bg(rgb(theme::OBSIDIAN_900))
            .border_t_1()
            .border_color(theme::hairline())
            .child(left)
            .child(center)
            .child(right)
    }

    fn now_playing_info(&mut self) -> Div {
        let current = self.playback.current.clone();
        let cover = current
            .as_ref()
            .and_then(|t| self.covers.get(&cover_cache::key_of(t)));

        let art = div()
            .size(px(56.))
            .flex_none()
            .rounded(px(10.))
            .overflow_hidden()
            .bg(rgb(theme::OBSIDIAN_800))
            .flex()
            .items_center()
            .justify_center()
            .child(match cover {
                Some(image) => img(image)
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .into_any_element(),
                None => icon("music", 20., theme::IVORY_500).into_any_element(),
            });

        let (title, subtitle) = match &current {
            Some(t) => {
                let who = [t.artist.as_deref(), t.album.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" \u{b7} ");
                (t.title.clone(), who)
            }
            None => ("Nothing playing".to_string(), String::new()),
        };

        let sub = match &self.engine_error {
            Some(e) => div()
                .text_size(px(11.))
                .text_color(rgb(theme::DANGER))
                .child(e.clone()),
            None => div()
                .text_size(px(13.))
                .text_color(rgb(theme::IVORY_300))
                .child(subtitle),
        };

        div()
            .w(px(300.))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(14.))
            .child(art)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .truncate()
                            .font_family(theme::FONT_DISPLAY)
                            .text_size(px(17.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(if current.is_some() {
                                theme::IVORY_50
                            } else {
                                theme::IVORY_500
                            }))
                            .child(title),
                    )
                    .child(sub.truncate()),
            )
    }

    fn now_playing_center(&self, cx: &mut Context<Self>) -> Div {
        let button = |id: &'static str, glyph: &'static str, color: u32| {
            div()
                .id(id)
                .size(px(36.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .cursor_pointer()
                .hover(|s| s.bg(rgb(theme::OBSIDIAN_800)))
                .child(icon(glyph, 18., color))
        };

        let shuffle_on = self.queue.is_shuffled();
        let repeat = self.queue.repeat();
        let playing = self.playback.playing;

        let controls = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(20.))
            .child(
                button(
                    "ctl-shuffle",
                    "shuffle",
                    if shuffle_on {
                        theme::IVORY_50
                    } else {
                        theme::IVORY_500
                    },
                )
                .on_click(cx.listener(|s, _, _, cx| s.toggle_shuffle(cx))),
            )
            .child(
                button("ctl-prev", "skip-back", theme::IVORY_100)
                    .on_click(cx.listener(|s, _, _, cx| s.prev(cx))),
            )
            .child(
                div()
                    .id("ctl-play")
                    .size(px(PLAY_SIZE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(rgb(theme::GOLD_500))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::GOLD_400)))
                    .active(|s| s.bg(rgb(theme::GOLD_700)))
                    .on_click(cx.listener(|s, _, _, cx| s.toggle_play(cx)))
                    .child(icon(
                        if playing { "pause" } else { "play" },
                        20.,
                        theme::OBSIDIAN_950,
                    )),
            )
            .child(
                button("ctl-next", "skip-forward", theme::IVORY_100)
                    .on_click(cx.listener(|s, _, _, cx| s.next(cx))),
            )
            .child(
                button(
                    "ctl-repeat",
                    if repeat == Repeat::One {
                        "repeat-1"
                    } else {
                        "repeat"
                    },
                    if repeat == Repeat::Off {
                        theme::IVORY_500
                    } else {
                        theme::IVORY_50
                    },
                )
                .on_click(cx.listener(|s, _, _, cx| s.cycle_repeat(cx))),
            );

        let duration = self.playback.duration;
        let ratio = match self.scrub {
            Some(r) => r,
            None if duration.is_zero() => 0.,
            None => (self.playback.position.as_secs_f32() / duration.as_secs_f32()).clamp(0., 1.),
        };
        let this = cx.entity();
        let (change_this, release_this) = (this.clone(), this);
        let progress = Slider {
            id: "progress",
            drag: ProgressDrag,
            ratio,
            active: self.scrub.is_some(),
            bounds: self.progress_bounds.clone(),
            on_change: Rc::new(move |r, _, app| {
                change_this.update(app, |s, cx| s.scrub_to(r, cx));
            }),
            on_release: Rc::new(move |_, app| {
                release_this.update(app, |s, cx| s.scrub_end(cx));
            }),
        }
        .render();

        let time = |text: String, right: bool| {
            let d = div()
                .w(px(44.))
                .flex_none()
                .flex()
                .font_family(theme::FONT_MONO)
                .text_size(px(11.))
                .text_color(rgb(theme::IVORY_500))
                .child(text);
            if right {
                d.justify_start()
            } else {
                d.justify_end()
            }
        };

        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .items_center()
            .child(controls)
            .child(
                div()
                    .w_full()
                    .max_w(px(620.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(time(clock(self.playback.position), false))
                    .child(div().flex_1().child(progress))
                    .child(time(clock(duration), true)),
            )
    }

    fn now_playing_volume(&self, cx: &mut Context<Self>) -> Div {
        let volume = self.settings.volume;
        let this = cx.entity();
        let slider = Slider {
            id: "volume",
            drag: VolumeDrag,
            ratio: volume,
            active: false,
            bounds: self.volume_bounds.clone(),
            on_change: Rc::new(move |r, _, app| {
                this.update(app, |s, cx| s.set_volume(r, cx));
            }),
            on_release: Rc::new(|_, _| {}),
        }
        .render();

        div()
            .w(px(300.))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .justify_end()
            .gap(px(10.))
            .child(
                div()
                    .id("mute")
                    .size(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::OBSIDIAN_800)))
                    .on_click(cx.listener(|s, _, _, cx| s.toggle_mute(cx)))
                    .child(icon(
                        if volume > 0. { "volume-2" } else { "volume-x" },
                        18.,
                        theme::IVORY_300,
                    )),
            )
            .child(div().w(px(96.)).child(slider))
    }
}
