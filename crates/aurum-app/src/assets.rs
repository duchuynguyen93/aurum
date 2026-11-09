//! Embedded assets: the app's Lucide icons first, then gpui-kit's default bundle as fallback.

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        const ICONS: &[(&str, &[u8])] = &[
            $((concat!("icons/", $name, ".svg"),
               include_bytes!(concat!("../assets/icons/", $name, ".svg")).as_slice())),*
        ];
    };
}

icons!(
    "play",
    "pause",
    "skip-back",
    "skip-forward",
    "shuffle",
    "repeat",
    "repeat-1",
    "volume-2",
    "volume-x",
    "settings",
    "disc-3",
    "music",
    "user",
    "list-music",
    "folder-plus",
    "trash",
    "refresh-cw",
);

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, data)) = ICONS.iter().find(|(p, _)| *p == path) {
            return Ok(Some(Cow::Borrowed(data)));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut out: Vec<SharedString> = ICONS
            .iter()
            .filter(|(p, _)| p.starts_with(path))
            .map(|(p, _)| SharedString::from(*p))
            .collect();
        out.extend(gpui_kit::assets::Assets.list(path)?);
        Ok(out)
    }
}
