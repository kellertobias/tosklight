//! glTF node matrices must survive importers that decompose them into TRS. Architecture
//! triangles use edge vectors as their basis, so their shear must be baked into the mesh.
use glam::{DMat3, DMat4, DVec3};
use viz_render::GeometryMesh;

pub(super) fn bake_shear(mesh: &GeometryMesh, columns: [[f32; 4]; 4]) -> Option<GeometryMesh> {
    let transform = DMat4::from_cols_array_2d(&columns.map(|c| c.map(f64::from)));
    if !transform.is_finite() {
        return None;
    }
    let axes = [
        transform.x_axis.truncate(),
        transform.y_axis.truncate(),
        transform.z_axis.truncate(),
    ];
    // Relative angular tolerance keeps harmless f32 rotation roundoff on the shared TRS path.
    let sheared = [(0, 1), (0, 2), (1, 2)]
        .into_iter()
        .any(|(a, b)| axes[a].dot(axes[b]).abs() > 1e-5 * axes[a].length() * axes[b].length());
    if !sheared {
        return None;
    }
    let positions: Vec<_> = mesh
        .positions
        .iter()
        .map(|p| {
            transform
                .transform_point3(DVec3::from_array(p.map(f64::from)))
                .as_vec3()
                .to_array()
        })
        .collect();
    if positions.iter().flatten().any(|v| !v.is_finite()) {
        return None;
    }
    // Cofactors are determinant * inverse-transpose, without dividing by a potentially zero
    // determinant. Singular collapsed geometry gets a finite fallback normal, never NaNs.
    let cofactors = DMat3::from_cols(
        axes[1].cross(axes[2]),
        axes[2].cross(axes[0]),
        axes[0].cross(axes[1]),
    );
    let determinant = axes[0].dot(axes[1].cross(axes[2]));
    let sign = if determinant < 0.0 { -1.0 } else { 1.0 };
    let normals = mesh
        .normals
        .iter()
        .map(|n| {
            (cofactors * DVec3::from_array(n.map(f64::from)) * sign)
                .try_normalize()
                .unwrap_or(DVec3::Y)
                .as_vec3()
                .to_array()
        })
        .collect();
    let mut indices = mesh.indices.clone();
    // Mirrored TRS nodes normally reverse front-face winding at import. Keep that behavior
    // when the node becomes identity and its reflection moves into the positions instead.
    if determinant < 0.0 {
        for triangle in indices.chunks_exact_mut(3) {
            triangle.swap(1, 2);
        }
    }
    Some(GeometryMesh {
        name: mesh.name.clone(),
        positions,
        normals,
        indices,
    })
}
