use light_fixture as fixture;
use light_wire::v2::patch as wire;

pub(super) fn application_architecture(
    value: wire::VenueArchitecture,
) -> fixture::VenueArchitecture {
    match value {
        wire::VenueArchitecture::Wall {
            openings,
            ceiling_height_millimetres,
        } => fixture::VenueArchitecture::Wall {
            openings: openings.into_iter().map(application_opening).collect(),
            ceiling_height_millimetres,
        },
        wire::VenueArchitecture::Slab {
            purpose,
            outline_millimetres,
            elevation_millimetres,
            thickness_millimetres,
        } => fixture::VenueArchitecture::Slab {
            purpose: application_slab_purpose(purpose),
            outline_millimetres: outline_millimetres
                .into_iter()
                .map(|p| fixture::VenuePlanPoint { x: p.x, y: p.y })
                .collect(),
            elevation_millimetres,
            thickness_millimetres,
        },
        wire::VenueArchitecture::Pillar { shape } => fixture::VenueArchitecture::Pillar {
            shape: application_footprint(shape),
        },
        wire::VenueArchitecture::Table { shape } => fixture::VenueArchitecture::Table {
            shape: application_table_shape(shape),
        },
        wire::VenueArchitecture::Chair { style } => fixture::VenueArchitecture::Chair {
            style: application_chair_style(style),
        },
        wire::VenueArchitecture::Bar => fixture::VenueArchitecture::Bar,
    }
}
fn application_opening(value: wire::WallOpening) -> fixture::WallOpening {
    fixture::WallOpening {
        id: value.id,
        offset_millimetres: value.offset_millimetres,
        sill_millimetres: value.sill_millimetres,
        width_millimetres: value.width_millimetres,
        height_millimetres: value.height_millimetres,
        opening: match value.opening {
            wire::WallOpeningKind::Window { shape } => fixture::WallOpeningKind::Window {
                shape: application_window_shape(shape),
            },
            wire::WallOpeningKind::Door {
                leaves,
                swing,
                hinge,
            } => fixture::WallOpeningKind::Door {
                leaves: application_door_leaves(leaves),
                swing: application_door_swing(swing),
                hinge: application_door_hinge(hinge),
            },
        },
    }
}
fn application_slab_purpose(v: wire::VenueSlabPurpose) -> fixture::VenueSlabPurpose {
    match v {
        wire::VenueSlabPurpose::Roof => fixture::VenueSlabPurpose::Roof,
        wire::VenueSlabPurpose::Balcony => fixture::VenueSlabPurpose::Balcony,
    }
}
fn application_footprint(v: wire::VenueFootprint) -> fixture::VenueFootprint {
    match v {
        wire::VenueFootprint::Round => fixture::VenueFootprint::Round,
        wire::VenueFootprint::Square => fixture::VenueFootprint::Square,
        wire::VenueFootprint::Rectangular => fixture::VenueFootprint::Rectangular,
    }
}
fn application_table_shape(v: wire::VenueTableShape) -> fixture::VenueTableShape {
    match v {
        wire::VenueTableShape::Round => fixture::VenueTableShape::Round,
        wire::VenueTableShape::Rectangular => fixture::VenueTableShape::Rectangular,
    }
}
fn application_chair_style(v: wire::VenueChairStyle) -> fixture::VenueChairStyle {
    match v {
        wire::VenueChairStyle::BlackMetal => fixture::VenueChairStyle::BlackMetal,
        wire::VenueChairStyle::White => fixture::VenueChairStyle::White,
        wire::VenueChairStyle::BarStool => fixture::VenueChairStyle::BarStool,
    }
}
fn application_window_shape(v: wire::VenueWindowShape) -> fixture::VenueWindowShape {
    match v {
        wire::VenueWindowShape::Rectangular => fixture::VenueWindowShape::Rectangular,
        wire::VenueWindowShape::Round => fixture::VenueWindowShape::Round,
        wire::VenueWindowShape::Arched => fixture::VenueWindowShape::Arched,
    }
}
fn application_door_leaves(v: wire::VenueDoorLeaves) -> fixture::VenueDoorLeaves {
    match v {
        wire::VenueDoorLeaves::Single => fixture::VenueDoorLeaves::Single,
        wire::VenueDoorLeaves::Double => fixture::VenueDoorLeaves::Double,
    }
}
fn application_door_swing(v: wire::VenueDoorSwing) -> fixture::VenueDoorSwing {
    match v {
        wire::VenueDoorSwing::Inward => fixture::VenueDoorSwing::Inward,
        wire::VenueDoorSwing::Outward => fixture::VenueDoorSwing::Outward,
    }
}
fn application_door_hinge(v: wire::VenueDoorHinge) -> fixture::VenueDoorHinge {
    match v {
        wire::VenueDoorHinge::Left => fixture::VenueDoorHinge::Left,
        wire::VenueDoorHinge::Right => fixture::VenueDoorHinge::Right,
    }
}
pub(super) fn wire_architecture(value: fixture::VenueArchitecture) -> wire::VenueArchitecture {
    match value {
        fixture::VenueArchitecture::Wall {
            openings,
            ceiling_height_millimetres,
        } => wire::VenueArchitecture::Wall {
            openings: openings.into_iter().map(wire_opening).collect(),
            ceiling_height_millimetres,
        },
        fixture::VenueArchitecture::Slab {
            purpose,
            outline_millimetres,
            elevation_millimetres,
            thickness_millimetres,
        } => wire::VenueArchitecture::Slab {
            purpose: wire_slab_purpose(purpose),
            outline_millimetres: outline_millimetres
                .into_iter()
                .map(|p| wire::VenuePlanPoint { x: p.x, y: p.y })
                .collect(),
            elevation_millimetres,
            thickness_millimetres,
        },
        fixture::VenueArchitecture::Pillar { shape } => wire::VenueArchitecture::Pillar {
            shape: wire_footprint(shape),
        },
        fixture::VenueArchitecture::Table { shape } => wire::VenueArchitecture::Table {
            shape: wire_table_shape(shape),
        },
        fixture::VenueArchitecture::Chair { style } => wire::VenueArchitecture::Chair {
            style: wire_chair_style(style),
        },
        fixture::VenueArchitecture::Bar => wire::VenueArchitecture::Bar,
    }
}
fn wire_opening(value: fixture::WallOpening) -> wire::WallOpening {
    wire::WallOpening {
        id: value.id,
        offset_millimetres: value.offset_millimetres,
        sill_millimetres: value.sill_millimetres,
        width_millimetres: value.width_millimetres,
        height_millimetres: value.height_millimetres,
        opening: match value.opening {
            fixture::WallOpeningKind::Window { shape } => wire::WallOpeningKind::Window {
                shape: wire_window_shape(shape),
            },
            fixture::WallOpeningKind::Door {
                leaves,
                swing,
                hinge,
            } => wire::WallOpeningKind::Door {
                leaves: wire_door_leaves(leaves),
                swing: wire_door_swing(swing),
                hinge: wire_door_hinge(hinge),
            },
        },
    }
}
fn wire_slab_purpose(v: fixture::VenueSlabPurpose) -> wire::VenueSlabPurpose {
    match v {
        fixture::VenueSlabPurpose::Roof => wire::VenueSlabPurpose::Roof,
        fixture::VenueSlabPurpose::Balcony => wire::VenueSlabPurpose::Balcony,
    }
}
fn wire_footprint(v: fixture::VenueFootprint) -> wire::VenueFootprint {
    match v {
        fixture::VenueFootprint::Round => wire::VenueFootprint::Round,
        fixture::VenueFootprint::Square => wire::VenueFootprint::Square,
        fixture::VenueFootprint::Rectangular => wire::VenueFootprint::Rectangular,
    }
}
fn wire_table_shape(v: fixture::VenueTableShape) -> wire::VenueTableShape {
    match v {
        fixture::VenueTableShape::Round => wire::VenueTableShape::Round,
        fixture::VenueTableShape::Rectangular => wire::VenueTableShape::Rectangular,
    }
}
fn wire_chair_style(v: fixture::VenueChairStyle) -> wire::VenueChairStyle {
    match v {
        fixture::VenueChairStyle::BlackMetal => wire::VenueChairStyle::BlackMetal,
        fixture::VenueChairStyle::White => wire::VenueChairStyle::White,
        fixture::VenueChairStyle::BarStool => wire::VenueChairStyle::BarStool,
    }
}
fn wire_window_shape(v: fixture::VenueWindowShape) -> wire::VenueWindowShape {
    match v {
        fixture::VenueWindowShape::Rectangular => wire::VenueWindowShape::Rectangular,
        fixture::VenueWindowShape::Round => wire::VenueWindowShape::Round,
        fixture::VenueWindowShape::Arched => wire::VenueWindowShape::Arched,
    }
}
fn wire_door_leaves(v: fixture::VenueDoorLeaves) -> wire::VenueDoorLeaves {
    match v {
        fixture::VenueDoorLeaves::Single => wire::VenueDoorLeaves::Single,
        fixture::VenueDoorLeaves::Double => wire::VenueDoorLeaves::Double,
    }
}
fn wire_door_swing(v: fixture::VenueDoorSwing) -> wire::VenueDoorSwing {
    match v {
        fixture::VenueDoorSwing::Inward => wire::VenueDoorSwing::Inward,
        fixture::VenueDoorSwing::Outward => wire::VenueDoorSwing::Outward,
    }
}
fn wire_door_hinge(v: fixture::VenueDoorHinge) -> wire::VenueDoorHinge {
    match v {
        fixture::VenueDoorHinge::Left => wire::VenueDoorHinge::Left,
        fixture::VenueDoorHinge::Right => wire::VenueDoorHinge::Right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn architecture_wire_roundtrips_every_kind_and_opening_shape() {
        let id = "b2da4015-5cb5-4fd4-bf90-6ffeb9ff8751";
        let mut values = vec![
            json!({"kind":"bar"}),
            json!({"kind":"slab","purpose":"balcony","outline_millimetres":[{"x":0,"y":0},{"x":4000,"y":0},{"x":2000,"y":3000}],"elevation_millimetres":3000,"thickness_millimetres":300}),
        ];
        for shape in ["round", "square", "rectangular"] {
            values.push(json!({"kind":"pillar","shape":shape}));
        }
        for shape in ["round", "rectangular"] {
            values.push(json!({"kind":"table","shape":shape}));
        }
        for style in ["black_metal", "white", "bar_stool"] {
            values.push(json!({"kind":"chair","style":style}));
        }
        let mut opening_kinds = vec![];
        for shape in ["rectangular", "round", "arched"] {
            opening_kinds.push(json!({"kind":"window","shape":shape}));
        }
        for leaves in ["single", "double"] {
            for swing in ["inward", "outward"] {
                for hinge in ["left", "right"] {
                    opening_kinds
                        .push(json!({"kind":"door","leaves":leaves,"swing":swing,"hinge":hinge}));
                }
            }
        }
        for opening in opening_kinds {
            values.push(json!({"kind":"wall","ceiling_height_millimetres":4000,"openings":[{"id":id,"offset_millimetres":500,"sill_millimetres":0,"width_millimetres":1000,"height_millimetres":2200,"opening":opening}]}));
        }
        for value in values {
            let input: wire::VenueArchitecture = serde_json::from_value(value.clone()).unwrap();
            let options = crate::application_scenery_options(wire::PatchSceneryOptions {
                architecture: Some(input),
                ..Default::default()
            });
            let output = crate::wire_scenery_options(&options).unwrap();
            assert_eq!(serde_json::to_value(output.architecture).unwrap(), value);
        }
    }
}
