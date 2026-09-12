//! ASCII DXF, read as far as a placed plan needs.
//!
//! A DXF file is a flat stream of (group code, value) pairs. Entities begin at a `0` pair, so the
//! reader groups the stream into records and converts the ones that describe a line: `LINE`,
//! `LWPOLYLINE`, `POLYLINE`, `CIRCLE`, `ARC` and `ELLIPSE`, plus `INSERT`, which repeats a block's
//! entities under a transform. Venue plans are drawn almost entirely out of blocks, so a reader
//! that skipped them would open nearly empty.
//!
//! Text, dimensions, hatches and splines are deliberately not drawn: they need a font, a fill rule
//! or a NURBS evaluator that a backdrop does not justify.

use std::collections::HashMap;

use crate::{
    DrawingError, DrawingUnits, FLATTEN_TOLERANCE_MM, Polyline, UnderlayGeometry, arc_points,
    finish,
};

/// How deep one block may nest inside another before the reader gives up on a cycle.
const MAX_BLOCK_DEPTH: usize = 16;

#[derive(Clone, Debug)]
struct Pair {
    code: i32,
    value: String,
}

/// One entity or block header: the `0` pair's value, and every pair up to the next `0`.
#[derive(Clone, Debug, Default)]
struct Record {
    kind: String,
    pairs: Vec<Pair>,
}

impl Record {
    fn number(&self, code: i32) -> Option<f64> {
        self.pairs
            .iter()
            .find(|pair| pair.code == code)
            .and_then(|pair| pair.value.parse().ok())
    }

    fn value(&self, code: i32) -> Option<&str> {
        self.pairs
            .iter()
            .find(|pair| pair.code == code)
            .map(|pair| pair.value.as_str())
    }

    fn flag(&self, code: i32, bit: i64) -> bool {
        self.number(code)
            .is_some_and(|value| (value as i64) & bit != 0)
    }

    fn layer(&self) -> &str {
        self.value(8).unwrap_or("0")
    }

    /// Vertex list of an `LWPOLYLINE`, whose 10/20 pairs repeat, with each vertex's bulge.
    fn vertices(&self) -> Vec<(f64, f64, f64)> {
        let mut vertices: Vec<(f64, f64, f64)> = Vec::new();
        for pair in &self.pairs {
            let Ok(number) = pair.value.parse::<f64>() else {
                continue;
            };
            match pair.code {
                10 => vertices.push((number, 0.0, 0.0)),
                20 => {
                    if let Some(last) = vertices.last_mut() {
                        last.1 = number;
                    }
                }
                42 => {
                    if let Some(last) = vertices.last_mut() {
                        last.2 = number;
                    }
                }
                _ => {}
            }
        }
        vertices
    }
}

/// A DXF transform: `INSERT` scales, rotates and moves a block's entities.
#[derive(Clone, Copy, Debug)]
struct Affine {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Affine {
    const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    fn insert(offset: [f64; 2], scale: [f64; 2], rotation_degrees: f64) -> Self {
        let (sin, cos) = rotation_degrees.to_radians().sin_cos();
        Self {
            a: cos * scale[0],
            b: sin * scale[0],
            c: -sin * scale[1],
            d: cos * scale[1],
            e: offset[0],
            f: offset[1],
        }
    }

    /// `self` applied after `inner`, so a nested block carries its parent's placement.
    fn then(self, inner: Self) -> Self {
        Self {
            a: self.a * inner.a + self.c * inner.b,
            b: self.b * inner.a + self.d * inner.b,
            c: self.a * inner.c + self.c * inner.d,
            d: self.b * inner.c + self.d * inner.d,
            e: self.a * inner.e + self.c * inner.f + self.e,
            f: self.b * inner.e + self.d * inner.f + self.f,
        }
    }

    fn apply(self, point: [f64; 2]) -> [f64; 2] {
        [
            self.a * point[0] + self.c * point[1] + self.e,
            self.b * point[0] + self.d * point[1] + self.f,
        ]
    }

    /// How much this transform stretches lengths, so flattening stays fine enough after scaling.
    fn magnitude(self) -> f64 {
        self.a.hypot(self.b).max(self.c.hypot(self.d)).max(1e-9)
    }
}

fn pairs(text: &str) -> Vec<Pair> {
    let mut pairs = Vec::new();
    let mut lines = text.lines();
    while let Some(code) = lines.next() {
        let Ok(code) = code.trim().parse::<i32>() else {
            continue;
        };
        let Some(value) = lines.next() else { break };
        if code == 999 {
            continue;
        }
        pairs.push(Pair {
            code,
            value: value.trim().to_owned(),
        });
    }
    pairs
}

fn units_from(value: f64) -> DrawingUnits {
    match value as i64 {
        1 => DrawingUnits::Inches,
        2 => DrawingUnits::Feet,
        4 => DrawingUnits::Millimetres,
        5 => DrawingUnits::Centimetres,
        6 => DrawingUnits::Metres,
        _ => DrawingUnits::Assumed,
    }
}

/// What the reader keeps of a file: the unit it is drawn in, its blocks, and its entities.
#[derive(Default)]
struct Document {
    units: DrawingUnits,
    blocks: HashMap<String, Vec<Record>>,
    entities: Vec<Record>,
}

/// The `$INSUNITS` header variable, which names the unit the drawing's numbers are in.
fn units_of(header: &Record) -> DrawingUnits {
    let mut expecting = false;
    for pair in &header.pairs {
        if pair.code == 9 {
            expecting = pair.value == "$INSUNITS";
        } else if expecting && pair.code == 70 {
            return pair
                .value
                .parse()
                .map(units_from)
                .unwrap_or(DrawingUnits::Assumed);
        }
    }
    DrawingUnits::Assumed
}

/// Group the pair stream into records, and the records into the header, the blocks and the
/// entities. Every record — `SECTION` and `ENDBLK` included — closes when the next `0` pair opens
/// the one after it, so one loop handles the whole file.
fn read(text: &str) -> Document {
    let mut document = Document::default();
    let mut section = String::new();
    let mut current: Option<Record> = None;
    let mut block: Option<(String, Vec<Record>)> = None;

    for pair in pairs(text) {
        if pair.code != 0 {
            if let Some(record) = current.as_mut() {
                record.pairs.push(pair);
            }
            continue;
        }
        if let Some(record) = current.take() {
            match record.kind.as_str() {
                // The header's variables have no records of their own: they accumulate into the
                // SECTION record that names the section, and are read back out of it here.
                "SECTION" => {
                    section = record.value(2).unwrap_or_default().to_owned();
                    if section == "HEADER" {
                        document.units = units_of(&record);
                    }
                }
                "ENDSEC" => section.clear(),
                "BLOCK" if section == "BLOCKS" => {
                    block = Some((record.value(2).unwrap_or_default().to_owned(), Vec::new()));
                }
                "ENDBLK" => {
                    if let Some((name, records)) = block.take() {
                        document.blocks.insert(name, records);
                    }
                }
                _ => match section.as_str() {
                    "ENTITIES" => document.entities.push(record),
                    "BLOCKS" => {
                        if let Some((_, records)) = block.as_mut() {
                            records.push(record);
                        }
                    }
                    _ => {}
                },
            }
        }
        if pair.value == "EOF" {
            break;
        }
        current = Some(Record {
            kind: pair.value,
            pairs: Vec::new(),
        });
    }
    document
}

fn polyline_from_vertices(
    vertices: &[(f64, f64, f64)],
    closed: bool,
    tolerance: f64,
) -> Vec<[f64; 2]> {
    let mut points: Vec<[f64; 2]> = Vec::new();
    let count = vertices.len();
    for index in 0..count {
        let (x, y, bulge) = vertices[index];
        points.push([x, y]);
        let next = if index + 1 < count {
            Some(vertices[index + 1])
        } else if closed && count > 2 {
            Some(vertices[0])
        } else {
            None
        };
        let Some((next_x, next_y, _)) = next else {
            continue;
        };
        if bulge.abs() < 1e-9 {
            continue;
        }
        // A bulge is tan(sweep / 4) of the arc that replaces this segment.
        let sweep = 4.0 * bulge.atan();
        let chord = (next_x - x).hypot(next_y - y);
        if chord < 1e-9 {
            continue;
        }
        let radius = chord / (2.0 * (sweep / 2.0).sin()).abs();
        let midpoint = [(x + next_x) / 2.0, (y + next_y) / 2.0];
        let height = radius * (sweep / 2.0).cos() * if bulge < 0.0 { -1.0 } else { 1.0 };
        let normal = [-(next_y - y) / chord, (next_x - x) / chord];
        let centre = [
            midpoint[0] + normal[0] * height,
            midpoint[1] + normal[1] * height,
        ];
        let start = (y - centre[1]).atan2(x - centre[0]);
        let arc = arc_points(centre, radius, start, sweep, tolerance);
        if arc.len() > 2 {
            points.extend(arc[1..arc.len() - 1].iter().copied());
        }
    }
    points
}

fn ellipse_points(record: &Record, tolerance: f64) -> Vec<[f64; 2]> {
    let centre = [
        record.number(10).unwrap_or_default(),
        record.number(20).unwrap_or_default(),
    ];
    let major = [
        record.number(11).unwrap_or_default(),
        record.number(21).unwrap_or_default(),
    ];
    let ratio = record.number(40).unwrap_or(1.0);
    let start = record.number(41).unwrap_or(0.0);
    let end = record.number(42).unwrap_or(std::f64::consts::TAU);
    let radius = major[0].hypot(major[1]);
    if radius <= 0.0 {
        return Vec::new();
    }
    let rotation = major[1].atan2(major[0]);
    let (sin, cos) = rotation.sin_cos();
    // Flatten as a circle of the major radius, then squash the minor axis: the segment count comes
    // from the larger radius, so the tighter end of the ellipse is at least as finely drawn.
    arc_points([0.0, 0.0], radius, start, end - start, tolerance)
        .into_iter()
        .map(|point| {
            let x = point[0];
            let y = point[1] * ratio;
            [centre[0] + x * cos - y * sin, centre[1] + x * sin + y * cos]
        })
        .collect()
}

fn emit(
    records: &[Record],
    blocks: &HashMap<String, Vec<Record>>,
    transform: Affine,
    depth: usize,
    out: &mut Vec<Polyline>,
) {
    let tolerance = FLATTEN_TOLERANCE_MM / transform.magnitude();
    for record in records {
        let layer = record.layer();
        let (points, closed) = match record.kind.as_str() {
            "LINE" => (
                vec![
                    [
                        record.number(10).unwrap_or_default(),
                        record.number(20).unwrap_or_default(),
                    ],
                    [
                        record.number(11).unwrap_or_default(),
                        record.number(21).unwrap_or_default(),
                    ],
                ],
                false,
            ),
            "LWPOLYLINE" | "POLYLINE" => {
                let closed = record.flag(70, 1);
                (
                    polyline_from_vertices(&record.vertices(), closed, tolerance),
                    closed,
                )
            }
            "CIRCLE" => (
                arc_points(
                    [
                        record.number(10).unwrap_or_default(),
                        record.number(20).unwrap_or_default(),
                    ],
                    record.number(40).unwrap_or_default(),
                    0.0,
                    std::f64::consts::TAU,
                    tolerance,
                ),
                true,
            ),
            "ARC" => {
                let start = record.number(50).unwrap_or_default().to_radians();
                let end = record.number(51).unwrap_or_default().to_radians();
                let sweep = if end >= start {
                    end - start
                } else {
                    end + std::f64::consts::TAU - start
                };
                (
                    arc_points(
                        [
                            record.number(10).unwrap_or_default(),
                            record.number(20).unwrap_or_default(),
                        ],
                        record.number(40).unwrap_or_default(),
                        start,
                        sweep,
                        tolerance,
                    ),
                    false,
                )
            }
            "ELLIPSE" => (ellipse_points(record, tolerance), false),
            "INSERT" => {
                if depth >= MAX_BLOCK_DEPTH {
                    continue;
                }
                let Some(name) = record.value(2) else {
                    continue;
                };
                let Some(block) = blocks.get(name) else {
                    continue;
                };
                let inner = Affine::insert(
                    [
                        record.number(10).unwrap_or_default(),
                        record.number(20).unwrap_or_default(),
                    ],
                    [
                        record.number(41).unwrap_or(1.0),
                        record.number(42).unwrap_or(1.0),
                    ],
                    record.number(50).unwrap_or_default(),
                );
                emit(block, blocks, transform.then(inner), depth + 1, out);
                continue;
            }
            _ => continue,
        };
        if points.len() < 2 {
            continue;
        }
        out.push(Polyline::new(
            points
                .into_iter()
                .map(|point| transform.apply(point))
                .collect(),
            closed,
            layer,
        ));
    }
}

/// Read an ASCII DXF drawing into millimetre polylines.
pub fn parse_dxf(name: &str, bytes: &[u8]) -> Result<UnderlayGeometry, DrawingError> {
    let text = String::from_utf8_lossy(bytes);
    if !text.contains("SECTION") {
        return Err(DrawingError::Malformed {
            name: name.to_owned(),
            detail: "no DXF sections were found; a binary DXF has to be saved as ASCII DXF first"
                .to_owned(),
        });
    }
    let document = read(&text);
    let mut polylines = Vec::new();
    emit(
        &document.entities,
        &document.blocks,
        Affine::IDENTITY,
        0,
        &mut polylines,
    );
    finish(name, polylines, document.units)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dxf(units: &str, blocks: &str, entities: &str) -> String {
        format!(
            "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n{units}\n0\nENDSEC\n\
             0\nSECTION\n2\nBLOCKS\n{blocks}0\nENDSEC\n\
             0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n"
        )
    }

    #[test]
    fn reads_lines_and_the_layer_they_are_on() {
        let source = dxf(
            "4",
            "",
            "0\nLINE\n8\nA-WALL\n10\n0\n20\n0\n11\n1000\n21\n500\n",
        );
        let geometry = parse_dxf("plan.dxf", source.as_bytes()).expect("geometry");
        assert_eq!(geometry.polylines.len(), 1);
        assert_eq!(geometry.polylines[0].layer, "A-WALL");
        assert_eq!(
            geometry.polylines[0].points,
            vec![[0.0, 0.0], [1000.0, 500.0]]
        );
        assert_eq!(geometry.units, DrawingUnits::Millimetres);
        assert_eq!(geometry.extents_millimetres, [0.0, 0.0, 1000.0, 500.0]);
    }

    #[test]
    fn inches_in_the_header_scale_the_drawing() {
        let source = dxf("1", "", "0\nLINE\n10\n0\n20\n0\n11\n12\n21\n0\n");
        let geometry = parse_dxf("plan.dxf", source.as_bytes()).expect("geometry");
        assert_eq!(geometry.units, DrawingUnits::Inches);
        assert!((geometry.polylines[0].points[1][0] - 304.8).abs() < 1e-6);
    }

    #[test]
    fn an_insert_places_its_block_where_the_transform_says() {
        let blocks = "0\nBLOCK\n2\nCHAIR\n10\n0\n20\n0\n\
                      0\nLINE\n10\n0\n20\n0\n11\n100\n21\n0\n\
                      0\nENDBLK\n";
        let entities = "0\nINSERT\n2\nCHAIR\n10\n500\n20\n250\n41\n2\n42\n2\n50\n90\n";
        let geometry =
            parse_dxf("plan.dxf", dxf("4", blocks, entities).as_bytes()).expect("geometry");
        assert_eq!(geometry.polylines.len(), 1);
        let points = &geometry.polylines[0].points;
        assert!((points[0][0] - 500.0).abs() < 1e-6);
        assert!((points[0][1] - 250.0).abs() < 1e-6);
        // 100 long, scaled twice and turned a quarter turn: straight up from the insertion point.
        assert!((points[1][0] - 500.0).abs() < 1e-6, "{points:?}");
        assert!((points[1][1] - 450.0).abs() < 1e-6, "{points:?}");
    }

    #[test]
    fn a_block_inside_a_block_compounds_both_transforms() {
        let blocks = "0\nBLOCK\n2\nLEG\n10\n0\n20\n0\n\
                      0\nLINE\n10\n0\n20\n0\n11\n10\n21\n0\n\
                      0\nENDBLK\n\
                      0\nBLOCK\n2\nTABLE\n10\n0\n20\n0\n\
                      0\nINSERT\n2\nLEG\n10\n100\n20\n0\n41\n2\n42\n2\n\
                      0\nENDBLK\n";
        let entities = "0\nINSERT\n2\nTABLE\n10\n1000\n20\n0\n41\n3\n42\n3\n";
        let geometry =
            parse_dxf("plan.dxf", dxf("4", blocks, entities).as_bytes()).expect("geometry");
        let points = &geometry.polylines[0].points;
        // leg starts at 100 inside the table, table sits at 1000 and is tripled: 1000 + 300.
        assert!((points[0][0] - 1300.0).abs() < 1e-6, "{points:?}");
        // the leg is 10 long, doubled by the table and tripled by the insert.
        assert!((points[1][0] - 1360.0).abs() < 1e-6, "{points:?}");
    }

    #[test]
    fn a_closed_polyline_keeps_its_vertices_and_closed_flag() {
        let entities =
            "0\nLWPOLYLINE\n8\nSTAGE\n90\n3\n70\n1\n10\n0\n20\n0\n10\n100\n20\n0\n10\n0\n20\n100\n";
        let geometry = parse_dxf("plan.dxf", dxf("4", "", entities).as_bytes()).expect("geometry");
        assert!(geometry.polylines[0].closed);
        assert_eq!(geometry.polylines[0].points.len(), 3);
    }

    #[test]
    fn a_circle_becomes_a_closed_run_of_segments() {
        let entities = "0\nCIRCLE\n10\n0\n20\n0\n40\n500\n";
        let geometry = parse_dxf("plan.dxf", dxf("4", "", entities).as_bytes()).expect("geometry");
        assert!(geometry.polylines[0].points.len() > 8);
        assert!(geometry.polylines[0].closed);
        for point in &geometry.polylines[0].points {
            assert!((point[0].hypot(point[1]) - 500.0).abs() < 1e-6);
        }
    }

    #[test]
    fn text_and_hatches_are_skipped_without_failing_the_file() {
        let entities = "0\nTEXT\n10\n0\n20\n0\n1\nSTAGE LEFT\n\
                        0\nLINE\n10\n0\n20\n0\n11\n10\n21\n0\n";
        let geometry = parse_dxf("plan.dxf", dxf("4", "", entities).as_bytes()).expect("geometry");
        assert_eq!(geometry.polylines.len(), 1);
    }

    #[test]
    fn a_file_that_is_not_dxf_is_refused_by_name() {
        let error = parse_dxf("plan.dxf", b"not a drawing").expect_err("refused");
        assert!(error.to_string().contains("plan.dxf"));
    }
}
