use super::*;
use std::{
    hash::{DefaultHasher, Hasher},
    io::Write,
};

/// Stream small authored metadata into a key without allocating a Scene serialization.
#[derive(Default)]
pub(super) struct Key(DefaultHasher);
impl Write for Key {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.write(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Key {
    pub fn add(&mut self, value: &impl serde::Serialize) {
        serde_json::to_writer(&mut *self, value).expect("instance metadata is serializable");
        self.0.write_u8(0xff);
    }
    pub fn finish(&self) -> u64 {
        self.0.finish()
    }
}

pub(super) fn model_key(model: &viz_scene::FixtureModel) -> u64 {
    let mut key = Key::default();
    key.add(&(
        model.extent,
        model.head_pivot,
        model.emitter_anchor,
        model.emitter_size,
        model.emitter_axis,
        model.has_head,
        model.bracket_hinge,
    ));
    for part in &model.parts {
        key.add(&(
            &part.name,
            part.kind,
            part.geometry_node_id,
            &part.node_ancestry,
        ));
        // Scanner pivot is derived from its actual mirror bounds. Ordinary triangle buffers
        // belong to the renderer's content-keyed GPU mesh cache, not instance reconstruction.
        if part.name == "head-mirror" {
            key.add(&part.positions);
        }
    }
    key.finish()
}

pub(super) fn style_key(style: &FrameStyle) -> u64 {
    let mut key = Key::default();
    key.add(&(
        style.quality,
        style.draw_aim_lines,
        style.plot_right,
        style.plot_up,
        style.projection_view,
        style.symbol_metres,
    ));
    key.add(&(
        style.ink,
        style.faint_ink,
        style.beam_ink,
        style.symbol_ink,
        style.selected_ink,
        style.group_selected_ink,
    ));
    key.add(&(
        style.fixture_models,
        style.scenery_surfaces,
        style.aim_guides,
        style.floor_grid,
    ));
    key.add(&((style.scenery as usize), style.crowd_amount));
    key.finish()
}
