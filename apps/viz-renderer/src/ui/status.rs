//! The status surface: the two lines along the bottom of the window, and Stage fixture labels.
//!
//! It says what the visualizer is connected to, what it is drawing, and what is wrong when
//! something is — which is the whole of an operator's feedback when the picture looks right but
//! the numbers behind it do not.

use super::*;
use viz_render::{Overlay, ResolvedCamera};
use viz_scene::{
    ConnectionState, ProviderDiagnostics, RenderQuality, Scene, SceneValues, Theme, UniverseGrade,
    UniverseHealth, ViewMode,
};

/// Everything the status surface and Quick Settings need from the running application.
pub struct StatusModel<'a> {
    pub connection: &'a ConnectionState,
    pub diagnostics: &'a ProviderDiagnostics,
    pub universes: &'a [UniverseHealth],
    pub view_mode: ViewMode,
    pub quality: RenderQuality,
    pub quality_is_local: bool,
    pub theme: Theme,
    pub fixtures: usize,
    pub emitters: usize,
    pub lights: u32,
    pub frames_per_second: f32,
    pub latency_p50_millis: f32,
    pub latency_p95_millis: f32,
    pub latency_max_millis: f32,
    pub dmx_hz: f32,
    pub fog_percent: f32,
    pub ambient_percent: f32,
    pub degraded: bool,
    pub exposure: f32,
    pub renderer: String,
    /// What the GPU spent on a recent frame, where the adapter can time one.
    pub gpu_millis: Option<f32>,
    pub waiting_for_dmx: bool,
    /// Who owns the dedicated external 3D presentation camera. This is always `None` for an
    /// embedded Stage pane and for orthographic views.
    pub camera_control: DmxCameraControlStatus,
    /// The fixture the operator clicked on: its number, name, address and level. Nothing selected
    /// is the ordinary case, and the bar says nothing at all then.
    pub selection: Option<String>,
    /// A short-lived word about something the operator just did — a snapshot taken, a Blender file
    /// written — and whether it is bad news.
    pub notice: Option<(String, bool)>,
}

/// Operator-facing ownership of the one DMX-controllable external camera.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DmxCameraControlStatus {
    #[default]
    None,
    Dmx,
    /// The last DMX pose is held because the fixture is absent or input is stale.
    Held,
    Local {
        can_release: bool,
    },
}

impl StatusModel<'_> {
    /// What the operator is looking at: the surface and its defining setting.
    ///
    /// A quality this renderer was told to hold locally says so, because it is then no longer
    /// whatever the source asked for and an operator has to be able to see that from the bar.
    pub fn surface_summary(&self) -> String {
        if self.view_mode.is_plot() {
            format!("2D {}", self.view_mode.label())
        } else if self.quality_is_local {
            format!(
                "{} {} (local)",
                self.view_mode.label(),
                self.quality.label()
            )
        } else {
            format!("{} {}", self.view_mode.label(), self.quality.label())
        }
    }

    /// The renderer's own line for the connection surface: which GPU is drawing, and how fast
    /// values are arriving to draw with.
    pub fn renderer_summary(&self) -> String {
        let inputs = if self.universes.is_empty() {
            "no inputs".to_owned()
        } else {
            format!(
                "{} inputs {} at {:.0} Hz",
                self.universes.len(),
                self.worst_grade().label(),
                self.dmx_hz
            )
        };
        match self.gpu_millis {
            Some(millis) => format!("{}  |  GPU {millis:5.2} ms  |  {inputs}", self.renderer),
            None => format!("{}  |  {inputs}", self.renderer),
        }
    }

    /// The worst grade across every universe, which is what the operator needs to notice.
    pub fn worst_grade(&self) -> UniverseGrade {
        self.universes
            .iter()
            .map(|universe| universe.grade)
            .max()
            .unwrap_or(UniverseGrade::Waiting)
    }
}

/// What separates one shortcut from the next in the status bar.
pub(super) const HINT_SEPARATOR: &str = "  \u{2022}  ";

/// What the second row says beside the connection.
///
/// Only one of these is ever true at a time, and they are ordered by what the operator is most
/// likely to be waiting to read: the thing they just did, then the reason the picture is empty.
pub(super) enum SecondRowNote<'a> {
    Notice(&'a str, bool),
    EmptyRig,
    WaitingForDmx,
    /// Nothing is arriving, but the planning window is driving the rig, so the picture is lit on
    /// purpose. Saying "Waiting for DMX" over a rig that is visibly lit reads as a fault.
    EditorDriving,
    LocalCameraRelease,
    LocalCameraWaiting,
    DmxCameraHeld,
}

impl SecondRowNote<'_> {
    pub(super) fn text(&self) -> &str {
        match self {
            Self::Notice(text, _) => text,
            Self::EmptyRig => EMPTY_RIG,
            Self::WaitingForDmx => "Waiting for DMX",
            Self::EditorDriving => EDITOR_DRIVING,
            Self::LocalCameraRelease => "Local camera control \u{2014} press C to return to DMX",
            Self::LocalCameraWaiting => "Local camera control \u{2014} waiting for the DMX camera",
            Self::DmxCameraHeld => "DMX camera unavailable \u{2014} holding its last pose",
        }
    }
}

/// A confirmation is about something that just happened and is gone in seconds; the connection is
/// permanent. So it takes the space beside the connection and gives it straight back.
pub(super) fn second_row_note<'a>(model: &'a StatusModel<'a>) -> Option<SecondRowNote<'a>> {
    if let Some((notice, failure)) = model.notice.as_ref() {
        return Some(SecondRowNote::Notice(notice, *failure));
    }
    match model.camera_control {
        DmxCameraControlStatus::Local { can_release: true } => {
            return Some(SecondRowNote::LocalCameraRelease);
        }
        DmxCameraControlStatus::Local { can_release: false } => {
            return Some(SecondRowNote::LocalCameraWaiting);
        }
        DmxCameraControlStatus::Held => return Some(SecondRowNote::DmxCameraHeld),
        DmxCameraControlStatus::None | DmxCameraControlStatus::Dmx => {}
    }
    // Connected to something with no rig in it. Without this the window is simply empty, which
    // reads as a broken visualizer rather than as a document nobody has patched yet.
    if model.connection.is_connected() && model.fixtures == 0 {
        return Some(SecondRowNote::EmptyRig);
    }
    // Nothing has arrived from the network. Whether that is a problem depends entirely on whether
    // anything else is driving the rig, so the two cases are named separately.
    if !model.diagnostics.preview_universes.is_empty() {
        return Some(SecondRowNote::EditorDriving);
    }
    model
        .waiting_for_dmx
        .then_some(SecondRowNote::WaitingForDmx)
}

/// Shown when the source answered but has no fixtures: a document nobody has patched yet, which
/// is otherwise an empty window with nothing to say for itself.
pub(super) const EMPTY_RIG: &str =
    "No fixtures in this show \u{2014} patch a rig in the Viz editor";

/// Shown when no DMX is arriving but the planning window's preview values are lighting the rig.
pub(super) const EDITOR_DRIVING: &str =
    "No DMX in \u{2014} the Viz editor is driving these fixtures";

/// The shortcuts worth keeping in front of the operator, most important first.
///
/// This is not a keyboard reference — the manual and Quick Settings are that. It is the handful an
/// operator reaches for without thinking, and **the snapshot comes first**: it is the one that
/// keeps a look which is about to be gone, and the one nobody would otherwise know is there.
///
/// The modifier is named the way the platform names it, because `Cmd` on a Windows desk and `Ctrl`
/// on a Mac are both simply wrong.
pub fn shortcut_hints() -> [&'static str; 4] {
    [
        if cfg!(target_os = "macos") {
            "Cmd+S snapshot"
        } else {
            "Ctrl+S snapshot"
        },
        "Enter settings",
        "Space overlays",
        "1-8 views",
    ]
}

/// As many shortcuts as fit in `available` pixels, joined, dropping the least important first.
///
/// Nothing at all comes back when even the first will not fit: half a word of a shortcut is worse
/// than no shortcut, and the bar has more urgent things to say when it is that tight.
pub(super) fn fitting_hints(available: f32, scale: f32) -> String {
    let hints = shortcut_hints();
    (1..=hints.len())
        .rev()
        .map(|count| hints[..count].join(HINT_SEPARATOR))
        .find(|text| Overlay::measure(text, scale) <= available)
        .unwrap_or_default()
}

/// Build the persistent status surface, returning the regions the operator can act on.
///
/// Never a bare spinner: every state names its boundary, and every number that can be changed
/// says so by being hoverable.
pub fn build_status(
    overlay: &mut Overlay,
    model: &StatusModel<'_>,
    width: f32,
    height: f32,
) -> Vec<HotspotRect> {
    // The footer is always dark chrome, whichever way round the page is: it is the application
    // talking, not part of the drawing.
    let palette = Palette::of(Theme::LightOnDark);
    // The status bar is a footer, not a panel: it reads at a glance and gives the stage the
    // screen. It is deliberately smaller than the rest of the operator surface.
    let scale = (ui_scale(width) * 0.62).max(1.0);
    let line = Overlay::line_height(scale);
    let padding = 7.0 * scale;
    let mut hotspots = Vec::new();

    let bar_height = line * 2.0 + padding * 1.6;
    overlay.rect(
        0.0,
        height - bar_height,
        width,
        bar_height,
        [0.0, 0.0, 0.0, 1.0],
    );
    overlay.rect(
        0.0,
        height - bar_height,
        width,
        (scale * 0.8).max(1.0),
        srgb([0.28, 0.29, 0.32, 1.0]),
    );
    let top = height - bar_height + padding * 0.8;

    // The application mark opens Quick Settings and sits at the head of the bar.
    let mark_size = bar_height - padding * 1.2;
    let mark = draw_application_mark(
        overlay,
        &palette,
        padding,
        height - bar_height + padding * 0.6,
        mark_size,
    );
    hotspots.push(HotspotRect {
        hotspot: Hotspot::OpenSettings,
        rect: mark,
    });
    let head = mark[0] + mark[2] + padding;

    // The right-hand groups are measured before anything is drawn so the left-hand groups know
    // exactly where they have to stop. Two halves sharing one bar must never overwrite each other.
    let gap = 14.0 * scale;
    let surface = model.surface_summary();
    let counts = format!("{} fixtures  {} heads", model.fixtures, model.emitters);
    let beams = format!("{} live beams", model.lights);
    let ambient = format!("ambient {:.0}%", model.ambient_percent);
    let exposure = format!("exposure {:.2}", model.exposure);
    let fog = format!("fog {:.0}%", model.fog_percent);
    let frame_rate = format!("{:.0} FPS", model.frames_per_second);
    let width_of = |texts: &[&str]| -> f32 {
        texts
            .iter()
            .map(|text| Overlay::measure(text, scale) + gap)
            .sum()
    };
    let top_limit = width - padding - width_of(&[&surface, &beams, &counts]);
    let mut bottom_row: Vec<&str> = vec![&ambient, &exposure, &fog, &frame_rate];
    if model.degraded {
        bottom_row.push("quality reduced");
    }
    let bottom_limit = width - padding - width_of(&bottom_row);

    build_universe_badges(
        overlay, model, palette, head, top, scale, line, top_limit, gap,
    );

    build_second_row(
        overlay,
        model,
        palette,
        head,
        top,
        line,
        scale,
        bottom_limit,
        gap,
    );

    let mut top_right = width - padding;
    let mut bottom_right = width - padding;
    let place =
        |overlay: &mut Overlay, cursor: &mut f32, text: &str, colour: [f32; 4], row: f32| {
            let text_width = Overlay::measure(text, scale);
            *cursor -= text_width;
            let x = *cursor;
            overlay.text(x, top + row, scale, colour, text);
            *cursor -= gap;
            [x, top + row - 3.0 * scale, text_width, line]
        };

    place(overlay, &mut top_right, &surface, palette.accent, 0.0);
    place(overlay, &mut top_right, &beams, palette.text, 0.0);
    place(overlay, &mut top_right, &counts, palette.text, 0.0);

    for (hotspot, text) in [
        (Hotspot::Ambient, &ambient),
        (Hotspot::Exposure, &exposure),
        (Hotspot::Fog, &fog),
    ] {
        let rect = place(overlay, &mut bottom_right, text, palette.text, line);
        hotspots.push(HotspotRect { hotspot, rect });
    }
    place(overlay, &mut bottom_right, &frame_rate, palette.dim, line);
    if model.degraded {
        place(
            overlay,
            &mut bottom_right,
            "quality reduced",
            palette.warn,
            line,
        );
    }

    hotspots
}

/// The first line's left half: one badge per universe, then the render latency beside them.
///
/// Positional geometry, like the overlay primitives it draws with: a place to start, a place to
/// stop, and the scale everything is measured in.
///
/// The latency is part of what the operator asked to see there, so its room is reserved before
/// the badges claim theirs, and the badges give up their units before they give up their place.
#[allow(clippy::too_many_arguments)]
fn build_universe_badges(
    overlay: &mut Overlay,
    model: &StatusModel<'_>,
    palette: Palette,
    head: f32,
    top: f32,
    scale: f32,
    line: f32,
    top_limit: f32,
    gap: f32,
) {
    // Bottom left: one badge per universe, then the render latency beside them. The latency is
    // part of what the operator asked to see there, so its room is reserved before the badges
    // claim theirs, and the badges give up their units before they give up their place.
    let latency = format!(
        "latency {:.0} / {:.0} / {:.0} ms",
        model.latency_p50_millis, model.latency_p95_millis, model.latency_max_millis
    );
    let short_latency = format!("{:.0} ms", model.latency_p95_millis);
    let badge_label = |universe: &UniverseHealth, compact: bool| {
        if compact {
            format!("U{}", universe.universe)
        } else {
            format!("U{} {:.0}Hz", universe.universe, universe.rate_hz)
        }
    };
    let badges_width = |compact: bool, badge_scale: f32| -> f32 {
        model
            .universes
            .iter()
            .map(|universe| {
                Overlay::measure(&badge_label(universe, compact), badge_scale) + 16.0 * badge_scale
            })
            .sum()
    };
    // Each step gives up the least the operator would miss: the long latency first, then badge
    // size, and only last the rates themselves.
    let fits = |compact: bool, badge_scale: f32, latency_text: &str| {
        head + badges_width(compact, badge_scale) + Overlay::measure(latency_text, scale)
            <= top_limit
    };
    let small = scale * 0.8;
    let (compact, badge_scale, latency_text) = if fits(false, scale, &latency) {
        (false, scale, latency.as_str())
    } else if fits(false, scale, &short_latency) {
        (false, scale, short_latency.as_str())
    } else if fits(false, small, &short_latency) {
        (false, small, short_latency.as_str())
    } else {
        (true, small, short_latency.as_str())
    };

    let mut x = head;
    if model.universes.is_empty() {
        x += overlay.text(x, top, scale, palette.waiting, "no universes") + gap;
    }
    for universe in model.universes {
        let label = badge_label(universe, compact);
        let badge_width = Overlay::measure(&label, badge_scale) + 10.0 * badge_scale;
        if x + badge_width > top_limit {
            // Never run a badge under the right-hand group: say how many are not shown instead.
            let remaining = model.universes.len()
                - model
                    .universes
                    .iter()
                    .position(|candidate| candidate.universe == universe.universe)
                    .unwrap_or(0);
            overlay.text(x, top, scale, palette.dim, &format!("+{remaining}"));
            x += Overlay::measure(&format!("+{remaining}"), scale) + gap;
            break;
        }
        let colour = palette.grade(universe.grade);
        // The badge itself carries the colour; the text stays legible on top of it. Its outline
        // is a step brighter so the shape reads as a badge rather than a block of colour.
        overlay.badge(
            x,
            top - 2.0 * scale,
            badge_width,
            line,
            line * 0.45,
            brighten(colour, 1.35),
            colour,
        );
        overlay.text(
            x + 5.0 * badge_scale,
            top + (line - Overlay::line_height(badge_scale)) * 0.5,
            badge_scale,
            contrasting_ink(colour),
            &label,
        );
        x += badge_width + 6.0 * badge_scale;
    }
    if x + Overlay::measure(latency_text, scale) <= top_limit {
        overlay.text(x + 6.0 * scale, top, scale, palette.dim, latency_text);
    }
}

/// The second line of the footer: what the visualizer is connected to, what the operator last
/// asked of the picture, and anything the renderer had to give up to keep drawing it.
///
/// Positional geometry, for the same reason [`build_universe_badges`] is.
#[allow(clippy::too_many_arguments)]
fn build_second_row(
    overlay: &mut Overlay,
    model: &StatusModel<'_>,
    palette: Palette,
    head: f32,
    top: f32,
    line: f32,
    scale: f32,
    bottom_limit: f32,
    gap: f32,
) {
    // Second row on the left: the connection, which must always name its boundary.
    let (state_colour, state_text) = match model.connection {
        ConnectionState::Connected { .. } => (palette.good, model.connection.summary()),
        ConnectionState::Failed { .. } => (palette.bad, model.connection.summary()),
        ConnectionState::Stale { .. } => (palette.warn, model.connection.summary()),
        _ => (palette.warn, model.connection.summary()),
    };
    let mut second = head;
    second += overlay.clipped_text(
        second,
        top + line,
        scale,
        state_colour,
        &state_text,
        bottom_limit,
    ) + 12.0 * scale;
    // What the operator clicked on sits beside the connection, where they are already looking
    // after clicking. It is what they asked of the picture, so it comes before a passing notice.
    if let Some(selection) = model.selection.as_ref() {
        second += overlay.clipped_text(
            second,
            top + line,
            scale,
            palette.accent,
            selection,
            bottom_limit,
        ) + 12.0 * scale;
    }
    // A confirmation is about something that just happened and is gone in seconds; the connection
    // is permanent. So it takes the space beside the connection and gives it straight back.
    match second_row_note(model) {
        Some(SecondRowNote::Notice(notice, failure)) => {
            let ink = if failure { palette.bad } else { palette.accent };
            second +=
                overlay.clipped_text(second, top + line, scale, ink, notice, bottom_limit) + gap;
        }
        Some(note) => {
            let text = note.text();
            if second + Overlay::measure(text, scale) <= bottom_limit {
                second += overlay.text(second, top + line, scale, palette.warn, text) + gap;
            }
        }
        None => {}
    }

    // The shortcuts live in whatever is left of the middle, against the values on the right. They
    // are the last thing in the bar to be given room and the first to give it back, an item at a
    // time from the least useful end — a shortcut nobody can see is a feature nobody has, and a
    // bar that has run out of room is not the place to argue about it.
    let hints = fitting_hints(bottom_limit - second - gap, scale);
    if !hints.is_empty() {
        let hint_width = Overlay::measure(&hints, scale);
        overlay.text(
            bottom_limit - gap - hint_width,
            top + line,
            scale,
            palette.dim,
            &hints,
        );
    }

    // Bottom right: what is on screen, and the values the wheel can change. Each row keeps its
    // own cursor so the two never collide.
}

/// Draw screen-space fixture labels in every Stage view.
///
/// Labels are operator annotations for light-producing fixtures. Collision dropping, scene
/// visibility, and a small coverage budget keep them from turning a dense rig into a wall of text.
/// Plan views additionally keep their existing live-colour dots.
pub fn build_fixture_labels(
    overlay: &mut Overlay,
    scene: &Scene,
    values: &SceneValues,
    camera: &ResolvedCamera,
    view: &viz_scene::ViewConfiguration,
    width: f32,
    height: f32,
) {
    let (theme, show_labels) = (view.theme, view.show_labels);
    let scale = (ui_scale(width) * 0.62).max(1.0);
    let line = Overlay::line_height(scale);
    let label_ink = srgb([
        theme.label_ink()[0],
        theme.label_ink()[1],
        theme.label_ink()[2],
        1.0,
    ]);

    if !show_labels && !view.mode.is_plot() {
        return;
    }

    // The colour a fixture is emitting, taken from its brightest head.
    let lit = fixture_lighting(scene, values);

    if view.mode.is_plot() {
        build_plot_fixture_labels(
            overlay,
            scene,
            camera,
            width,
            height,
            scale,
            line,
            label_ink,
            show_labels,
            &lit,
        );
        return;
    }

    build_perspective_fixture_labels(
        overlay, scene, camera, width, height, scale, line, label_ink, theme, &lit,
    );
}

#[allow(clippy::too_many_arguments)]
fn build_perspective_fixture_labels(
    overlay: &mut Overlay,
    scene: &Scene,
    camera: &ResolvedCamera,
    width: f32,
    height: f32,
    scale: f32,
    line: f32,
    label_ink: [f32; 4],
    theme: Theme,
    lit: &[Option<(f32, [f32; 3])>],
) {
    // A 3D picture with overlapping labels is unreadable, so a label is dropped when it would collide
    // with one already placed. Near fixtures win deterministically, which prevents a label behind
    // the rig from hiding the fixture in front without pretending the overlay is depth-tested.
    // Zooming in makes room and the rest appear.
    let mut placed: Vec<[f32; 4]> = Vec::with_capacity(scene.fixtures.len());
    let mut used_area = 0.0_f32;
    let mut order: Vec<usize> = (0..scene.fixtures.len()).collect();
    order.sort_by(|left, right| {
        let depth = |index: usize| {
            -camera
                .view
                .transform_point3(scene.fixtures[index].position)
                .z
        };
        depth(*left)
            .total_cmp(&depth(*right))
            .then_with(|| {
                scene.fixtures[*left]
                    .number
                    .cmp(&scene.fixtures[*right].number)
            })
            .then_with(|| {
                scene.fixtures[*left]
                    .address
                    .cmp(&scene.fixtures[*right].address)
            })
            .then_with(|| {
                scene.fixtures[*left]
                    .instance_id
                    .cmp(&scene.fixtures[*right].instance_id)
            })
    });
    for index in order {
        if lit[index].is_none() {
            continue;
        }
        let fixture = &scene.fixtures[index];
        let Some((x, y)) = camera.project(fixture.position, width, height) else {
            continue;
        };
        let ray = camera.ray_through(x, y, width, height);
        if viz_render::pick(scene, &ray, camera.position.distance(fixture.position)).element
            != viz_render::PickedElement::Fixture(index)
        {
            continue;
        }
        // Keep labels off the bottom status bar and the top-left mark.
        if y > height - 40.0 * ui_scale(width) {
            continue;
        }

        let number = fixture
            .number
            .map(|number| number.to_string())
            .unwrap_or_else(|| fixture.name.chars().take(18).collect());
        let address = match fixture.address {
            Some((universe, address)) => format!("{universe}.{address}"),
            None => "unpatched".to_owned(),
        };
        let text_width = Overlay::measure(&number, scale).max(Overlay::measure(&address, scale));
        let offset = 9.0 * scale;
        let right = x + offset;
        let label_x = if right + text_width <= width - 4.0 {
            right
        } else {
            x - offset - text_width
        };
        let label_y = y - line;
        if label_x < 4.0 || label_y < 4.0 || label_y + line * 2.0 > height - 40.0 * ui_scale(width)
        {
            continue;
        }
        let rect = [label_x, label_y, text_width, line * 2.0];
        let pad = 2.0 * scale;
        let collision_rect = [
            rect[0] - pad,
            rect[1] - pad,
            rect[2] + pad * 2.0,
            rect[3] + pad * 2.0,
        ];
        if placed
            .iter()
            .any(|existing| overlaps(*existing, collision_rect))
        {
            continue;
        }
        let label_area = collision_rect[2] * collision_rect[3];
        if used_area + label_area > width * height * 0.14 {
            continue;
        }
        used_area += label_area;
        placed.push(collision_rect);
        let panel = match theme {
            Theme::LightOnDark => srgb([0.015, 0.02, 0.03, 0.72]),
            Theme::DarkOnLight => srgb([0.94, 0.95, 0.97, 0.78]),
        };
        overlay.rect(
            collision_rect[0],
            collision_rect[1],
            collision_rect[2],
            collision_rect[3],
            panel,
        );
        if !number.is_empty() {
            // The number is what an operator looks for first, so it carries the extra weight.
            overlay.bold_text(rect[0], rect[1], scale, label_ink, &number);
        }
        overlay.text(rect[0], y, scale, label_ink, &address);
    }
}

fn fixture_lighting(scene: &Scene, values: &SceneValues) -> Vec<Option<(f32, [f32; 3])>> {
    let fallback = viz_scene::EmitterValues::default();
    let mut lit = vec![None; scene.fixtures.len()];
    for (index, emitter) in scene.emitters.iter().enumerate() {
        if emitter.kind == viz_scene::EmitterKind::Atmosphere {
            continue;
        }
        let value = values.emitters.get(index).unwrap_or(&fallback);
        let intensity = value.visible_intensity();
        let Some(slot) = lit.get_mut(emitter.fixture_index as usize) else {
            continue;
        };
        if slot.is_none_or(|(existing, _)| intensity > existing) {
            *slot = Some((intensity, value.colour));
        }
    }
    lit
}

#[allow(clippy::too_many_arguments)]
fn build_plot_fixture_labels(
    overlay: &mut Overlay,
    scene: &Scene,
    camera: &ResolvedCamera,
    width: f32,
    height: f32,
    scale: f32,
    line: f32,
    label_ink: [f32; 4],
    show_labels: bool,
    lit: &[Option<(f32, [f32; 3])>],
) {
    // Preserve the established plan contract exactly: emission dots remain even with labels
    // hidden, text follows scene order, and only direct label collisions drop text.
    let mut placed: Vec<[f32; 4]> = Vec::with_capacity(scene.fixtures.len());
    for (index, fixture) in scene.fixtures.iter().enumerate() {
        if lit[index].is_none() {
            continue;
        }
        let Some((x, y)) = camera.project(fixture.position, width, height) else {
            continue;
        };
        if y > height - 40.0 * ui_scale(width) {
            continue;
        }
        if let Some((intensity, colour)) = lit[index].filter(|(level, _)| *level > 0.004) {
            let level = 0.35 + 0.65 * intensity;
            overlay.disc(
                x - 8.0 * scale,
                y,
                2.6 * scale,
                [colour[0] * level, colour[1] * level, colour[2] * level, 1.0],
            );
        }
        if !show_labels {
            continue;
        }
        let number = fixture
            .number
            .map(|number| number.to_string())
            .unwrap_or_default();
        let address = match fixture.address {
            Some((universe, address)) => format!("{universe}.{address}"),
            None => "unpatched".to_owned(),
        };
        let text_width = Overlay::measure(&number, scale).max(Overlay::measure(&address, scale));
        let rect = [x + 9.0 * scale, y - line, text_width, line * 2.0];
        if placed.iter().any(|existing| overlaps(*existing, rect)) {
            continue;
        }
        placed.push(rect);
        if !number.is_empty() {
            overlay.bold_text(rect[0], rect[1], scale, label_ink, &number);
        }
        overlay.text(rect[0], y, scale, label_ink, &address);
    }
}

fn overlaps(left: [f32; 4], right: [f32; 4]) -> bool {
    left[0] < right[0] + right[2]
        && right[0] < left[0] + left[2]
        && left[1] < right[1] + right[3]
        && right[1] < left[1] + left[3]
}

#[cfg(test)]
mod fixture_label_tests {
    use super::*;
    use glam::Vec3;
    use viz_scene::{
        Camera, EmitterInstance, EmitterKind, EmitterLayoutCells, EmitterOptics, FixtureBody,
        FixtureInstance, Scene, SceneValues, SceneryKind, SceneryObject, ViewConfiguration,
        ViewMode,
    };

    fn fixture(number: u32, position: Vec3) -> FixtureInstance {
        FixtureInstance {
            instance_id: viz_scene::uuid::Uuid::new_v4(),
            fixture_id: viz_scene::uuid::Uuid::new_v4(),
            name: format!("Fixture {number}"),
            number: Some(number),
            position,
            rotation_degrees: Vec3::ZERO,
            bracket_degrees: 0.0,
            shaper_degrees: None,
            installed_colour: [1.0; 3],
            installed_shaper_angles_degrees: [0.0; 4],
            body: FixtureBody::default(),
            patched: true,
            address: Some((1, number as u16)),
            model: None,
            fallback: None,
        }
    }

    fn labels(scene: &Scene, show_labels: bool) -> Overlay {
        let mut view = ViewConfiguration::default();
        view.show_labels = show_labels;
        let camera =
            ResolvedCamera::resolve(&Camera::default(), view.mode, 1600.0 / 900.0, scene.bounds);
        let mut overlay = Overlay::default();
        build_fixture_labels(
            &mut overlay,
            scene,
            &SceneValues::default(),
            &camera,
            &view,
            1600.0,
            900.0,
        );
        overlay
    }

    fn push_lamp(scene: &mut Scene, fixture: FixtureInstance) {
        let fixture_index = scene.fixtures.len() as u32;
        scene.fixtures.push(fixture);
        scene.emitters.push(EmitterInstance {
            fixture_index,
            head_index: 0,
            label: "Main".into(),
            local_origin: Vec3::ZERO,
            tilt_pivot: Vec3::ZERO,
            local_orientation_degrees: Vec3::ZERO,
            pan: None,
            tilt: None,
            beam_angle_degrees: 10.0,
            field_angle_degrees: 20.0,
            optics: EmitterOptics::default(),
            kind: EmitterKind::Beam,
            cells: EmitterLayoutCells::single(),
            laser: None,
            live_shaper_angle_roles: [false; 4],
            shaper_roles: [false; 4],
            live_shaper_rotation_role: false,
        });
    }

    #[test]
    fn full_3d_labels_are_screen_space_and_obey_the_authoritative_switch() {
        let mut near = Scene::default();
        push_lamp(&mut near, fixture(7, Vec3::new(0.0, 3.0, 0.0)));
        near.recompute_bounds();
        let visible = labels(&near, true);
        assert!(!visible.quads.is_empty(), "Full 3D receives a fixture tag");
        assert!(
            labels(&near, false).quads.is_empty(),
            "show_labels is authoritative"
        );

        let mut far = Scene::default();
        push_lamp(&mut far, fixture(7, Vec3::new(0.0, 3.0, -6.0)));
        far.recompute_bounds();
        let far = labels(&far, true);
        assert_eq!(
            visible.quads[0].rect[3], far.quads[0].rect[3],
            "tag height is constant in physical pixels instead of shrinking with distance"
        );
    }

    #[test]
    fn dense_overlap_keeps_only_the_nearest_label_deterministically() {
        let camera = Camera::default();
        let near_position = camera.target;
        let far_position = camera.position + (camera.target - camera.position) * 1.5;
        let near = fixture(1, near_position);
        let mut single = Scene::default();
        push_lamp(&mut single, near.clone());
        single.recompute_bounds();
        let expected = labels(&single, true);

        let mut dense = Scene::default();
        for number in 100..180 {
            push_lamp(&mut dense, fixture(number, far_position));
        }
        // Deliberately append the near fixture after every far one: depth, not source order, wins.
        push_lamp(&mut dense, near);
        dense.recompute_bounds();
        let actual = labels(&dense, true);

        assert_eq!(
            actual.quads.len(),
            expected.quads.len(),
            "colliding far labels are dropped instead of blanketing the rig"
        );
        assert_eq!(actual.quads[0].rect, expected.quads[0].rect);
    }

    #[test]
    fn labels_skip_non_lamps_and_lamps_hidden_by_scenery() {
        let position = Camera::default().target;
        let mut machine_only = Scene::default();
        machine_only.fixtures.push(fixture(1, position));
        machine_only.recompute_bounds();
        assert!(
            labels(&machine_only, true).quads.is_empty(),
            "a Venue object or non-light-producing machine never receives a label"
        );

        let mut visible = Scene::default();
        push_lamp(&mut visible, fixture(2, position));
        visible.recompute_bounds();
        assert!(!labels(&visible, true).quads.is_empty());

        let camera = Camera::default();
        let mut hidden = visible;
        hidden.scenery.push(SceneryObject {
            id: viz_scene::uuid::Uuid::new_v4(),
            name: "Front curtain".into(),
            position: camera.position.lerp(position, 0.5),
            rotation_degrees: Vec3::ZERO,
            size: Vec3::splat(3.0),
            colour: [0.1; 3],
            roughness: 1.0,
            kind: SceneryKind::Curtain,
            chords: 0,
        });
        hidden.recompute_bounds();
        assert!(
            labels(&hidden, true).quads.is_empty(),
            "the curtain suppresses the fixture's floating label"
        );
    }

    #[test]
    fn plan_colour_dot_and_plain_text_contract_are_unchanged() {
        let mut scene = Scene::default();
        scene.fixtures.push(fixture(7, Vec3::new(0.0, 3.0, 0.0)));
        scene.emitters.push(EmitterInstance {
            fixture_index: 0,
            head_index: 0,
            label: "Main".into(),
            local_origin: Vec3::ZERO,
            tilt_pivot: Vec3::ZERO,
            local_orientation_degrees: Vec3::ZERO,
            pan: None,
            tilt: None,
            beam_angle_degrees: 10.0,
            field_angle_degrees: 20.0,
            optics: EmitterOptics::default(),
            kind: EmitterKind::Beam,
            cells: EmitterLayoutCells::single(),
            laser: None,
            live_shaper_angle_roles: [false; 4],
            shaper_roles: [false; 4],
            live_shaper_rotation_role: false,
        });
        scene.recompute_bounds();
        let mut values = SceneValues::default();
        values.resize(1);
        values.emitters[0].intensity = 1.0;
        values.emitters[0].held_intensity = 1.0;
        let render = |show_labels| {
            let mut view = ViewConfiguration::default();
            view.mode = ViewMode::TopDown;
            view.show_labels = show_labels;
            let camera = ResolvedCamera::resolve(
                &Camera::framed(view.mode, scene.bounds),
                view.mode,
                1600.0 / 900.0,
                scene.bounds,
            );
            let mut overlay = Overlay::default();
            build_fixture_labels(&mut overlay, &scene, &values, &camera, &view, 1600.0, 900.0);
            overlay
        };

        let dots_only = render(false);
        let labelled = render(true);
        assert_eq!(
            dots_only.quads.len(),
            7,
            "the established seven-span colour dot remains"
        );
        assert_eq!(
            labelled.quads.len(),
            12,
            "dot plus plain number/address glyphs, no panel"
        );
        for (labelled_dot, original_dot) in labelled.quads[..7].iter().zip(&dots_only.quads) {
            assert_eq!(labelled_dot.rect, original_dot.rect);
            assert_eq!(labelled_dot.colour, original_dot.colour);
        }
    }
}
