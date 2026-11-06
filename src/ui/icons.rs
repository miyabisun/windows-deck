//! The 24×24 line icons (drawn in the text colour) and the spinner, served to GPUI as SVG
//! assets.

use std::{borrow::Cow, time::Duration};

use gpui::{
    Animation, AnimationExt, AssetSource, Hsla, IntoElement, Pixels, SharedString, Styled, Svg,
    Transformation, percentage, svg,
};

use crate::model::display::Glyph;

impl Glyph {
    fn name(self) -> &'static str {
        match self {
            Self::Plus => "plus",
            Self::Moon => "moon",
            Self::Sun => "sun",
            Self::Close => "close",
            Self::Check => "check",
            Self::Volume => "volume",
            Self::Sound => "sound",
            Self::Muted => "muted",
            Self::Speaker => "speaker",
            Self::Headphones => "headphones",
        }
    }

    fn path(self) -> &'static str {
        match self {
            Self::Plus => "M12 5v14M5 12h14",
            Self::Moon => "M20 14.5A8.5 8.5 0 1 1 9.5 4a7 7 0 0 0 10.5 10.5z",
            Self::Sun => {
                "M12 8a4 4 0 1 0 0 8a4 4 0 1 0 0-8zM12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"
            }
            Self::Close => "M6 6l12 12M18 6 6 18",
            Self::Check => "M5 12.5l4.5 4.5L19 7.5",
            // A speaker cone: alone for the mixer, with sound or crossed out for mute.
            Self::Volume => "M11 5 6 9H2v6h4l5 4z",
            Self::Sound => "M11 5 6 9H2v6h4l5 4zM15.5 8.5a5 5 0 0 1 0 7M19 5a10 10 0 0 1 0 14",
            Self::Muted => "M11 5 6 9H2v6h4l5 4zM22 9l-6 6M16 9l6 6",
            Self::Speaker => {
                "M6 2h12a2 2 0 0 1 2 2v16a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2zM12 10a4 4 0 1 0 0 8a4 4 0 1 0 0-8zM12 6h.01"
            }
            Self::Headphones => {
                "M3 18v-6a9 9 0 0 1 18 0v6M21 19a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3zM3 19a2 2 0 0 0 2 2h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H3z"
            }
        }
    }

    const ALL: [Self; 10] = [
        Self::Plus,
        Self::Moon,
        Self::Sun,
        Self::Close,
        Self::Check,
        Self::Volume,
        Self::Sound,
        Self::Muted,
        Self::Speaker,
        Self::Headphones,
    ];
}

const SPINNER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="12" r="9" fill="none" stroke="black" stroke-width="2" opacity="0.25"/><path d="M21 12a9 9 0 0 0-9-9" fill="none" stroke="black" stroke-width="2" stroke-linecap="round"/></svg>"#;

/// The icons as `glyph/<name>.svg` and the spinner as `spinner.svg`.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if path == "spinner.svg" {
            return Ok(Some(Cow::Borrowed(SPINNER.as_bytes())));
        }
        let Some(name) = path
            .strip_prefix("glyph/")
            .and_then(|p| p.strip_suffix(".svg"))
        else {
            return Ok(None);
        };
        Ok(Glyph::ALL.iter().find(|g| g.name() == name).map(|g| {
            Cow::Owned(
                format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="{}"/></svg>"#,
                    g.path()
                )
                .into_bytes(),
            )
        }))
    }

    fn list(&self, _path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

/// A line icon of `size` in `color`.
pub fn glyph(glyph: Glyph, size: Pixels, color: Hsla) -> Svg {
    svg()
        .path(SharedString::from(format!("glyph/{}.svg", glyph.name())))
        .size(size)
        .flex_none()
        .text_color(color)
}

/// The 18px spinner in the accent colour; it stops turning when Windows asks for less motion.
pub fn spinner(color: Hsla) -> impl IntoElement {
    let icon = svg()
        .path("spinner.svg")
        .size(gpui::px(18.))
        .flex_none()
        .text_color(color);
    if crate::platform::reduced_motion() {
        return icon.into_any_element();
    }
    icon.with_animation(
        "spinner",
        Animation::new(Duration::from_millis(800)).repeat(),
        |icon, delta| icon.with_transformation(Transformation::rotate(percentage(delta))),
    )
    .into_any_element()
}
