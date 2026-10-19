//! Lazy GPU upload of runtime-rasterized glyphs. Font discovery stays outside this adapter.
use crate::{gpu::Gpu, texture::SourceTexture};
use media_domain::{
    LayerState,
    slice_grid::{SLICE_TEXT_HEIGHT, SLICE_TEXT_WIDTH, SliceTextAtlas},
};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct Cache {
    keys: [Option<Arc<SliceTextAtlas>>; 4],
    texture: Option<SourceTexture>,
}
impl Cache {
    pub(super) fn prepare(&mut self, gpu: &Gpu, layer: Option<&LayerState>) {
        let keys = std::array::from_fn(|index| {
            layer.and_then(|layer| {
                let effect = &layer.effects[index];
                effect
                    .beat_slice_grid_parameters()
                    .and(effect.text_atlas.as_ref())
                    .cloned()
            })
        });
        if self.keys.iter().zip(&keys).all(|(a, b)| match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }) {
            return;
        }
        let count: usize = keys.iter().flatten().map(|atlas| atlas.aspects.len()).sum();
        self.texture = if count == 0 {
            None
        } else {
            // At most 32 entries: compact columns of eight keep both dimensions <=2048.
            let columns = count.div_ceil(8);
            let rows = count.min(8);
            let width = SLICE_TEXT_WIDTH as usize * columns;
            let height = SLICE_TEXT_HEIGHT as usize * rows;
            let mut pixels = vec![0; width * height * 4];
            let mut entry = 0;
            for atlas in keys.iter().flatten() {
                for index in 0..atlas.aspects.len() {
                    for row in 0..SLICE_TEXT_HEIGHT as usize {
                        let source = (index * SLICE_TEXT_HEIGHT as usize + row)
                            * SLICE_TEXT_WIDTH as usize
                            * 4;
                        let dest = (((entry % 8) * SLICE_TEXT_HEIGHT as usize + row) * width
                            + (entry / 8) * SLICE_TEXT_WIDTH as usize)
                            * 4;
                        pixels[dest..dest + SLICE_TEXT_WIDTH as usize * 4].copy_from_slice(
                            &atlas.pixels[source..source + SLICE_TEXT_WIDTH as usize * 4],
                        );
                    }
                    entry += 1;
                }
            }
            Some(
                SourceTexture::from_rgba8(
                    gpu,
                    media_domain::Size::new(width as u32, height as u32),
                    &pixels,
                )
                .expect("validated slice text atlas fits WebGPU minimum texture limits"),
            )
        };
        self.keys = keys;
    }
    pub(super) fn view<'a>(&'a self, fallback: &'a SourceTexture) -> &'a wgpu::TextureView {
        &self.texture.as_ref().unwrap_or(fallback).view
    }
}
