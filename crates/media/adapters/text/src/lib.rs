#![forbid(unsafe_code)]

//! Rasterizing text sources.
//!
//! The domain decides *what* a text entry says — including the whole countdown lifecycle — and
//! knows nothing about fonts. This owns the part that needs a machine: finding a family, falling
//! back when it is absent, and turning a line into pixels.
//!
//! Text is drawn at a higher internal scale than the glyphs need and handed on as an ordinary
//! source texture, so it is transformed, tinted, dimmed, and masked exactly like a video.

mod fonts;
mod raster;

pub use fonts::{FontError, Fonts};
pub use raster::{Rendered, render_line};

/// How much larger than the requested height the raster is made.
///
/// A layer is very often scaled up on the way to a wall, and glyphs rasterized at the final size
/// would show it. Twice is enough to keep an edge clean without doubling the upload cost twice
/// over.
pub const SUPERSAMPLE: u32 = 2;
