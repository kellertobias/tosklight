use light_core::FixtureId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

/// Stage coordinates used by selection-grid projection.
///
/// X increases left-to-right, Y increases into the room, and Z increases bottom-to-top.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct StageGridPosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// Operator-authored 2D Stage coordinates. They are independent from the 3D Stage position:
/// manual 2D placement must not be reconstructed from XYZ.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct StageGridPosition2d {
    pub x: f64,
    pub y: f64,
}

impl StageGridPosition2d {
    fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl StageGridPosition {
    fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
pub struct AxisOrigin {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl PartialEq for AxisOrigin {
    fn eq(&self, other: &Self) -> bool {
        self.x.to_bits() == other.x.to_bits()
            && self.y.to_bits() == other.y.to_bits()
            && self.z.to_bits() == other.z.to_bits()
    }
}

impl Eq for AxisOrigin {}

impl Hash for AxisOrigin {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.x.to_bits().hash(state);
        self.y.to_bits().hash(state);
        self.z.to_bits().hash(state);
    }
}

impl AxisOrigin {
    fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

/// Stage-derived grid methods in their authoritative `[SHIFT] [ALL]` cycling order.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GridMethod {
    #[default]
    Stage2d,
    TopToBottom,
    BottomToTop,
    FrontToBack,
    BackToFront,
    LeftToRight,
    RightToLeft,
    HorizontalAxisX,
    VerticalAxisZ,
    RoomDepthAxisY,
}

impl GridMethod {
    pub const ALL: [Self; 10] = [
        Self::Stage2d,
        Self::TopToBottom,
        Self::BottomToTop,
        Self::FrontToBack,
        Self::BackToFront,
        Self::LeftToRight,
        Self::RightToLeft,
        Self::HorizontalAxisX,
        Self::VerticalAxisZ,
        Self::RoomDepthAxisY,
    ];

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Stage2d => Self::TopToBottom,
            Self::TopToBottom => Self::BottomToTop,
            Self::BottomToTop => Self::FrontToBack,
            Self::FrontToBack => Self::BackToFront,
            Self::BackToFront => Self::LeftToRight,
            Self::LeftToRight => Self::RightToLeft,
            Self::RightToLeft => Self::HorizontalAxisX,
            Self::HorizontalAxisX => Self::VerticalAxisZ,
            Self::VerticalAxisZ => Self::RoomDepthAxisY,
            Self::RoomDepthAxisY => Self::Stage2d,
        }
    }

    #[must_use]
    pub const fn uses_axis_origin(self) -> bool {
        matches!(
            self,
            Self::HorizontalAxisX | Self::VerticalAxisZ | Self::RoomDepthAxisY
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct GridMethodConfiguration {
    pub method: GridMethod,
    #[serde(default)]
    pub axis_origin: AxisOrigin,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PositionedFixture {
    pub fixture_id: FixtureId,
    /// The authored/automatic 2D Stage position used exclusively by [`GridMethod::Stage2d`].
    pub position_2d: Option<StageGridPosition2d>,
    /// The XYZ Stage position used by all directional and cylindrical methods.
    pub position_3d: Option<StageGridPosition>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GridCell {
    pub fixture_id: FixtureId,
    /// Zero is the visually topmost row.
    pub row: usize,
    /// Zero is the visually leftmost column.
    pub column: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SelectionGrid {
    pub cells: Vec<GridCell>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GridConstructionError {
    NonFiniteAxisOrigin,
    NonFinitePosition { fixture_id: FixtureId },
}

#[derive(Clone, Copy, Debug)]
struct Projection {
    fixture_id: FixtureId,
    horizontal: f64,
    vertical: f64,
}

impl SelectionGrid {
    pub fn from_stage_positions(
        fixtures: &[PositionedFixture],
        configuration: GridMethodConfiguration,
    ) -> Result<Self, GridConstructionError> {
        if configuration.method.uses_axis_origin() && !configuration.axis_origin.is_finite() {
            return Err(GridConstructionError::NonFiniteAxisOrigin);
        }

        let mut projected = Vec::with_capacity(fixtures.len());
        let mut missing = Vec::new();
        for fixture in fixtures {
            let (horizontal, vertical) = if configuration.method == GridMethod::Stage2d {
                let Some(position) = fixture.position_2d else {
                    missing.push(fixture.fixture_id);
                    continue;
                };
                if !position.is_finite() {
                    return Err(GridConstructionError::NonFinitePosition {
                        fixture_id: fixture.fixture_id,
                    });
                }
                (position.x, -position.y)
            } else {
                let Some(position) = fixture.position_3d else {
                    missing.push(fixture.fixture_id);
                    continue;
                };
                if !position.is_finite() {
                    return Err(GridConstructionError::NonFinitePosition {
                        fixture_id: fixture.fixture_id,
                    });
                }
                project(configuration.method, configuration.axis_origin, position)
            };
            projected.push(Projection {
                fixture_id: fixture.fixture_id,
                horizontal,
                vertical,
            });
        }

        let mut horizontal_values = projected
            .iter()
            .map(|item| item.horizontal)
            .collect::<Vec<_>>();
        horizontal_values.sort_by(f64::total_cmp);
        horizontal_values.dedup_by(|left, right| left.total_cmp(right).is_eq());

        let mut vertical_values = projected
            .iter()
            .map(|item| item.vertical)
            .collect::<Vec<_>>();
        vertical_values.sort_by(|left, right| right.total_cmp(left));
        vertical_values.dedup_by(|left, right| left.total_cmp(right).is_eq());

        // Exact-position ties expand only their horizontal rank. Width is reserved for the largest
        // tie at that horizontal coordinate, so other rows retain vertical alignment and holes
        // remain visible rather than being compacted away.
        let mut ties = BTreeMap::<usize, BTreeMap<usize, Vec<FixtureId>>>::new();
        for item in &projected {
            let horizontal_rank = rank_ascending(&horizontal_values, item.horizontal);
            let vertical_rank = rank_descending(&vertical_values, item.vertical);
            ties.entry(horizontal_rank)
                .or_default()
                .entry(vertical_rank)
                .or_default()
                .push(item.fixture_id);
        }
        for rows in ties.values_mut() {
            for fixtures in rows.values_mut() {
                fixtures.sort_by_key(fixture_identity);
            }
        }

        let widths = (0..horizontal_values.len())
            .map(|horizontal_rank| {
                ties.get(&horizontal_rank)
                    .and_then(|rows| rows.values().map(Vec::len).max())
                    .unwrap_or(1)
            })
            .collect::<Vec<_>>();
        let column_offsets = widths
            .iter()
            .scan(0usize, |offset, width| {
                let current = *offset;
                *offset += width;
                Some(current)
            })
            .collect::<Vec<_>>();

        let mut cells = Vec::with_capacity(fixtures.len());
        for (horizontal_rank, rows) in ties {
            for (row, tied_fixtures) in rows {
                for (tie_rank, fixture_id) in tied_fixtures.into_iter().enumerate() {
                    cells.push(GridCell {
                        fixture_id,
                        row,
                        column: column_offsets[horizontal_rank] + tie_rank,
                    });
                }
            }
        }

        missing.sort_by_key(fixture_identity);
        let overflow_row = vertical_values.len();
        for (column, fixture_id) in missing.into_iter().enumerate() {
            cells.push(GridCell {
                fixture_id,
                row: overflow_row,
                column,
            });
        }
        cells.sort_by_key(|cell| (cell.row, cell.column));
        Ok(Self { cells })
    }

    #[must_use]
    pub fn rows_first(&self, traversal: RowsFirstTraversal) -> Vec<FixtureId> {
        let mut cells = self.cells.clone();
        cells.sort_by_key(|cell| match traversal {
            RowsFirstTraversal::TopLeft => (cell.row, cell.column),
            RowsFirstTraversal::TopRight => (cell.row, usize::MAX - cell.column),
            RowsFirstTraversal::BottomLeft => (usize::MAX - cell.row, cell.column),
            RowsFirstTraversal::BottomRight => (usize::MAX - cell.row, usize::MAX - cell.column),
        });
        cells.into_iter().map(|cell| cell.fixture_id).collect()
    }

    #[must_use]
    pub fn columns_first(&self, traversal: ColumnsFirstTraversal) -> Vec<FixtureId> {
        let mut cells = self.cells.clone();
        cells.sort_by_key(|cell| match traversal {
            ColumnsFirstTraversal::TopLeft => (cell.column, cell.row),
            ColumnsFirstTraversal::BottomLeft => (cell.column, usize::MAX - cell.row),
            ColumnsFirstTraversal::TopRight => (usize::MAX - cell.column, cell.row),
            ColumnsFirstTraversal::BottomRight => (usize::MAX - cell.column, usize::MAX - cell.row),
        });
        cells.into_iter().map(|cell| cell.fixture_id).collect()
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RowsFirstTraversal {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl RowsFirstTraversal {
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::TopLeft => Self::TopRight,
            Self::TopRight => Self::BottomLeft,
            Self::BottomLeft => Self::BottomRight,
            Self::BottomRight => Self::TopLeft,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnsFirstTraversal {
    #[default]
    TopLeft,
    BottomLeft,
    TopRight,
    BottomRight,
}

/// The two independent operator traversal families.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GridTraversalAxis {
    Rows,
    Columns,
}

impl ColumnsFirstTraversal {
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::TopLeft => Self::BottomLeft,
            Self::BottomLeft => Self::TopRight,
            Self::TopRight => Self::BottomRight,
            Self::BottomRight => Self::TopLeft,
        }
    }
}

fn project(method: GridMethod, origin: AxisOrigin, position: StageGridPosition) -> (f64, f64) {
    let x = position.x - origin.x;
    let y = position.y - origin.y;
    let z = position.z - origin.z;
    match method {
        // Stage's 2D Y coordinate grows down-screen, so negate it into the module's
        // positive-is-up vertical convention.
        GridMethod::Stage2d => unreachable!("2D Stage projection uses its independent position"),
        GridMethod::TopToBottom => (position.x, -position.y),
        // Looking from the opposite end of an axis mirrors the view horizontally.
        GridMethod::BottomToTop => (-position.x, -position.y),
        GridMethod::FrontToBack => (position.x, position.z),
        GridMethod::BackToFront => (-position.x, position.z),
        GridMethod::LeftToRight => (position.y, position.z),
        GridMethod::RightToLeft => (-position.y, position.z),
        GridMethod::HorizontalAxisX => (position.x, z.atan2(y)),
        GridMethod::VerticalAxisZ => (y.atan2(x), position.z),
        GridMethod::RoomDepthAxisY => (z.atan2(x), -position.y),
    }
}

fn rank_ascending(values: &[f64], value: f64) -> usize {
    values
        .binary_search_by(|candidate| candidate.total_cmp(&value))
        .expect("projected value came from the ranked set")
}

fn rank_descending(values: &[f64], value: f64) -> usize {
    values
        .binary_search_by(|candidate| candidate.total_cmp(&value).reverse())
        .expect("projected value came from the ranked set")
}

fn fixture_identity(fixture_id: &FixtureId) -> [u8; 16] {
    *fixture_id.0.as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn fixture(number: u128, x: f64, y: f64, z: f64) -> PositionedFixture {
        PositionedFixture {
            fixture_id: FixtureId(Uuid::from_u128(number)),
            position_2d: Some(StageGridPosition2d { x, y }),
            position_3d: Some(StageGridPosition { x, y, z }),
        }
    }

    fn missing(number: u128) -> PositionedFixture {
        PositionedFixture {
            fixture_id: FixtureId(Uuid::from_u128(number)),
            position_2d: None,
            position_3d: None,
        }
    }

    fn ids(numbers: &[u128]) -> Vec<FixtureId> {
        numbers
            .iter()
            .map(|number| FixtureId(Uuid::from_u128(*number)))
            .collect()
    }

    fn cells(method: GridMethod, fixtures: &[PositionedFixture]) -> Vec<(u128, usize, usize)> {
        SelectionGrid::from_stage_positions(
            fixtures,
            GridMethodConfiguration {
                method,
                axis_origin: AxisOrigin::default(),
            },
        )
        .unwrap()
        .cells
        .into_iter()
        .map(|cell| (cell.fixture_id.0.as_u128(), cell.row, cell.column))
        .collect()
    }

    #[test]
    fn all_ten_methods_have_the_documented_cycle_and_wrap() {
        let mut method = GridMethod::Stage2d;
        let mut visited = Vec::new();
        for _ in 0..GridMethod::ALL.len() {
            visited.push(method);
            method = method.next();
        }
        assert_eq!(visited, GridMethod::ALL);
        assert_eq!(method, GridMethod::Stage2d);
    }

    #[test]
    fn stage_and_top_views_use_x_left_to_right_and_y_front_to_back() {
        let fixtures = [
            fixture(1, -1.0, 2.0, 0.0),
            fixture(2, 1.0, 2.0, 0.0),
            fixture(3, -1.0, 4.0, 0.0),
        ];
        let expected = vec![(1, 0, 0), (2, 0, 1), (3, 1, 0)];
        assert_eq!(cells(GridMethod::Stage2d, &fixtures), expected);
        assert_eq!(cells(GridMethod::TopToBottom, &fixtures), expected);
    }

    #[test]
    fn stage_2d_uses_its_independent_authored_positions_while_3d_methods_use_xyz() {
        let fixtures = [
            PositionedFixture {
                fixture_id: FixtureId(Uuid::from_u128(1)),
                position_2d: Some(StageGridPosition2d { x: 90.0, y: 90.0 }),
                position_3d: Some(StageGridPosition {
                    x: -4.0,
                    y: -4.0,
                    z: 0.0,
                }),
            },
            PositionedFixture {
                fixture_id: FixtureId(Uuid::from_u128(2)),
                position_2d: Some(StageGridPosition2d { x: 10.0, y: 10.0 }),
                position_3d: Some(StageGridPosition {
                    x: 4.0,
                    y: 4.0,
                    z: 0.0,
                }),
            },
        ];

        assert_eq!(
            cells(GridMethod::Stage2d, &fixtures),
            vec![(2, 0, 0), (1, 1, 1)]
        );
        assert_eq!(
            cells(GridMethod::TopToBottom, &fixtures),
            vec![(1, 0, 0), (2, 1, 1)]
        );
    }

    #[test]
    fn opposite_directional_views_mirror_horizontally() {
        let fixtures = [
            fixture(1, -1.0, -2.0, 1.0),
            fixture(2, 1.0, 2.0, 1.0),
            fixture(3, 0.0, 0.0, 3.0),
        ];
        assert_eq!(
            cells(GridMethod::BottomToTop, &fixtures),
            vec![(1, 0, 2), (3, 1, 1), (2, 2, 0)]
        );
        assert_eq!(
            cells(GridMethod::FrontToBack, &fixtures),
            vec![(3, 0, 1), (1, 1, 0), (2, 1, 2)]
        );
        assert_eq!(
            cells(GridMethod::BackToFront, &fixtures),
            vec![(3, 0, 1), (2, 1, 0), (1, 1, 2)]
        );
        assert_eq!(
            cells(GridMethod::LeftToRight, &fixtures),
            vec![(3, 0, 1), (1, 1, 0), (2, 1, 2)]
        );
        assert_eq!(
            cells(GridMethod::RightToLeft, &fixtures),
            vec![(3, 0, 1), (2, 1, 0), (1, 1, 2)]
        );
    }

    #[test]
    fn horizontal_axis_keeps_x_and_ranks_angle_around_x() {
        let fixtures = [
            fixture(1, -2.0, 1.0, 0.0),
            fixture(2, 2.0, 0.0, 1.0),
            fixture(3, 0.0, -1.0, 0.0),
        ];
        assert_eq!(
            cells(GridMethod::HorizontalAxisX, &fixtures),
            vec![(3, 0, 1), (2, 1, 2), (1, 2, 0)]
        );
    }

    #[test]
    fn vertical_axis_keeps_z_and_ranks_angle_around_z() {
        let fixtures = [
            fixture(1, 1.0, 0.0, -2.0),
            fixture(2, 0.0, 1.0, 2.0),
            fixture(3, -1.0, 0.0, 0.0),
        ];
        assert_eq!(
            cells(GridMethod::VerticalAxisZ, &fixtures),
            vec![(2, 0, 1), (3, 1, 2), (1, 2, 0)]
        );
    }

    #[test]
    fn room_depth_axis_keeps_y_and_ranks_angle_around_y() {
        let fixtures = [
            fixture(1, 1.0, -2.0, 0.0),
            fixture(2, 0.0, 2.0, 1.0),
            fixture(3, -1.0, 0.0, 0.0),
        ];
        assert_eq!(
            cells(GridMethod::RoomDepthAxisY, &fixtures),
            vec![(1, 0, 0), (3, 1, 2), (2, 2, 1)]
        );
    }

    #[test]
    fn editable_axis_origin_changes_angular_ranks_but_not_the_natural_axis() {
        let fixtures = [fixture(1, -2.0, 1.0, 1.0), fixture(2, 2.0, 1.0, 3.0)];
        let grid = SelectionGrid::from_stage_positions(
            &fixtures,
            GridMethodConfiguration {
                method: GridMethod::HorizontalAxisX,
                axis_origin: AxisOrigin {
                    x: 99.0,
                    y: 1.0,
                    z: 2.0,
                },
            },
        )
        .unwrap();
        assert_eq!(
            grid.cells
                .iter()
                .map(|cell| (cell.fixture_id.0.as_u128(), cell.row, cell.column))
                .collect::<Vec<_>>(),
            vec![(2, 0, 1), (1, 1, 0)]
        );
    }

    #[test]
    fn exact_position_ties_use_fixture_identity_and_reserve_sparse_columns() {
        let fixtures = [
            fixture(3, 0.0, 0.0, 0.0),
            fixture(1, 0.0, 0.0, 0.0),
            fixture(4, 0.0, 1.0, 0.0),
            fixture(2, 1.0, 1.0, 0.0),
        ];
        assert_eq!(
            cells(GridMethod::Stage2d, &fixtures),
            vec![(1, 0, 0), (3, 0, 1), (4, 1, 0), (2, 1, 2)]
        );
        let reversed = fixtures.into_iter().rev().collect::<Vec<_>>();
        assert_eq!(
            cells(GridMethod::Stage2d, &reversed),
            vec![(1, 0, 0), (3, 0, 1), (4, 1, 0), (2, 1, 2)]
        );
    }

    #[test]
    fn missing_positions_follow_positioned_cells_in_stable_identity_order() {
        let fixtures = [missing(4), fixture(2, 1.0, 1.0, 1.0), missing(1)];
        assert_eq!(
            cells(GridMethod::Stage2d, &fixtures),
            vec![(2, 0, 0), (1, 1, 0), (4, 1, 1)]
        );
        assert!(
            SelectionGrid::from_stage_positions(&[], GridMethodConfiguration::default())
                .unwrap()
                .cells
                .is_empty()
        );
    }

    #[test]
    fn all_rows_first_traversals_skip_sparse_cells_and_cycle() {
        let grid = SelectionGrid {
            cells: vec![
                GridCell {
                    fixture_id: ids(&[1])[0],
                    row: 0,
                    column: 0,
                },
                GridCell {
                    fixture_id: ids(&[2])[0],
                    row: 0,
                    column: 2,
                },
                GridCell {
                    fixture_id: ids(&[3])[0],
                    row: 2,
                    column: 1,
                },
            ],
        };
        assert_eq!(
            grid.rows_first(RowsFirstTraversal::TopLeft),
            ids(&[1, 2, 3])
        );
        assert_eq!(
            grid.rows_first(RowsFirstTraversal::TopRight),
            ids(&[2, 1, 3])
        );
        assert_eq!(
            grid.rows_first(RowsFirstTraversal::BottomLeft),
            ids(&[3, 1, 2])
        );
        assert_eq!(
            grid.rows_first(RowsFirstTraversal::BottomRight),
            ids(&[3, 2, 1])
        );
        let mut traversal = RowsFirstTraversal::TopLeft;
        for expected in [
            RowsFirstTraversal::TopRight,
            RowsFirstTraversal::BottomLeft,
            RowsFirstTraversal::BottomRight,
            RowsFirstTraversal::TopLeft,
        ] {
            traversal = traversal.next();
            assert_eq!(traversal, expected);
        }
    }

    #[test]
    fn all_columns_first_traversals_skip_sparse_cells_and_cycle() {
        let grid = SelectionGrid {
            cells: vec![
                GridCell {
                    fixture_id: ids(&[1])[0],
                    row: 0,
                    column: 0,
                },
                GridCell {
                    fixture_id: ids(&[2])[0],
                    row: 2,
                    column: 0,
                },
                GridCell {
                    fixture_id: ids(&[3])[0],
                    row: 1,
                    column: 2,
                },
            ],
        };
        assert_eq!(
            grid.columns_first(ColumnsFirstTraversal::TopLeft),
            ids(&[1, 2, 3])
        );
        assert_eq!(
            grid.columns_first(ColumnsFirstTraversal::BottomLeft),
            ids(&[2, 1, 3])
        );
        assert_eq!(
            grid.columns_first(ColumnsFirstTraversal::TopRight),
            ids(&[3, 1, 2])
        );
        assert_eq!(
            grid.columns_first(ColumnsFirstTraversal::BottomRight),
            ids(&[3, 2, 1])
        );
        let mut traversal = ColumnsFirstTraversal::TopLeft;
        for expected in [
            ColumnsFirstTraversal::BottomLeft,
            ColumnsFirstTraversal::TopRight,
            ColumnsFirstTraversal::BottomRight,
            ColumnsFirstTraversal::TopLeft,
        ] {
            traversal = traversal.next();
            assert_eq!(traversal, expected);
        }
    }

    #[test]
    fn non_finite_input_fails_safely_and_non_axis_methods_ignore_unused_origin() {
        let bad_fixture = fixture(7, f64::NAN, 0.0, 0.0);
        assert_eq!(
            SelectionGrid::from_stage_positions(&[bad_fixture], GridMethodConfiguration::default()),
            Err(GridConstructionError::NonFinitePosition {
                fixture_id: bad_fixture.fixture_id
            })
        );
        assert_eq!(
            SelectionGrid::from_stage_positions(
                &[],
                GridMethodConfiguration {
                    method: GridMethod::VerticalAxisZ,
                    axis_origin: AxisOrigin {
                        x: f64::INFINITY,
                        y: 0.0,
                        z: 0.0,
                    },
                }
            ),
            Err(GridConstructionError::NonFiniteAxisOrigin)
        );
        assert!(
            SelectionGrid::from_stage_positions(
                &[],
                GridMethodConfiguration {
                    method: GridMethod::Stage2d,
                    axis_origin: AxisOrigin {
                        x: f64::NAN,
                        y: f64::NAN,
                        z: f64::NAN,
                    },
                }
            )
            .is_ok()
        );
    }
}
