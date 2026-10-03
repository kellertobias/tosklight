//! The per-frame drawing style a view hands the instance builder.

use glam::Vec3;

/// How one frame should be drawn.
#[derive(Clone, Debug)]
pub struct FrameStyle {
    pub quality: viz_scene::RenderQuality,
    pub draw_beams: bool,
    pub draw_aim_lines: bool,
    /// Draw the scene as an outline plan instead of a shaded picture.
    pub plot: bool,
    /// Screen-plane axes used to billboard plot symbols so they read from any plan direction.
    pub plot_right: Vec3,
    pub plot_up: Vec3,
    pub projection_view: viz_scene::ProjectionView,
    /// World size one plot symbol should occupy, chosen so a symbol keeps a constant on-screen
    /// size however far the plan is zoomed out.
    pub symbol_metres: f32,
    /// Ink colour for a fixture that makes light.
    pub ink: Vec3,
    /// Ink colour for scenery and for a fixture that makes no light.
    pub faint_ink: Vec3,
    /// The one colour every beam is drawn in on a plan.
    pub beam_ink: Vec3,
    /// Ink for a fixture symbol or outline. Quieter than [`Self::ink`], which is for the things a
    /// plan is read *for*: a rig has far more lanterns on it than anything else, and drawn at full
    /// strength they are what the eye lands on instead of the light.
    pub symbol_ink: Vec3,
    /// Ink for a fixture the operator has selected — the one thing allowed to stand out.
    pub selected_ink: Vec3,
    /// Ink for the members of a whole selected Venue group, apart from one element on its own.
    pub group_selected_ink: Vec3,
    /// Draw each fixture's own model, rather than a box standing where it is.
    pub fixture_models: bool,
    /// Draw the emitting faces that belong to a simulated-light picture.
    pub emitter_apertures: bool,
    /// Draw retained scenery as shaded surfaces instead of quiet outlines.
    pub scenery_surfaces: bool,
    /// Draw an aim guideline for every directional emitter, lit or not.
    pub aim_guides: bool,
    /// Lay the reference grid on the ground plane.
    pub floor_grid: bool,
    /// Which scenery this view draws at all.
    pub scenery: fn(viz_scene::SceneryKind) -> bool,
    /// Renderer-local fraction of every authored crowd to draw.
    pub crowd_amount: f32,
    /// Per-frame crowd budget selected from quality and the renderer's adaptive hardware ladder.
    pub crowd_person_budget: usize,
    /// Per-frame Effect-particle budget selected from quality and the same adaptive hardware
    /// ladder as the expensive Ultra rendering features.
    pub effect_particle_budget: usize,
    /// Live/fallback media is a standalone capability. Helpers and embedded Stage panes draw the
    /// same authored geometry with neutral faces and open no media transport.
    pub media_content: bool,
    /// Current decoded appearance by Media Surface identity. This is volatile renderer state,
    /// never authored show intent.
    pub media_appearance: std::collections::BTreeMap<viz_scene::uuid::Uuid, MediaAppearance>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MediaAppearance {
    pub average: Vec3,
    pub flicker: f32,
}

impl Default for FrameStyle {
    fn default() -> Self {
        Self {
            quality: viz_scene::RenderQuality::High,
            draw_beams: true,
            draw_aim_lines: false,
            plot: false,
            plot_right: Vec3::X,
            plot_up: Vec3::Y,
            projection_view: viz_scene::ProjectionView::Top,
            symbol_metres: 0.3,
            beam_ink: Vec3::new(1.0, 0.82, 0.25),
            ink: Vec3::splat(0.85),
            faint_ink: Vec3::splat(0.35),
            symbol_ink: Vec3::splat(0.42),
            selected_ink: Vec3::new(0.25, 0.6, 1.0),
            group_selected_ink: Vec3::new(0.74, 0.47, 1.0),
            fixture_models: true,
            emitter_apertures: true,
            scenery_surfaces: true,
            aim_guides: false,
            floor_grid: true,
            scenery: |_| true,
            crowd_amount: 1.0,
            crowd_person_budget: 384,
            effect_particle_budget: 2_048,
            media_content: true,
            media_appearance: std::collections::BTreeMap::new(),
        }
    }
}
