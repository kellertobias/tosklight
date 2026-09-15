use super::*;

const SQUARE_WITH_HOLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-1 -1 12 12">
  <title>test — top</title>
  <g id="silhouette" fill="#1b1f24"><path id="silhouette-body" fill-rule="evenodd" d="M0 0 L10 0 L10 10 L0 10 Z M3 3 L7 3 L7 7 L3 7 Z"/></g>
  <g id="lines" fill="none" stroke="#d7dde4">
    <g id="base"><path d="M0 0 H10 V10"/></g>
    <g id="hardware"><polyline points="1,1 2,2"/></g>
  </g>
  <g id="origin" stroke="#e5484d"><path d="M-20 0 H20 M0 -20 V20"/></g>
</svg>"##;

#[test]
fn a_drawing_reads_its_silhouette_and_every_line_group_but_not_the_origin() {
    let drawing = svg::parse(SQUARE_WITH_HOLE);

    assert_eq!(drawing.silhouette.len(), 2, "outline and hole");
    assert_eq!(
        drawing.lines,
        vec![
            vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)],
            vec![(1.0, 1.0), (2.0, 2.0)],
        ]
    );
}

#[test]
fn the_embedded_form_keeps_a_drawing_to_a_tenth_of_a_millimetre() {
    let drawing = svg::parse(SQUARE_WITH_HOLE);
    let decoded = svg::decode(&svg::encode(&drawing)).expect("decodes");

    assert_eq!(decoded.silhouette.len(), drawing.silhouette.len());
    assert_eq!(decoded.lines.len(), drawing.lines.len());
    for (read, written) in decoded
        .silhouette
        .iter()
        .chain(&decoded.lines)
        .flatten()
        .zip(drawing.silhouette.iter().chain(&drawing.lines).flatten())
    {
        assert!((read.0 - written.0).abs() <= 0.05 && (read.1 - written.1).abs() <= 0.05);
    }
    assert!(
        svg::decode(&[1, 2]).is_none(),
        "a cut-short blob is refused"
    );
}

#[test]
fn an_even_odd_silhouette_is_filled_around_its_hole() {
    let drawing = svg::parse(SQUARE_WITH_HOLE);
    let area = triangulate(&drawing.silhouette)
        .iter()
        .map(|[a, b, c]| (*b - *a).perp_dot(*c - *a).abs() * 0.5)
        .sum::<f32>();

    assert!(
        (area - 84.0).abs() < 1e-3,
        "10 x 10 less the 4 x 4 hole, got {area}"
    );
}

#[test]
fn every_default_model_drawing_on_disk_is_embedded() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../assets/models/2d");
    let groups = std::fs::read_dir(root).expect("shipped drawings");
    let directories = groups
        .flatten()
        .flat_map(|group| {
            std::fs::read_dir(group.path())
                .into_iter()
                .flatten()
                .flatten()
        })
        .map(|model| model.path())
        .collect::<Vec<_>>();
    let mut embedded = 0;
    for model in crate::default_model::all() {
        for view in ["top", "front", "side"] {
            let on_disk = directories.iter().any(|directory| {
                directory.file_name().is_some_and(|name| name == model.name)
                    && directory.join(format!("{view}.svg")).is_file()
            });
            if !on_disk {
                continue;
            }
            embedded += 1;
            assert!(
                has_model_drawing(model.name, view),
                "{} has no embedded {view} drawing",
                model.name
            );
        }
    }
    assert!(
        embedded > 100,
        "only {embedded} drawings were found on disk"
    );
}

#[test]
fn a_models_drawings_cover_the_plan_and_all_four_elevations() {
    let artwork = model_drawing_artwork("moving-head-profile", 1.0);
    let views = artwork
        .iter()
        .map(|artwork| artwork.view)
        .collect::<Vec<_>>();

    assert_eq!(
        views,
        [
            ProjectionView::Top,
            ProjectionView::Front,
            ProjectionView::Back,
            ProjectionView::Left,
            ProjectionView::Right,
        ]
    );
    for view in &artwork {
        assert!(!view.indices.is_empty(), "{:?} has a silhouette", view.view);
        assert!(!view.lines.is_empty(), "{:?} has linework", view.view);
        assert_eq!(view.lines.len() % 2, 0, "lines are segment pairs");
        let normal = Vec3::from(view.normals[0]);
        for triangle in view.indices.as_chunks::<3>().0 {
            let [a, b, c] = triangle.map(|at| Vec3::from(view.vertices[at as usize]));
            assert!((b - a).cross(c - a).dot(normal) >= 0.0, "faces its viewer");
        }
    }
    // A mirrored elevation is the same geometry seen from the other side.
    assert_eq!(artwork[1].vertices.len(), artwork[2].vertices.len());
    assert_eq!(artwork[1].lines, artwork[2].lines);
    assert_eq!(artwork[1].normals[0], [0.0, 0.0, 1.0]);
    assert_eq!(artwork[2].normals[0], [0.0, 0.0, -1.0]);
}

#[test]
fn a_drawing_is_placed_at_the_scale_its_body_is_drawn_at() {
    let extent = |artwork: &PlanArtwork| {
        artwork
            .lines
            .iter()
            .map(|point| Vec3::from(*point).abs().max_element())
            .fold(0.0_f32, f32::max)
    };
    let natural = model_drawing_artwork("par-64-short-nose-black", 1.0);
    let doubled = model_drawing_artwork("par-64-short-nose-black", 2.0);

    assert!((extent(&doubled[0]) - 2.0 * extent(&natural[0])).abs() < 1e-3);
    assert!(extent(&natural[0]) > 0.1, "millimetres became metres");
}

#[test]
fn a_model_without_a_drawing_yields_no_artwork() {
    assert!(model_drawing_artwork("no-such-model", 1.0).is_empty());
    assert!(!has_model_drawing("no-such-model", "top"));
}
