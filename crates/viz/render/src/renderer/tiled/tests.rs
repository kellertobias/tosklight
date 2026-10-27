use super::*;
use crate::renderer::CULL_WGSL;
#[test]
fn per_light_bounds_shader_layout_and_both_compute_entries_validate() {
    let module = naga::front::wgsl::parse_str(CULL_WGSL).expect("cull WGSL parses");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("cull WGSL validates");
    let stored = module
        .types
        .iter()
        .find(|(_, ty)| ty.name.as_deref() == Some("StoredBounds"))
        .unwrap()
        .1;
    let naga::TypeInner::Struct { members, span } = &stored.inner else {
        panic!("stored bounds is a struct")
    };
    assert_eq!(u64::from(*span), BOUNDS_STRIDE);
    assert_eq!(members[0].offset, 0);
    assert_eq!(members[1].offset, 16);
    for entry in ["bounds_main", "cull_main"] {
        let entry = module
            .entry_points
            .iter()
            .find(|e| e.name == entry)
            .unwrap();
        assert_eq!(entry.stage, naga::ShaderStage::Compute);
        assert_eq!(entry.workgroup_size, [64, 1, 1]);
    }
}
#[path = "gpu.rs"]
mod gpu;
