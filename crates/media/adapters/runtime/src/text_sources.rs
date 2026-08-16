//! Text sources, per layer.
//!
//! Each layer keeps its own countdown, because the lifecycle is the *layer's* transport: the same
//! entry on a stopped layer and a running one behaves differently, and two layers showing the same
//! countdown are two independent counts.
//!
//! A rasterized line is uploaded only when the text it draws has changed. A clock changes once a
//! second; re-uploading the same pixels sixty times a second would cost a show real frames.

use std::collections::HashMap;

use media_domain::geometry::Size;
use media_domain::text::{Countdown, Visibility};
use media_domain::text_catalog::TextStyle;
use media_domain::{LayerState, SourceFailure};
use media_render::{Gpu, SourceTexture};
use media_text::{Fonts, render_line};

use crate::layer_pipeline::FrameContext;

/// One layer's text state.
struct Drawn {
    /// What was last rasterized, so unchanged text is not uploaded again.
    text: String,
    /// Style is part of the raster cache key: the same words in a new font are new pixels.
    style: Option<TextStyle>,
    countdown: Countdown,
    texture: Option<SourceTexture>,
}

impl Drawn {
    fn new() -> Self {
        Self {
            // No text has been drawn yet, and an empty entry produces an empty string — so the
            // sentinel has to be something a text entry can never produce.
            text: "\u{0}".to_owned(),
            style: None,
            countdown: Countdown::new(),
            texture: None,
        }
    }
}

/// Text sources for one output.
pub struct TextSources {
    gpu: Gpu,
    size: Size,
    fonts: Option<Fonts>,
    layers: HashMap<usize, Drawn>,
}

impl TextSources {
    pub fn new(gpu: Gpu, size: Size) -> Self {
        // Scanning a machine's fonts is slow, so it happens once. A machine with none says so
        // once here rather than on every frame that selects text.
        let fonts = match Fonts::load() {
            Ok(fonts) => Some(fonts),
            Err(error) => {
                tracing::error!(%error, "text sources cannot be drawn on this machine");
                None
            }
        };
        Self {
            gpu,
            size,
            fonts,
            layers: HashMap::new(),
        }
    }

    pub fn resize(&mut self, size: Size) {
        if size == self.size || size.is_empty() {
            return;
        }
        self.size = size;
        // Every raster is output-sized, so they are all stale at once. Clearing the drawn text
        // with them is what makes the next frame redraw rather than keep a wrong-sized texture.
        for drawn in self.layers.values_mut() {
            drawn.texture = None;
            drawn.text = "\u{0}".to_owned();
            drawn.style = None;
        }
    }

    /// Brings one layer's text up to date. Returns whether it has something to draw.
    pub fn prepare(
        &mut self,
        layer_index: usize,
        layer: &LayerState,
        context: FrameContext<'_>,
    ) -> Result<bool, SourceFailure> {
        let Some(slot) = context.configuration.text.resolve(layer.address) else {
            // An address in the text range with nothing assigned is a missing source, exactly as
            // a missing file is.
            self.layers.remove(&layer_index);
            return Err(SourceFailure::MissingFile);
        };

        let drawn = self.layers.entry(layer_index).or_insert_with(Drawn::new);
        // The countdown keys on the layer, not on the text: this is what starts it when the layer
        // becomes visible and freezes it when the operator pauses.
        drawn.countdown.observe(
            Visibility {
                visible: layer.draws(),
                play_mode: layer.play_mode,
            },
            context.now.as_millis(),
        );

        let Some(text) = media_domain::text::render(
            &slot.entry,
            &drawn.countdown,
            context.now_unix_millis,
            context.now.as_millis(),
        ) else {
            // A disabled entry produces nothing, which is how an operator parks one.
            drawn.texture = None;
            drawn.text.clear();
            drawn.style = None;
            return Ok(false);
        };

        if raster_matches(drawn, &text, &slot.style) {
            return Ok(drawn.texture.is_some());
        }

        let Some(fonts) = self.fonts.as_mut() else {
            return Err(SourceFailure::UnsupportedCodec);
        };
        let rendered = render_line(fonts, &text, &slot.style, self.size.width, self.size.height)
            .map_err(|error| {
                tracing::warn!(address = %layer.address, %error, "a text source could not be drawn");
                SourceFailure::UnsupportedCodec
            })?;
        if !rendered.exact_family {
            tracing::debug!(
                family = %slot.style.family,
                address = %layer.address,
                "this machine does not have that font; another was used"
            );
        }

        let texture = SourceTexture::from_rgba8(
            &self.gpu,
            Size::new(rendered.width, rendered.height),
            &rendered.pixels,
        )
        .map_err(|error| {
            tracing::warn!(%error, "a text source could not be uploaded");
            SourceFailure::GpuUploadFailed
        })?;

        drawn.text = text;
        drawn.style = Some(slot.style.clone());
        drawn.texture = Some(texture);
        Ok(true)
    }

    pub fn texture(&self, layer_index: usize) -> Option<&SourceTexture> {
        self.layers
            .get(&layer_index)
            .and_then(|drawn| drawn.texture.as_ref())
    }
}

fn raster_matches(drawn: &Drawn, text: &str, style: &TextStyle) -> bool {
    text == drawn.text && drawn.style.as_ref() == Some(style)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changing_style_invalidates_the_raster_even_when_the_words_are_identical() {
        let mut drawn = Drawn::new();
        let original = TextStyle::default();
        drawn.text = "Stand by".to_owned();
        drawn.style = Some(original.clone());
        assert!(raster_matches(&drawn, "Stand by", &original));

        let changed = TextStyle {
            size: 0.4,
            ..original
        };
        assert!(!raster_matches(&drawn, "Stand by", &changed));
    }
}
