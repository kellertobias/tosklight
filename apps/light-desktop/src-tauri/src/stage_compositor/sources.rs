use super::{Source, StageCompositor, ownership::Picture};
use std::sync::Arc;
#[cfg(not(target_os = "macos"))]
use viz_helper::protocol::SharedSurfaceHandle;
use viz_helper::protocol::{FrameId, SHARED_FRAME_SLOTS};

#[derive(Clone)]
pub(super) enum Payload {
    Texture(Arc<Source>),
    Pixels {
        width: u32,
        height: u32,
        rgba: Arc<[u8]>,
    },
}

pub(super) struct Slot {
    pub generation: u64,
    pub slot: u8,
    pub source: Arc<Source>,
}

impl StageCompositor {
    fn validate_slot(
        &mut self,
        generation: u64,
        slot: u8,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        if generation == 0
            || usize::from(slot) >= SHARED_FRAME_SLOTS
            || width == 0
            || height == 0
            || width > self.device.limits().max_texture_dimension_2d
            || height > self.device.limits().max_texture_dimension_2d
        {
            return Err("invalid Stage source slot or dimensions".to_owned());
        }
        if generation < self.source_generation {
            return Err("stale Stage surface generation".to_owned());
        }
        if generation > self.source_generation {
            self.slots.clear();
            self.source_generation = generation;
            self.ready_sequence.reset(generation);
        }
        if self.slots.iter().any(|source| source.slot == slot) {
            return Err("Stage source slot already imported".to_owned());
        }
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    pub fn import_shared_slot(
        &mut self,
        generation: u64,
        slot: u8,
        handle: SharedSurfaceHandle,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        self.validate_slot(generation, slot, width, height)?;
        let shared = viz_surface::import(&self.device, handle, width, height)
            .map_err(|error| error.to_string())?;
        self.install_slot(generation, slot, shared, width, height);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    pub fn import_shared_slot_port(
        &mut self,
        generation: u64,
        slot: u8,
        port: viz_surface::OwnedSurfacePort,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        self.validate_slot(generation, slot, width, height)?;
        let shared = viz_surface::import_from_port(&self.device, port, width, height)
            .map_err(|error| error.to_string())?;
        self.install_slot(generation, slot, shared, width, height);
        Ok(())
    }

    fn install_slot(
        &mut self,
        generation: u64,
        slot: u8,
        shared: viz_surface::SharedSurface,
        width: u32,
        height: u32,
    ) {
        let texture = shared.texture().clone();
        let bind_group = self.bind(&texture);
        self.slots.push(Slot {
            generation,
            slot,
            source: Arc::new(Source {
                shared: Some(shared),
                _texture: texture,
                bind_group,
                size: (width, height),
            }),
        });
    }

    /// Pixels remain CPU-owned until FrameReady and a visible draw admission; hidden frames
    /// return their exact credit without uploading or allocating a GPU texture.
    pub fn accept_copy(
        &mut self,
        frame: FrameId,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> Result<(), String> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|size| size.checked_mul(4));
        if !super::sequence::copy_identity_valid(frame)
            || width == 0
            || height == 0
            || expected != Some(rgba.len())
            || width > self.device.limits().max_texture_dimension_2d
            || height > self.device.limits().max_texture_dimension_2d
        {
            return Err("the renderer sent invalid Stage copied pixels".to_owned());
        }
        if !self.waiting_copies.is_empty()
            || self
                .waiting_copies
                .iter()
                .any(|picture| picture.frame == frame)
            || self.pictures.owns_slot(frame)
        {
            return Err("Stage copied-frame capacity or ownership violation".to_owned());
        }
        if frame.generation > self.source_generation {
            self.source_generation = frame.generation;
            self.ready_sequence.reset(frame.generation);
            self.slots.clear();
        }
        self.waiting_copies.push(Picture {
            frame,
            shared: false,
            credit_released: false,
            data: Payload::Pixels {
                width,
                height,
                rgba: rgba.into(),
            },
        });
        Ok(())
    }

    pub fn mark_ready(&mut self, frame: FrameId) -> Result<(), String> {
        if !frame.is_valid() {
            return Err("invalid Stage ready-frame identity".to_owned());
        }
        let copied = self
            .waiting_copies
            .iter()
            .position(|picture| picture.frame == frame)
            .map(|index| self.waiting_copies.remove(index));
        if frame.generation < self.source_generation {
            if self.pictures.owns_slot(frame) {
                return Err("duplicate Stage ready frame".to_owned());
            }
            return self.pictures.release(frame);
        }
        if frame.generation != self.source_generation {
            return Err("stale or duplicate Stage ready-frame sequence".to_owned());
        }
        if self.pictures.owns_slot(frame) {
            return Err("producer reused a Stage slot before release".to_owned());
        }
        let picture = if let Some(copied) = copied {
            copied
        } else {
            let source = self
                .slots
                .iter()
                .find(|slot| slot.generation == frame.generation && slot.slot == frame.slot)
                .ok_or("Stage ready frame has no imported source")?
                .source
                .clone();
            Picture {
                frame,
                shared: true,
                credit_released: false,
                data: Payload::Texture(source),
            }
        };
        // Different shared slots may complete out of order. Return an older picture's exact
        // credit without replacing a newer pending/front image, even if its own slot is new.
        if !self.ready_sequence.accept(frame)? || !self.producer_visible() {
            self.pictures.release(frame)?;
            return Ok(());
        }
        self.pictures.replace_ready(picture)?;
        self.schedule.dirty();
        Ok(())
    }

    pub(super) fn materialize(&self, picture: &Picture<Payload>) -> Arc<Source> {
        match &picture.data {
            Payload::Texture(source) => source.clone(),
            Payload::Pixels {
                width,
                height,
                rgba,
            } => {
                let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("stage pane copy"),
                    size: wgpu::Extent3d {
                        width: *width,
                        height: *height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: viz_surface::FORMAT,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                self.queue.write_texture(
                    texture.as_image_copy(),
                    rgba,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(*width * 4),
                        rows_per_image: Some(*height),
                    },
                    wgpu::Extent3d {
                        width: *width,
                        height: *height,
                        depth_or_array_layers: 1,
                    },
                );
                let bind_group = self.bind(&texture);
                Arc::new(Source {
                    shared: None,
                    _texture: texture,
                    bind_group,
                    size: (*width, *height),
                })
            }
        }
    }
}
