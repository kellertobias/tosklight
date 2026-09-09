use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DisplayDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DisplayRectangle {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) width: u32,
    pub(super) height: u32,
}

pub(super) fn monitor_rectangle(monitor: &winit::monitor::MonitorHandle) -> DisplayRectangle {
    let position = monitor.position();
    let size = monitor.size();
    DisplayRectangle {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    }
}

pub(super) fn nearest_display(
    current: DisplayRectangle,
    displays: &[DisplayRectangle],
    direction: DisplayDirection,
) -> Option<usize> {
    let centre = |display: DisplayRectangle| {
        (
            i64::from(display.x) * 2 + i64::from(display.width),
            i64::from(display.y) * 2 + i64::from(display.height),
        )
    };
    let (current_x, current_y) = centre(current);
    let current_right = i64::from(current.x) + i64::from(current.width);
    let current_bottom = i64::from(current.y) + i64::from(current.height);
    displays
        .iter()
        .enumerate()
        .filter_map(|(index, display)| {
            if *display == current {
                return None;
            }
            let (x, y) = centre(*display);
            let (primary, cross) = match direction {
                DisplayDirection::Left
                    if i64::from(display.x) + i64::from(display.width) <= i64::from(current.x) =>
                {
                    (current_x - x, (y - current_y).abs())
                }
                DisplayDirection::Right if i64::from(display.x) >= current_right => {
                    (x - current_x, (y - current_y).abs())
                }
                DisplayDirection::Up
                    if i64::from(display.y) + i64::from(display.height) <= i64::from(current.y) =>
                {
                    (current_y - y, (x - current_x).abs())
                }
                DisplayDirection::Down if i64::from(display.y) >= current_bottom => {
                    (y - current_y, (x - current_x).abs())
                }
                _ => return None,
            };
            Some((primary + cross * 2, primary, index))
        })
        .min()
        .map(|(_, _, index)| index)
}

/// Samples one output's frame into its pixel map and sends it.
///
/// Output rather than a preview: it runs on its own cadence and does not wait for anyone to be
/// watching a thumbnail. The cadence is asked before the readback, because the readback is the
/// expensive half and a frame that will not be sent should not pay for one.
#[allow(clippy::too_many_arguments)]
pub(super) fn map_pixels(
    pixels: &mut crate::pixel_output::PixelOutputs,
    configuration: &OutputConfiguration,
    output: &mut WindowedOutput,
    master: &media_domain::MasterState,
    master_mask: Option<&SourceTexture>,
    now: Timestamp,
    instance: [u8; 16],
    universe_inputs: &crate::dmx::SharedUniverseInputs,
) {
    if !pixels.wants(configuration, now.as_millis()) {
        return;
    }
    let size = output.size();
    let frame = output.capture_preview(size, master, master_mask);
    pixels.send(
        configuration,
        media_domain::pixel_map::CanvasImage {
            width: size.width,
            height: size.height,
            rgba: &frame,
        },
        now.as_millis(),
        instance,
        universe_inputs,
    );
}
