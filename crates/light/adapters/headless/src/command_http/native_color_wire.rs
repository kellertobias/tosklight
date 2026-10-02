//! TL-554 Direct Color edit options and adoption report conversions between wire and application.
use light_application as application;
use light_core::FixtureId;
use light_core::programming::{ColorIntent, VirtualColorAuthoringV1, VirtualColorRecipe};
use light_wire::v2::native_color as wire;

/// The Direct reference head and the explicit semantic starting colour of one intent. An
/// out-of-range starting colour is dropped, so the edit holds for an explicit start again.
pub(super) fn color_adoption_request(
    native_reference: Option<wire::NativeColorReferenceRef>,
    explicit_color_start: Option<wire::ExplicitColorStart>,
) -> application::ProgrammingColorAdoptionRequest {
    application::ProgrammingColorAdoptionRequest {
        native_reference: native_reference.map(|reference| {
            application::ProgrammingNativeReference {
                fixture_id: FixtureId(reference.fixture_id),
                head_id: reference.head_id,
            }
        }),
        explicit_start: explicit_color_start.and_then(|start| explicit_start(start.rgb)),
    }
}

/// An explicit virtual-recipe start: base XYZ follows the recipe (black stays black).
pub(super) fn explicit_start(rgb: [f32; 3]) -> Option<ColorIntent> {
    let recipe = VirtualColorRecipe {
        rgb,
        ..VirtualColorRecipe::default()
    };
    let base_xyz = VirtualColorAuthoringV1::recipe_xyz(&recipe).ok()?;
    Some(ColorIntent {
        base_xyz,
        recipe,
        ..ColorIntent::default()
    })
}

pub(super) fn color_adoption_report(
    report: application::ProgrammingColorAdoption,
) -> wire::ColorAdoptionReport {
    wire::ColorAdoptionReport {
        fixtures: report
            .fixtures
            .into_iter()
            .map(|fixture| wire::ColorAdoptionFixture {
                fixture_id: fixture.fixture_id.0,
                start: match fixture.start {
                    application::ProgrammingColorAdoptionStart::Approximate => {
                        wire::ColorAdoptionStart::Approximate
                    }
                    application::ProgrammingColorAdoptionStart::Explicit => {
                        wire::ColorAdoptionStart::Explicit
                    }
                },
                uv_unknown: fixture.uv_unknown,
            })
            .collect(),
        limitations: report.limitations,
    }
}
