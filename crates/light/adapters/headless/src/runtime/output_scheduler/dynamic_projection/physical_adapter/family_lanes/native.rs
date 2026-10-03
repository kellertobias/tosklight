//! TL-548 C3: native installation of every family's fitted writes in ONE engine call per frame
//! (`PreparedStaticFamilyFrame::project_family_native`, TL-548 C2). The engine validates each
//! owner's footprint on every physical instance, cross-family slot disjointness and same-family
//! agreement, and rejects the whole collection otherwise; a rejection fails the frame.
use super::*;

fn invalid(message: &str) -> TransitionError {
    IntentError(message.into()).into()
}

/// The family a sidecar variant may carry; a mismatch is a routing defect, never dropped.
fn variant_matches(row: &FamilySidecar) -> bool {
    matches!(
        (row, owner_family(row.owner())),
        (FamilySidecar::Position(_), PhysicalFamily::Position)
            | (FamilySidecar::Color(_), PhysicalFamily::Color)
            | (FamilySidecar::Optics(_), PhysicalFamily::Optics)
    )
}

/// Install every write of `rows` on the frame's semantic token. Rows must belong to
/// `frame_token`; a passive or held row without writes contributes nothing.
pub(super) fn project_family_native_rows(
    capture: &PreparedOutputFrame,
    frame_token: &CapturedFrameToken,
    token: &mut light_engine::PreparedStaticFamilyFrame,
    rows: &[FamilySidecar],
    (writes, memo): &mut (
        Vec<light_engine::FamilyNativeWrite>,
        light_engine::FamilyNativeMemo,
    ),
) -> Result<(), TransitionError> {
    if rows
        .iter()
        .any(|row| row.token() != frame_token || !variant_matches(row))
    {
        return Err(invalid(
            "native family sidecar belongs to another frame or family",
        ));
    }
    writes.clear();
    writes.extend(rows.iter().flat_map(|row| {
        row.writes()
            .iter()
            .map(move |write| light_engine::FamilyNativeWrite {
                owner: row.owner(),
                target: row.target(),
                instance_id: write.slot.destination.0,
                channel_index: write.slot.channel_index,
                channel_id: write.channel_id,
                function_id: write.function_id,
                split: write.slot.split,
                raw: write.raw,
            })
    }));
    let result = token
        .project_family_native_kept(capture, frame_token, writes, memo)
        .map_err(|error| invalid(&error.to_string()));
    writes.clear();
    result
}
