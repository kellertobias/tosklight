//! Pixel mapping: sampling the composited canvas into DMX.
//!
//! A pixel map turns part of what an output is showing into light on a rig. A zone is a rectangle
//! of the canvas divided into a grid; every cell of that grid is one mapped pixel, and every mapped
//! pixel carries a colour layout and a DMX address of its own. Nothing here talks to a GPU or a
//! socket: it decides *where* to sample and *which slots the samples belong in*, and an adapter
//! does the sampling and the sending.

use serde::{Deserialize, Serialize};

/// The colour channels one mapped pixel occupies, in the order they sit on the wire.
///
/// The list is deliberately open: a layout is a sequence of components, so a fixture nobody has
/// thought of yet is a new sequence rather than a new branch in every match.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelComponent {
    Red,
    Green,
    Blue,
    White,
    Amber,
    UltraViolet,
    /// The pixel's own intensity, ahead of its colour. A strip whose fixtures dim per pixel.
    Dimmer,
}

impl PixelComponent {
    /// The value this component takes from one sampled colour.
    ///
    /// White, amber and ultraviolet are derived from the sample rather than measured from it: a
    /// canvas has no such channel to read, so the least of the three primaries is the white content
    /// the sample already carries.
    pub fn value_of(self, sample: Rgb) -> u8 {
        let Rgb { red, green, blue } = sample;
        match self {
            Self::Red => red,
            Self::Green => green,
            Self::Blue => blue,
            Self::White => red.min(green).min(blue),
            Self::Amber => red.min(green),
            Self::UltraViolet => blue.min(red),
            Self::Dimmer => red.max(green).max(blue),
        }
    }
}

/// One sampled colour, straight from the canvas.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Rgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

/// The channel order one kind of mapped fixture uses.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PixelLayout {
    pub name: String,
    pub components: Vec<PixelComponent>,
}

impl PixelLayout {
    pub fn new(name: impl Into<String>, components: Vec<PixelComponent>) -> Self {
        Self {
            name: name.into(),
            components,
        }
    }

    /// How many DMX slots one pixel of this layout occupies.
    pub fn footprint(&self) -> usize {
        self.components.len()
    }

    pub fn rgb() -> Self {
        Self::new(
            "RGB",
            vec![
                PixelComponent::Red,
                PixelComponent::Green,
                PixelComponent::Blue,
            ],
        )
    }

    pub fn rgbw() -> Self {
        Self::new(
            "RGBW",
            vec![
                PixelComponent::Red,
                PixelComponent::Green,
                PixelComponent::Blue,
                PixelComponent::White,
            ],
        )
    }

    pub fn dimmer_rgb() -> Self {
        Self::new(
            "Dimmer RGB",
            vec![
                PixelComponent::Dimmer,
                PixelComponent::Red,
                PixelComponent::Green,
                PixelComponent::Blue,
            ],
        )
    }
}

/// A point on the canvas, as a fraction of its width and height.
///
/// Fractions rather than pixels, so a zone drawn against a 1920×1080 canvas still means the same
/// part of the picture when that output is reconfigured to 3840×2160.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasPoint {
    pub x: f32,
    pub y: f32,
}

impl CanvasPoint {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Where one mapped pixel takes its colour from, and where that colour goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MappedPixel {
    /// Column and row within the zone's grid, counted from the start corner.
    pub column: u32,
    pub row: u32,
    /// The point on the canvas this pixel reads, in canvas fractions.
    pub sample: CanvasPoint,
    /// The universe this pixel's slots live in.
    pub universe: u16,
    /// The one-based address of this pixel's first slot.
    pub address: u16,
}

/// A rectangle of the canvas divided into a grid of mapped pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PixelZone {
    pub id: String,
    pub name: String,
    /// The corner the grid counts from.
    pub start: CanvasPoint,
    pub end: CanvasPoint,
    pub columns: u32,
    pub rows: u32,
    pub layout: PixelLayout,
    pub order: PixelOrder,
    pub universe: u16,
    /// The one-based address of the zone's first pixel.
    pub start_address: u16,
    pub enabled: bool,
}

/// The path addresses run along as they are handed out across the grid.
///
/// A strip folded back and forth up a wall is wired one way; a matrix addressed row by row is wired
/// another. The grid is the same either way, so the difference belongs here rather than in a second
/// set of zones.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelOrder {
    /// Left to right, then down to the next row and left to right again.
    #[default]
    RowMajor,
    /// Top to bottom, then across to the next column and top to bottom again.
    ColumnMajor,
    /// Left to right, then right to left on the row below, as a single strip folds.
    SerpentineRows,
    /// Top to bottom, then bottom to top on the next column.
    SerpentineColumns,
}

impl PixelZone {
    /// How many pixels the grid holds.
    pub fn pixel_count(&self) -> u32 {
        self.columns.saturating_mul(self.rows)
    }

    /// How many DMX slots the whole zone occupies.
    pub fn footprint(&self) -> usize {
        self.pixel_count() as usize * self.layout.footprint()
    }

    /// Every pixel of the grid, in the order their addresses were handed out.
    ///
    /// Sample points sit at the centre of each cell rather than on the zone's edges, so a grid
    /// reads the picture it covers instead of the line around it, and the two ends of the zone are
    /// treated alike.
    pub fn pixels(&self) -> Vec<MappedPixel> {
        let mut pixels = Vec::with_capacity(self.pixel_count() as usize);
        for (index, (column, row)) in self.grid_order().into_iter().enumerate() {
            let slot = index * self.layout.footprint();
            pixels.push(MappedPixel {
                column,
                row,
                sample: self.sample_of(column, row),
                universe: self.universe,
                address: self.start_address.saturating_add(slot as u16),
            });
        }
        pixels
    }

    /// The canvas point at the centre of one cell.
    fn sample_of(&self, column: u32, row: u32) -> CanvasPoint {
        CanvasPoint::new(
            self.start.x + (self.end.x - self.start.x) * centre_fraction(column, self.columns),
            self.start.y + (self.end.y - self.start.y) * centre_fraction(row, self.rows),
        )
    }

    /// The cells in addressing order.
    fn grid_order(&self) -> Vec<(u32, u32)> {
        let mut order = Vec::with_capacity(self.pixel_count() as usize);
        match self.order {
            PixelOrder::RowMajor | PixelOrder::SerpentineRows => {
                for row in 0..self.rows {
                    let reversed = self.order == PixelOrder::SerpentineRows && row % 2 == 1;
                    for step in 0..self.columns {
                        let column = if reversed {
                            self.columns - 1 - step
                        } else {
                            step
                        };
                        order.push((column, row));
                    }
                }
            }
            PixelOrder::ColumnMajor | PixelOrder::SerpentineColumns => {
                for column in 0..self.columns {
                    let reversed = self.order == PixelOrder::SerpentineColumns && column % 2 == 1;
                    for step in 0..self.rows {
                        let row = if reversed { self.rows - 1 - step } else { step };
                        order.push((column, row));
                    }
                }
            }
        }
        order
    }
}

/// Where the centre of one cell sits along an axis, as a fraction of the zone.
///
/// A single cell covers the whole zone, so it reads the middle of it.
fn centre_fraction(index: u32, count: u32) -> f32 {
    if count <= 1 {
        return 0.5;
    }
    (index as f32 + 0.5) / count as f32
}

/// The canvas a zone reads, as tightly packed RGBA8 rows.
#[derive(Clone, Copy, Debug)]
pub struct CanvasImage<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
}

impl CanvasImage<'_> {
    /// The colour at a canvas fraction, clamped to the picture.
    ///
    /// Nearest sample rather than an interpolated one: a mapped pixel stands for a real lantern
    /// pointed at part of the picture, and blending its neighbours in would report a colour that is
    /// on the canvas nowhere.
    pub fn sample(&self, point: CanvasPoint) -> Rgb {
        if self.width == 0 || self.height == 0 {
            return Rgb::default();
        }
        let x = fraction_to_index(point.x, self.width);
        let y = fraction_to_index(point.y, self.height);
        let offset = ((y as usize * self.width as usize) + x as usize) * 4;
        match self.rgba.get(offset..offset + 3) {
            Some([red, green, blue]) => Rgb {
                red: *red,
                green: *green,
                blue: *blue,
            },
            _ => Rgb::default(),
        }
    }
}

fn fraction_to_index(fraction: f32, extent: u32) -> u32 {
    if !fraction.is_finite() {
        return 0;
    }
    let scaled = (fraction * extent as f32).floor();
    scaled.clamp(0.0, (extent - 1) as f32) as u32
}

/// One universe's worth of slots, as the wire carries it.
pub const DMX_SLOTS: usize = 512;

/// The slots a zone writes, gathered by universe.
///
/// A frame is always the full 512 slots. A universe that a zone only partly fills still leaves
/// every other slot at zero rather than shortening the frame, because a receiver reads a whole
/// universe and a short one tells it nothing about the rest.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UniverseFrames {
    frames: std::collections::BTreeMap<u16, [u8; DMX_SLOTS]>,
}

impl UniverseFrames {
    pub fn get(&self, universe: u16) -> Option<&[u8; DMX_SLOTS]> {
        self.frames.get(&universe)
    }

    pub fn universes(&self) -> impl Iterator<Item = u16> + '_ {
        self.frames.keys().copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (u16, &[u8; DMX_SLOTS])> {
        self.frames
            .iter()
            .map(|(universe, frame)| (*universe, frame))
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Writes one slot, creating the universe's frame the first time it is touched.
    ///
    /// A one-based address outside the universe is dropped rather than wrapped into the next one:
    /// a pixel that does not fit is a configuration to fix, not slots to scatter.
    pub fn write(&mut self, universe: u16, address: u16, value: u8) {
        let Some(index) = (address as usize).checked_sub(1) else {
            return;
        };
        if index >= DMX_SLOTS {
            return;
        }
        self.frames.entry(universe).or_insert([0; DMX_SLOTS])[index] = value;
    }

    /// Starts every universe a zone addresses, so a zone that samples pure black still sends.
    pub fn open(&mut self, universe: u16) {
        self.frames.entry(universe).or_insert([0; DMX_SLOTS]);
    }
}

/// The two control values carried by a desk-merge zone fixture.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ZoneMergeControls {
    /// Brightness applied only to the Media Server contribution.
    pub dimmer: u8,
    /// 0 is Media Server, 254 is desk, and 255 is HTP.
    pub mix: u8,
}

/// Combines one Media Server pixel slot with its lighting-desk counterpart.
///
/// Integer arithmetic keeps the result identical on every platform. Intermediate values use the
/// complete 0..=254 crossfade range; 255 is deliberately outside that range and selects HTP.
pub fn merge_slot(media: u8, desk: u8, controls: ZoneMergeControls) -> u8 {
    let media = (u32::from(media) * u32::from(controls.dimmer) + 127) / 255;
    if controls.mix == 255 {
        return media.max(u32::from(desk)) as u8;
    }
    let desk_weight = u32::from(controls.mix);
    let media_weight = 254 - desk_weight;
    ((media * media_weight + u32::from(desk) * desk_weight + 127) / 254) as u8
}

/// Samples the canvas through every enabled zone and lays the result out in universes.
pub fn map_pixels(zones: &[PixelZone], canvas: CanvasImage<'_>) -> UniverseFrames {
    let mut frames = UniverseFrames::default();
    for zone in zones.iter().filter(|zone| zone.enabled) {
        frames.open(zone.universe);
        for pixel in zone.pixels() {
            let sample = canvas.sample(pixel.sample);
            for (offset, component) in zone.layout.components.iter().enumerate() {
                frames.write(
                    pixel.universe,
                    pixel.address.saturating_add(offset as u16),
                    component.value_of(sample),
                );
            }
        }
    }
    frames
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone(columns: u32, rows: u32) -> PixelZone {
        PixelZone {
            id: "zone".into(),
            name: "Upstage strip".into(),
            start: CanvasPoint::new(0.0, 0.0),
            end: CanvasPoint::new(1.0, 1.0),
            columns,
            rows,
            layout: PixelLayout::rgb(),
            order: PixelOrder::RowMajor,
            universe: 1,
            start_address: 1,
            enabled: true,
        }
    }

    /// A canvas whose left half is red and right half is green.
    fn split_canvas() -> Vec<u8> {
        let mut rgba = vec![0; 4 * 4 * 4];
        for y in 0..4 {
            for x in 0..4 {
                let offset = (y * 4 + x) * 4;
                if x < 2 {
                    rgba[offset] = 255;
                } else {
                    rgba[offset + 1] = 255;
                }
                rgba[offset + 3] = 255;
            }
        }
        rgba
    }

    #[test]
    fn a_grid_samples_the_centre_of_each_cell() {
        let pixels = zone(2, 2).pixels();
        let samples: Vec<(f32, f32)> = pixels
            .iter()
            .map(|pixel| (pixel.sample.x, pixel.sample.y))
            .collect();
        assert_eq!(
            samples,
            vec![(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)]
        );
    }

    #[test]
    fn a_single_cell_reads_the_middle_of_the_zone() {
        let pixels = zone(1, 1).pixels();
        assert_eq!(pixels[0].sample, CanvasPoint::new(0.5, 0.5));
    }

    #[test]
    fn addresses_run_one_pixel_footprint_apart() {
        let pixels = zone(3, 1).pixels();
        assert_eq!(
            pixels.iter().map(|p| p.address).collect::<Vec<_>>(),
            vec![1, 4, 7]
        );
        assert_eq!(zone(3, 1).footprint(), 9);
    }

    #[test]
    fn a_serpentine_strip_turns_back_on_every_other_row() {
        let mut folded = zone(3, 2);
        folded.order = PixelOrder::SerpentineRows;
        let columns: Vec<u32> = folded.pixels().iter().map(|pixel| pixel.column).collect();
        // Left to right along the top, then right to left along the row below it.
        assert_eq!(columns, vec![0, 1, 2, 2, 1, 0]);
    }

    #[test]
    fn a_column_major_grid_runs_down_before_across() {
        let mut down = zone(2, 3);
        down.order = PixelOrder::ColumnMajor;
        let cells: Vec<(u32, u32)> = down
            .pixels()
            .iter()
            .map(|pixel| (pixel.column, pixel.row))
            .collect();
        assert_eq!(cells, vec![(0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (1, 2)]);
    }

    #[test]
    fn a_zone_reads_the_colour_under_each_cell() {
        let rgba = split_canvas();
        let canvas = CanvasImage {
            width: 4,
            height: 4,
            rgba: &rgba,
        };
        let frames = map_pixels(&[zone(2, 1)], canvas);
        let frame = frames.get(1).expect("the zone's universe");
        // The left pixel is red, the right one green, three slots apart.
        assert_eq!(&frame[0..3], &[255, 0, 0]);
        assert_eq!(&frame[3..6], &[0, 255, 0]);
    }

    #[test]
    fn a_universe_is_always_a_whole_frame() {
        let rgba = split_canvas();
        let frames = map_pixels(
            &[zone(1, 1)],
            CanvasImage {
                width: 4,
                height: 4,
                rgba: &rgba,
            },
        );
        assert_eq!(frames.get(1).expect("the universe").len(), DMX_SLOTS);
        // One pixel fills three slots; every other slot is still carried, at zero.
        assert!(
            frames.get(1).expect("the universe")[3..]
                .iter()
                .all(|slot| *slot == 0)
        );
    }

    #[test]
    fn a_zone_that_samples_black_still_sends_its_universe() {
        let rgba = vec![0; 4 * 4 * 4];
        let frames = map_pixels(
            &[zone(2, 2)],
            CanvasImage {
                width: 4,
                height: 4,
                rgba: &rgba,
            },
        );
        assert_eq!(frames.universes().collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn a_disabled_zone_sends_nothing() {
        let mut off = zone(2, 2);
        off.enabled = false;
        let rgba = split_canvas();
        let frames = map_pixels(
            &[off],
            CanvasImage {
                width: 4,
                height: 4,
                rgba: &rgba,
            },
        );
        assert!(frames.is_empty());
    }

    #[test]
    fn zone_merge_endpoints_and_htp_are_exact() {
        let full = ZoneMergeControls {
            dimmer: 255,
            mix: 0,
        };
        assert_eq!(merge_slot(200, 40, full), 200);
        assert_eq!(
            merge_slot(200, 40, ZoneMergeControls { mix: 254, ..full }),
            40
        );
        assert_eq!(
            merge_slot(100, 180, ZoneMergeControls { mix: 255, ..full }),
            180
        );
    }

    #[test]
    fn zone_merge_dims_only_the_media_contribution() {
        let half_media = ZoneMergeControls {
            dimmer: 128,
            mix: 0,
        };
        assert_eq!(merge_slot(200, 255, half_media), 100);
        assert_eq!(
            merge_slot(
                200,
                180,
                ZoneMergeControls {
                    mix: 255,
                    ..half_media
                }
            ),
            180,
            "desk HTP remains undimmed"
        );
    }

    #[test]
    fn zone_merge_crossfades_over_zero_through_254() {
        let controls = ZoneMergeControls {
            dimmer: 255,
            mix: 127,
        };
        assert_eq!(merge_slot(200, 40, controls), 120);
    }

    #[test]
    fn white_amber_and_ultraviolet_come_out_of_the_sample() {
        let warm = Rgb {
            red: 255,
            green: 200,
            blue: 40,
        };
        assert_eq!(PixelComponent::White.value_of(warm), 40);
        assert_eq!(PixelComponent::Amber.value_of(warm), 200);
        assert_eq!(PixelComponent::UltraViolet.value_of(warm), 40);
        assert_eq!(PixelComponent::Dimmer.value_of(warm), 255);
    }

    #[test]
    fn an_rgbw_pixel_takes_four_slots_and_carries_its_white() {
        let mut warm = zone(1, 1);
        warm.layout = PixelLayout::rgbw();
        assert_eq!(warm.footprint(), 4);
        let rgba = vec![255, 255, 255, 255];
        let frames = map_pixels(
            &[warm],
            CanvasImage {
                width: 1,
                height: 1,
                rgba: &rgba,
            },
        );
        assert_eq!(
            &frames.get(1).expect("the universe")[0..4],
            &[255, 255, 255, 255]
        );
    }

    #[test]
    fn a_pixel_past_the_end_of_a_universe_is_dropped_rather_than_wrapped() {
        let mut late = zone(2, 1);
        late.start_address = 511;
        let rgba = split_canvas();
        let frames = map_pixels(
            &[late],
            CanvasImage {
                width: 4,
                height: 4,
                rgba: &rgba,
            },
        );
        let frame = frames.get(1).expect("the universe");
        // Slots 511 and 512 take the first pixel's red and green; nothing wraps to slot 1.
        assert_eq!(frame[510], 255);
        assert_eq!(frame[511], 0);
        assert_eq!(frame[0], 0);
    }

    #[test]
    fn sampling_outside_the_canvas_is_clamped_to_it() {
        let rgba = split_canvas();
        let canvas = CanvasImage {
            width: 4,
            height: 4,
            rgba: &rgba,
        };
        assert_eq!(
            canvas.sample(CanvasPoint::new(-1.0, -1.0)),
            Rgb {
                red: 255,
                green: 0,
                blue: 0
            }
        );
        assert_eq!(
            canvas.sample(CanvasPoint::new(9.0, 9.0)),
            Rgb {
                red: 0,
                green: 255,
                blue: 0
            }
        );
    }
}
