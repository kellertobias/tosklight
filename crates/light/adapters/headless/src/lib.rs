#![forbid(unsafe_code)]

pub mod highlight;
pub use light_application::programming_update as update;

mod runtime;
mod tolerant_json;

pub use runtime::run;

/// Narrow released-benchmark seam using the same Patch DTO conversion as the HTTP adapter.
pub fn benchmark_patch_application_command(
    show_id: light_core::ShowId,
    request: light_wire::v2::patch::PatchFixturesRequest,
) -> Result<light_application::PatchFixturesCommand, String> {
    runtime::show_patch_wire::application_command(show_id, request)
}

/// Narrow released-benchmark seam using the same Patch outcome conversion as the HTTP adapter.
pub fn benchmark_patch_wire_outcome(
    result: light_application::PatchFixturesResult,
) -> light_wire::v2::patch::PatchFixturesOutcome {
    runtime::show_patch_wire::wire_outcome(result)
}
