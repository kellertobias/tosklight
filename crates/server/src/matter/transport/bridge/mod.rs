mod bridged_info;
mod level_control;
mod on_off;

use super::super::{MAX_MATTER_LEVEL, MatterPlaybackWrite};
use super::model::{MatterRemoteWrite, TransportLight};
use parking_lot::RwLock;
use rs_matter::dm::Dataver;
use rs_matter::dm::clusters::decl::level_control as level_cluster;
use rs_matter::error::{Error, ErrorCode};
use std::collections::BTreeMap;
use std::sync::mpsc::Sender;

#[derive(Clone, Debug)]
pub(super) struct EndpointState {
    pub(super) name: String,
    pub(super) on: bool,
    pub(super) level: u8,
    options: u8,
    on_level: Option<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AttributeChanges {
    pub(super) endpoint_id: u16,
    pub(super) on: bool,
    pub(super) level: bool,
}

pub(super) struct BridgeLights {
    endpoints: RwLock<BTreeMap<u16, EndpointState>>,
    remote_writes: Sender<MatterRemoteWrite>,
    on_off_dataver: Dataver,
    level_dataver: Dataver,
    bridged_info_dataver: Dataver,
}

impl std::fmt::Debug for BridgeLights {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BridgeLights")
            .field("endpoint_count", &self.endpoints.read().len())
            .finish_non_exhaustive()
    }
}

impl BridgeLights {
    pub(super) fn new(
        lights: Vec<TransportLight>,
        remote_writes: Sender<MatterRemoteWrite>,
        on_off_dataver: Dataver,
        level_dataver: Dataver,
        bridged_info_dataver: Dataver,
    ) -> Self {
        let endpoints = lights
            .into_iter()
            .map(|light| {
                (
                    light.endpoint_id,
                    EndpointState {
                        name: light.name,
                        on: light.on,
                        level: if light.level == 0 { 1 } else { light.level },
                        options: level_cluster::OptionsBitmap::EXECUTE_IF_OFF.bits(),
                        on_level: None,
                    },
                )
            })
            .collect();
        Self {
            endpoints: RwLock::new(endpoints),
            remote_writes,
            on_off_dataver,
            level_dataver,
            bridged_info_dataver,
        }
    }

    pub(super) fn reconcile(&self, lights: Vec<TransportLight>) -> Vec<AttributeChanges> {
        let mut endpoints = self.endpoints.write();
        let mut changes = Vec::new();
        for light in lights {
            if let Some(endpoint) = endpoints.get_mut(&light.endpoint_id) {
                let old_on = endpoint.on;
                let old_level = endpoint.level;
                endpoint.on = light.on;
                if light.level > 0 {
                    endpoint.level = light.level;
                }
                if old_on != endpoint.on || old_level != endpoint.level {
                    changes.push(AttributeChanges {
                        endpoint_id: light.endpoint_id,
                        on: old_on != endpoint.on,
                        level: old_level != endpoint.level,
                    });
                }
            }
        }
        changes
    }

    pub(super) fn endpoint(&self, endpoint_id: u16) -> Result<EndpointState, Error> {
        self.endpoints
            .read()
            .get(&endpoint_id)
            .cloned()
            .ok_or_else(|| ErrorCode::EndpointNotFound.into())
    }

    fn send_write(&self, endpoint_id: u16, write: MatterPlaybackWrite) -> Result<(), Error> {
        self.remote_writes
            .send(MatterRemoteWrite { endpoint_id, write })
            .map_err(|_| ErrorCode::Failure.into())
    }

    pub(super) fn set_on(&self, endpoint_id: u16, on: bool) -> Result<u8, Error> {
        let level = {
            let mut endpoints = self.endpoints.write();
            let endpoint = endpoints
                .get_mut(&endpoint_id)
                .ok_or(ErrorCode::EndpointNotFound)?;
            endpoint.on = on;
            endpoint.level
        };
        self.send_write(
            endpoint_id,
            MatterPlaybackWrite {
                on: Some(on),
                level: on.then_some(level),
            },
        )?;
        Ok(level)
    }

    pub(super) fn set_level(&self, endpoint_id: u16, level: u8) -> Result<bool, Error> {
        if level == u8::MAX {
            return Err(ErrorCode::ConstraintError.into());
        }
        let on = level > 0;
        {
            let mut endpoints = self.endpoints.write();
            let endpoint = endpoints
                .get_mut(&endpoint_id)
                .ok_or(ErrorCode::EndpointNotFound)?;
            endpoint.on = on;
            if level > 0 {
                endpoint.level = level.min(MAX_MATTER_LEVEL);
            }
        }
        self.send_write(
            endpoint_id,
            MatterPlaybackWrite {
                on: Some(on),
                level: on.then_some(level.min(MAX_MATTER_LEVEL)),
            },
        )?;
        Ok(on)
    }

    fn step_level(&self, endpoint_id: u16, up: bool, step: u8) -> Result<u8, Error> {
        let current = self.endpoint(endpoint_id)?.level;
        let target = if up {
            current.saturating_add(step).min(MAX_MATTER_LEVEL)
        } else {
            current.saturating_sub(step)
        };
        self.set_level(endpoint_id, target)?;
        Ok(target)
    }
}
