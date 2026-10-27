//! Exact projection of shader/depth dependencies. Colours of opaque surfaces and media pixels
//! do not affect depth; projected media LIGHTS remain in the complete light/beam arrays.
use crate::instances::{FrameInstances, MeshKind};
use crate::renderer::Globals;
#[derive(Default, Debug, PartialEq, Eq)]
pub(in crate::renderer) struct Key {
    bytes: Vec<u8>,
    kinds: Vec<MeshKind>,
}
impl Key {
    fn add(&mut self, bytes: &[u8]) {
        self.bytes
            .extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        self.bytes.extend_from_slice(bytes);
    }
    pub fn build(
        scene: &viz_scene::Scene,
        frame: &FrameInstances,
        globals: &Globals,
        generation: u64,
        beam_size: (u32, u32),
        lights: u32,
        beams: u32,
    ) -> Self {
        let mut key = Self::default();
        key.add(&generation.to_le_bytes());
        key.add(&scene.revision.to_le_bytes());
        key.add(&[u8::from(scene.show_id.is_some())]);
        key.add(&scene.show_id.map_or(0, |id| id.as_u128()).to_le_bytes());
        key.add(bytemuck::cast_slice(&[beam_size.0, beam_size.1]));
        let mut uniforms = *globals;
        uniforms.camera_position[3] = 0.0; // wall clock is unused by uniform Draft haze
        uniforms.params[0] = 0.0; // exposure is applied by current composition
        uniforms.params2[2] = 0.0; // opaque ambient does not affect depth
        uniforms.params3[2] = 0.0; // haze time; nonuniform fog bypasses reuse entirely
        uniforms.params3[3] = 0.0; // laser brightness is composited after this attachment
        uniforms.params5[2] = 0.0;
        uniforms.params5[3] = 0.0;
        key.add(bytemuck::bytes_of(&uniforms));
        key.add(bytemuck::cast_slice(&frame.lights[..lights as usize]));
        key.add(bytemuck::cast_slice(&frame.beams[..beams as usize]));
        key.add(bytemuck::cast_slice(&frame.curtain_blockers));
        for (kind, instances) in frame
            .meshes
            .iter()
            .filter(|(_, instances)| !instances.is_empty())
        {
            key.kinds.push(*kind);
            key.add(&(instances.len() as u64).to_le_bytes());
            for instance in instances {
                key.add(bytemuck::bytes_of(&instance.model));
                key.add(&instance.surface[1].to_bits().to_le_bytes()); // surface discard predicate
            }
        }
        // All media texels write alpha1; UV/crop/content/material affect colour, never depth.
        key.add(&(frame.media_panels.len() as u64).to_le_bytes());
        for panel in &frame.media_panels {
            key.add(bytemuck::bytes_of(&panel.model.to_cols_array_2d()));
        }
        // Opaque guide lines have depth writes disabled; laser ribbons run after the beam pass.
        key
    }
}
