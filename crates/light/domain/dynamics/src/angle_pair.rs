//! Cold normalization of typed Position Dynamics. A saved Angle Dynamic always owns both
//! axes; an untouched partner follows Current in the same pre-Dynamic source frame.
use crate::*;
use light_core::programming::ProgrammingComponent;
use uuid::Uuid;

impl DynamicLane {
    pub fn is_angle_current_passthrough(&self) -> bool {
        self.is_programming_angles()
            && matches!(&self.body,
            DynamicLaneBody::Programming(ProgrammingLaneBody {
                configuration: ProgrammingLaneConfiguration::Keyframes(config), ..
            }) if !config.points.is_empty()
                && config.points.iter().all(|point| point.source == DynamicValueSource::Current))
    }

    pub fn is_programming_angles(&self) -> bool {
        matches!(&self.body, DynamicLaneBody::Programming(body)
            if body.address.representation == DynamicFamilyRepresentation::Angles)
    }
}

impl DynamicDefinition {
    pub(crate) fn is_automatic_angle_partner_id(
        &self,
        lane: Uuid,
        component: ProgrammingComponent,
    ) -> bool {
        matches!(
            component,
            ProgrammingComponent::Pan | ProgrammingComponent::Tilt
        ) && current_partner(self.id, component).id == lane
    }

    pub fn is_automatic_angle_partner(&self, lane: &DynamicLane) -> bool {
        [ProgrammingComponent::Pan, ProgrammingComponent::Tilt]
            .into_iter()
            .any(|axis| same_automatic_partner(lane, &current_partner(self.id, axis)))
    }

    /// Copy/create changes the definition namespace. Only untouched generated partners get a
    /// new derived identity; authored lanes keep their existing identity and content.
    pub fn reidentify(&mut self, id: Uuid) -> Vec<(Uuid, Uuid)> {
        let mut changed = Vec::new();
        for component in [ProgrammingComponent::Pan, ProgrammingComponent::Tilt] {
            let previous = current_partner(self.id, component);
            if let Some(lane) = self
                .lanes
                .iter_mut()
                .find(|lane| same_automatic_partner(lane, &previous))
            {
                let new_id = current_partner(id, component).id;
                changed.push((lane.id, new_id));
                lane.id = new_id;
            }
        }
        self.id = id;
        self.normalize_angle_pair();
        changed
    }

    /// Persist real partner lanes, rather than synthesizing output-only scalar shadows.
    /// Idempotent and revision-independent, including for embedded deletion fallbacks.
    pub fn normalize_angle_pair(&mut self) {
        if !self.lanes.iter().any(DynamicLane::is_programming_angles) {
            return;
        }
        let pan = current_partner(self.id, ProgrammingComponent::Pan);
        let tilt = current_partner(self.id, ProgrammingComponent::Tilt);
        let automatic = |lane: &DynamicLane| {
            same_automatic_partner(lane, &pan) || same_automatic_partner(lane, &tilt)
        };
        let mut authored_pan = false;
        let mut authored_tilt = false;
        let mut whole = false;
        for lane in self.lanes.iter().filter(|lane| !automatic(lane)) {
            if let DynamicLaneBody::Programming(body) = &lane.body
                && lane.is_programming_angles()
            {
                match body.address.component {
                    Some(ProgrammingComponent::Pan) => authored_pan = true,
                    Some(ProgrammingComponent::Tilt) => authored_tilt = true,
                    None => whole = true,
                    _ => {}
                }
            }
        }
        // Removing the authored axis removes its untouched automatic partner. Replacing a
        // partner's content makes it authored, so it is never silently discarded afterward.
        self.lanes.retain(|lane| {
            !(same_automatic_partner(lane, &pan) && (whole || authored_pan || !authored_tilt)
                || same_automatic_partner(lane, &tilt) && (whole || authored_tilt || !authored_pan))
        });
        if !whole {
            if authored_pan
                && !authored_tilt
                && !self
                    .lanes
                    .iter()
                    .any(|lane| same_automatic_partner(lane, &tilt))
            {
                self.lanes.push(tilt);
            }
            if authored_tilt
                && !authored_pan
                && !self
                    .lanes
                    .iter()
                    .any(|lane| same_automatic_partner(lane, &pan))
            {
                self.lanes.push(pan);
            }
        }
    }

    pub(crate) fn validate_angle_pair(&self) -> Result<(), DynamicValidationError> {
        let mut axes = [0; 3];
        for lane in self
            .lanes
            .iter()
            .filter(|lane| lane.is_programming_angles())
        {
            let DynamicLaneBody::Programming(body) = &lane.body else {
                unreachable!()
            };
            axes[match body.address.component {
                Some(ProgrammingComponent::Pan) => 0,
                Some(ProgrammingComponent::Tilt) => 1,
                None => 2,
                _ => continue,
            }] += 1;
        }
        if !matches!(axes, [0, 0, 0] | [1, 1, 0] | [0, 0, 1]) {
            return Err(DynamicValidationError::Programming(
                "an Angle Dynamic needs one complete Pan/Tilt pair or one whole Angles lane".into(),
            ));
        }
        Ok(())
    }
}

fn same_automatic_partner(lane: &DynamicLane, expected: &DynamicLane) -> bool {
    // Per-lane phase seeding, speed and width do not animate a Current/Current source.
    // Treat only changes to the actual source configuration as authoring this partner.
    lane.id == expected.id && lane.body == expected.body && lane.random_group_id.is_none()
}

fn current_partner(definition: Uuid, component: ProgrammingComponent) -> DynamicLane {
    let key: &[u8] = match component {
        ProgrammingComponent::Pan => b"tosklight:position-current:pan:v1",
        ProgrammingComponent::Tilt => b"tosklight:position-current:tilt:v1",
        _ => unreachable!(),
    };
    DynamicLane {
        id: Uuid::new_v5(&definition, key),
        body: DynamicLaneBody::Programming(ProgrammingLaneBody {
            address: DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Angles,
                component: Some(component),
            },
            configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
                points: [0.0, 0.5]
                    .map(|position| DynamicKeyframe {
                        position,
                        source: DynamicValueSource::Current,
                        interpolation: ScalarInterpolation::Linear,
                    })
                    .to_vec(),
                size: 1.0,
            }),
        }),
        speed_multiplier: Rational::default(),
        width: 1.0,
        phase: None,
        random_group_id: None,
    }
}
