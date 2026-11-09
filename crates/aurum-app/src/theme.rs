//! Design tokens from docs/DESIGN.md. Hex values are 0xRRGGBB.
#![allow(dead_code)]

pub const OBSIDIAN_950: u32 = 0x0C0B0E;
pub const OBSIDIAN_900: u32 = 0x121114;
pub const OBSIDIAN_850: u32 = 0x1A181C;
pub const OBSIDIAN_800: u32 = 0x222025;
pub const OBSIDIAN_700: u32 = 0x2E2B31;
pub const OBSIDIAN_600: u32 = 0x45414A;

pub const IVORY_500: u32 = 0x8E887E;
pub const IVORY_300: u32 = 0xB9B2A6;
pub const IVORY_100: u32 = 0xEFE9DE;
pub const IVORY_50: u32 = 0xF7F3EA;

pub const GOLD_500: u32 = 0xD4AF6A;
pub const GOLD_400: u32 = 0xE6C680;
pub const GOLD_700: u32 = 0xA8853F;

pub const SUCCESS: u32 = 0x7FB58A;
pub const DANGER: u32 = 0xC96B6B;

pub const FONT_DISPLAY: &str = "Fraunces";
pub const FONT_UI: &str = "Inter";
pub const FONT_MONO: &str = "JetBrains Mono";

/// Token colour as a GPUI colour.
pub fn color(hex: u32) -> gpui_kit::Hsla {
    gpui_kit::rgb(hex).into()
}

/// Divider: hairline `Ivory50` at 6% alpha.
pub fn hairline() -> gpui_kit::Hsla {
    color(IVORY_50).opacity(0.06)
}

/// Maps the tokens onto the gpui-component theme, whose widgets (search input, tooltip,
/// skeleton) read their colours from it. Call once after `gpui_kit::init`.
pub fn apply(cx: &mut gpui_kit::App) {
    use gpui_kit::component::{Theme, ThemeMode};

    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |t| {
        t.background = color(OBSIDIAN_950);
        t.foreground = color(IVORY_100);
        t.muted_foreground = color(IVORY_500);
        t.border = color(OBSIDIAN_700);
        t.input = color(OBSIDIAN_700);
        t.ring = color(GOLD_500);
        t.caret = color(IVORY_100);
        t.selection = color(GOLD_500).opacity(0.3);
        t.popover = color(OBSIDIAN_800);
        t.popover_foreground = color(IVORY_100);
        t.skeleton = color(OBSIDIAN_700);
        t.font_family = FONT_UI.into();
        t.radius = gpui_kit::px(10.);
    });
}
