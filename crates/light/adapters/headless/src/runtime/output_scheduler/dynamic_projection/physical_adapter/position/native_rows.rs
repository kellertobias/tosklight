//! Native Position projection of fitted sidecars (TL-548 C1), used by the Position-only observer.
//! The all-family observer installs every family at once (`family_lanes/native.rs`, TL-548 C3).
use super::*;

/// Install every Position write of `rows` on the frame's semantic token in one call (the engine
/// accepts one installation per frame). Rows must belong to `frame_token` and own Position.
pub(in crate::runtime) fn project_position_native_rows<'r>(
    capture: &light_engine::PreparedOutputFrame,
    frame_token: &CapturedFrameToken,
    token: &mut light_engine::PreparedStaticFamilyFrame,
    rows: impl Iterator<Item = &'r PhysicalHeadResult<PositionAdapter>> + Clone,
) -> Result<(), TransitionError> {
    if rows
        .clone()
        .any(|row| row.token != *frame_token || row.owner != ProgrammingOwner::Position)
    {
        return Err(invalid(
            "native Position sidecar belongs to another frame or owner",
        ));
    }
    let writes: Vec<_> = rows
        .flat_map(|row| {
            row.writes
                .iter()
                .map(|write| light_engine::PositionNativeWrite {
                    target: row.target,
                    instance_id: write.slot.destination.0,
                    channel_index: write.slot.channel_index,
                    channel_id: write.channel_id,
                    function_id: write.function_id,
                    split: write.slot.split,
                    raw: write.raw,
                })
        })
        .collect();
    token
        .project_position_native(capture, frame_token, &writes)
        .map_err(|error| invalid(error.to_string()))
}
