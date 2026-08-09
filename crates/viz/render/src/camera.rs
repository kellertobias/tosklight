//! View and projection matrices, and the operator's local camera control.

use glam::{Mat4, Quat, Vec3};
use viz_scene::{Aabb, Camera, ViewMode};

/// A line into the scene, used to work out what the operator is pointing at.
#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub origin: Vec3,
    /// Unit length.
    pub direction: Vec3,
}

/// Resolved matrices plus the values shaders need for ray reconstruction.
#[derive(Clone, Copy, Debug)]
pub struct ResolvedCamera {
    pub view: Mat4,
    pub projection: Mat4,
    pub view_projection: Mat4,
    pub inverse_view_projection: Mat4,
    pub position: Vec3,
    pub near: f32,
    pub far: f32,
    pub orthographic: bool,
}

impl ResolvedCamera {
    pub fn resolve(camera: &Camera, mode: ViewMode, aspect: f32, bounds: Aabb) -> Self {
        let radius = bounds.radius().max(1.0);
        let near = 0.05_f32;
        let far = (radius * 12.0).max(60.0);
        let mut up = camera.up;
        let forward = camera.target - camera.position;
        if forward.length_squared() < 1e-8 {
            up = Vec3::Y;
        } else if forward.normalize().cross(up).length_squared() < 1e-6 {
            up = Vec3::Z;
        }
        let view = Mat4::look_at_rh(camera.position, camera.target, up);
        let orthographic = mode.is_orthographic();
        let projection = if orthographic {
            let half_height = camera.orthographic_size.max(0.1);
            let half_width = half_height * aspect.max(0.01);
            // Orthographic views frame the whole scene, so the depth range is centred on the
            // camera rather than starting in front of it.
            Mat4::orthographic_rh(
                -half_width,
                half_width,
                -half_height,
                half_height,
                -radius * 6.0,
                radius * 12.0,
            )
        } else {
            Mat4::perspective_rh(
                // Long-lens virtual cameras legitimately reach roughly 1.146 degrees at
                // 1,200 mm on the defined 36 x 24 mm sensor. Keep only a numerical-safety floor;
                // a presentation lens must not be silently widened by the renderer.
                camera.fov_degrees.clamp(0.5, 120.0).to_radians(),
                aspect.max(0.01),
                near,
                far,
            )
        };
        let view_projection = projection * view;
        Self {
            view,
            projection,
            view_projection,
            inverse_view_projection: view_projection.inverse(),
            position: camera.position,
            near,
            far,
            orthographic,
        }
    }

    /// Project a world point to physical pixels, or `None` when it is behind the camera.
    ///
    /// The plan views label fixtures with their number and address, which means placing text at a
    /// world position on the same page the renderer just drew.
    pub fn project(&self, world: Vec3, width: f32, height: f32) -> Option<(f32, f32)> {
        let clip = self.view_projection * world.extend(1.0);
        if clip.w <= 1e-5 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        if !(-1.4..=1.4).contains(&ndc.x) || !(-1.4..=1.4).contains(&ndc.y) {
            return None;
        }
        Some(((ndc.x * 0.5 + 0.5) * width, (0.5 - ndc.y * 0.5) * height))
    }

    /// The ray a pointer at these physical pixels casts into the scene.
    ///
    /// Both projections are handled: a perspective ray fans out from the eye, an orthographic one
    /// leaves the near plane parallel to its neighbours.
    pub fn ray_through(&self, x: f32, y: f32, width: f32, height: f32) -> Ray {
        let ndc_x = (x / width.max(1.0)) * 2.0 - 1.0;
        let ndc_y = 1.0 - (y / height.max(1.0)) * 2.0;
        let unproject = |depth: f32| {
            let point = self.inverse_view_projection * glam::Vec4::new(ndc_x, ndc_y, depth, 1.0);
            point.truncate() / point.w
        };
        // Depth `0` is the near plane in this projection's clip space and `1` the far plane.
        let near = unproject(0.0);
        let far = unproject(1.0);
        Ray {
            origin: near,
            direction: (far - near).normalize_or(Vec3::NEG_Z),
        }
    }
}

/// The operator's local camera, kept until the source sends another authoritative view message.
///
/// One control serves every named view. In the orthographic plan views its heading is ignored and
/// the mode's own axis is used, so panning and zooming a plan view can never tilt it off axis.
#[derive(Clone, Copy, Debug)]
pub struct CameraControl {
    /// The point the camera looks at. Panning and walking move this; orbiting rotates around it.
    pub target: Vec3,
    pub distance: f32,
    /// Heading in radians, used only by the perspective views.
    pub yaw: f32,
    /// Elevation in radians, used only by the perspective views.
    pub pitch: f32,
    pub orthographic_size: f32,
}

impl Default for CameraControl {
    fn default() -> Self {
        Self {
            target: Vec3::new(0.0, 2.5, 0.0),
            distance: 18.0,
            yaw: 0.0,
            pitch: 0.35,
            orthographic_size: 8.0,
        }
    }
}

impl CameraControl {
    /// Build a control that reproduces `camera`, for code that only needs its axes.
    pub fn from_camera(camera: &Camera) -> Self {
        let mut control = Self::default();
        control.adopt(camera);
        control
    }

    /// The page axes a plan view draws its symbols on: right and up as the operator sees them.
    pub fn page_axes(&self, mode: ViewMode) -> (Vec3, Vec3) {
        self.screen_axes(mode)
    }

    /// Reset the control so it reproduces `camera`.
    pub fn adopt(&mut self, camera: &Camera) {
        let offset = camera.position - camera.target;
        self.target = camera.target;
        self.distance = offset.length().max(0.5);
        self.yaw = offset.x.atan2(offset.z);
        self.pitch = (offset.y / self.distance).clamp(-0.99, 0.99).asin();
        self.orthographic_size = camera.orthographic_size;
    }

    /// Rotate around the target.
    ///
    /// Of the three turns here, [`CameraControl::look`] is the one the standalone application binds
    /// to a right-button drag; the other two are offered for a later selection-driven gesture. No
    /// turn applies to a plan view — that is defined by its axis.
    pub fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw += delta_yaw;
        // Stop just short of the poles so the up vector never collapses.
        self.pitch = (self.pitch + delta_pitch).clamp(-1.53, 1.53);
    }

    /// Where the camera itself sits for `mode`.
    pub fn eye(&self, mode: ViewMode) -> Vec3 {
        self.target - self.view_direction(mode) * self.distance
    }

    /// Turn on the spot: the camera keeps its position and swings its aim, the way someone standing
    /// in the room turns their head.
    ///
    /// A positive `delta_yaw` turns to the left and a positive `delta_pitch` aims further down, so
    /// a drag that moves the hand right and down is passed in negated on the yaw only.
    pub fn look(&mut self, mode: ViewMode, delta_yaw: f32, delta_pitch: f32) {
        let eye = self.eye(mode);
        self.yaw += delta_yaw;
        self.pitch = (self.pitch + delta_pitch).clamp(-1.53, 1.53);
        self.target = eye + self.view_direction(mode) * self.distance;
    }

    /// Swing around an arbitrary `pivot` rather than around the target.
    ///
    /// The whole camera is rotated rigidly about that point, so a point picked out of the scene
    /// stays where it is on screen while the rest of the rig turns around it. Orbiting about the
    /// point the camera already looks at behaves exactly like [`CameraControl::orbit`].
    pub fn orbit_about(&mut self, mode: ViewMode, pivot: Vec3, delta_yaw: f32, delta_pitch: f32) {
        let eye = self.eye(mode);
        let pitch = (self.pitch + delta_pitch).clamp(-1.53, 1.53);
        // Whatever the pitch clamp allowed is what the position is allowed to travel, so the eye
        // can never wind past an aim that has stopped at the pole.
        let applied_pitch = pitch - self.pitch;
        let (right, _) = self.screen_axes(mode);
        // Increasing pitch aims further down, which is a negative turn about the right axis.
        let rotation = Quat::from_axis_angle(Vec3::Y, delta_yaw)
            * Quat::from_axis_angle(right, -applied_pitch);
        let eye = pivot + rotation * (eye - pivot);
        self.yaw += delta_yaw;
        self.pitch = pitch;
        self.target = eye + self.view_direction(mode) * self.distance;
    }

    /// Zoom. In perspective this changes the orbit distance; in orthographic the framed height.
    /// Both change together so switching view keeps a comparable framing.
    pub fn zoom(&mut self, factor: f32) {
        self.distance = (self.distance * factor).clamp(0.4, 600.0);
        self.orthographic_size = (self.orthographic_size * factor).clamp(0.2, 600.0);
    }

    /// Move in the camera plane: right and up as the operator sees them, in pixels of drag.
    pub fn pan_camera_plane(&mut self, mode: ViewMode, right_pixels: f32, up_pixels: f32) {
        let (right, up) = self.screen_axes(mode);
        let scale = self.drag_scale(mode);
        self.target += right * right_pixels * scale + up * up_pixels * scale;
    }

    /// Move parallel to the stage floor, following where the camera is pointed, in pixels of drag.
    pub fn pan_floor_plane(&mut self, mode: ViewMode, right_pixels: f32, forward_pixels: f32) {
        let (right, forward) = self.floor_axes(mode);
        let scale = self.drag_scale(mode);
        self.target += right * right_pixels * scale + forward * forward_pixels * scale;
    }

    /// Walk the floor plane, in metres. `forward` is towards where the camera looks, projected
    /// onto the floor; `right` is to the operator's right. Height never changes.
    pub fn walk_floor_plane(&mut self, mode: ViewMode, forward: f32, right: f32) {
        let (right_axis, forward_axis) = self.floor_axes(mode);
        self.target += forward_axis * forward + right_axis * right;
    }

    /// Metres of world movement per pixel of drag, so a drag feels the same at any zoom.
    fn drag_scale(&self, mode: ViewMode) -> f32 {
        if mode.is_orthographic() {
            self.orthographic_size * 0.0025
        } else {
            self.distance * 0.0018
        }
    }

    /// The world up this control hands to the view matrix. It has to agree exactly with
    /// [`Camera::framed`]: if the two disagree, the picture mirrors the moment the operator
    /// touches the camera and the framed view is replaced by the controlled one.
    fn world_up(&self, mode: ViewMode) -> Vec3 {
        let direction = self.view_direction(mode);
        if direction.y.abs() > 0.9 {
            // Looking straight down or up: keep the stage's front edge at the bottom of frame.
            Vec3::new(0.0, 0.0, -1.0)
        } else {
            Vec3::Y
        }
    }

    /// The camera's right and up axes in world space, derived the same way the view matrix
    /// derives them, so a drag and a plan symbol agree with what is actually on screen.
    fn screen_axes(&self, mode: ViewMode) -> (Vec3, Vec3) {
        let forward = self.view_direction(mode);
        let right = forward.cross(self.world_up(mode)).normalize_or(Vec3::X);
        let up = right.cross(forward).normalize_or(Vec3::Y);
        (right, up)
    }

    /// Right and forward axes flattened onto the floor plane.
    fn floor_axes(&self, mode: ViewMode) -> (Vec3, Vec3) {
        let forward = self.view_direction(mode);
        // Looking straight down leaves no horizontal heading, so the screen up axis stands in.
        let flat_forward = Vec3::new(forward.x, 0.0, forward.z);
        let forward_axis = if flat_forward.length_squared() < 1e-6 {
            let (_, up) = self.screen_axes(mode);
            Vec3::new(up.x, 0.0, up.z).normalize_or(Vec3::NEG_Z)
        } else {
            flat_forward.normalize()
        };
        let right_axis = forward_axis.cross(Vec3::Y).normalize_or(Vec3::X);
        (right_axis, forward_axis)
    }

    /// Where the camera looks. A plan view always uses its own axis.
    pub fn view_direction(&self, mode: ViewMode) -> Vec3 {
        mode.orthographic_direction().unwrap_or_else(|| {
            Vec3::new(
                -self.yaw.sin() * self.pitch.cos(),
                -self.pitch.sin(),
                -self.yaw.cos() * self.pitch.cos(),
            )
            .normalize_or(Vec3::NEG_Z)
        })
    }

    /// Build the camera this control describes for `mode`.
    pub fn camera(&self, base: &Camera, mode: ViewMode) -> Camera {
        let direction = self.view_direction(mode);
        Camera {
            position: self.target - direction * self.distance,
            target: self.target,
            up: self.world_up(mode),
            fov_degrees: base.fov_degrees,
            orthographic_size: self.orthographic_size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control() -> CameraControl {
        let mut control = CameraControl::default();
        control.adopt(&Camera {
            position: Vec3::new(0.0, 4.0, 20.0),
            target: Vec3::new(0.0, 4.0, 0.0),
            ..Camera::default()
        });
        control
    }

    /// The framed camera and the controlled camera have to describe the same picture. When they
    /// disagree the view mirrors the instant the operator first zooms or pans, which is exactly
    /// what an operator reports as "pressing 2 and then zooming flips something".
    #[test]
    fn taking_control_of_a_plan_view_never_mirrors_it() {
        let bounds = Aabb {
            min: Vec3::new(-6.0, 0.0, -4.0),
            max: Vec3::new(6.0, 8.0, 4.0),
        };
        for mode in [
            ViewMode::TopDown,
            ViewMode::FrontToBack,
            ViewMode::BackToFront,
            ViewMode::LeftToRight,
            ViewMode::RightToLeft,
        ] {
            let framed = Camera::framed(mode, bounds);
            let mut control = CameraControl::from_camera(&framed);
            control.zoom(0.9);
            let taken = control.camera(&framed, mode);
            assert!(
                framed.up.dot(taken.up) > 0.99,
                "{mode:?} flipped its up vector when the operator took control: \
                 {:?} then {:?}",
                framed.up,
                taken.up
            );

            // The same point must stay on the same side of the picture.
            let probe = Vec3::new(3.0, 1.0, 2.0);
            let before = ResolvedCamera::resolve(&framed, mode, 1.6, bounds)
                .project(probe, 1600.0, 900.0)
                .expect("probe is in frame before");
            let after = ResolvedCamera::resolve(&taken, mode, 1.6, bounds)
                .project(probe, 1600.0, 900.0)
                .expect("probe is in frame after");
            assert_eq!(
                (before.0 > 800.0, before.1 > 450.0),
                (after.0 > 800.0, after.1 > 450.0),
                "{mode:?} moved a fixed point across the picture: {before:?} then {after:?}"
            );
        }
    }

    /// Dragging moves the scene with the cursor. The operator grabs the stage, not the camera.
    #[test]
    fn a_drag_moves_the_picture_the_way_the_hand_moves() {
        let bounds = Aabb {
            min: Vec3::new(-6.0, 0.0, -4.0),
            max: Vec3::new(6.0, 8.0, 4.0),
        };
        for mode in [ViewMode::TopDown, ViewMode::FrontToBack, ViewMode::Full3d] {
            let framed = Camera::framed(mode, bounds);
            let mut control = CameraControl::from_camera(&framed);
            let probe = Vec3::ZERO;
            let before = ResolvedCamera::resolve(&framed, mode, 1.6, bounds)
                .project(probe, 1600.0, 900.0)
                .expect("in frame");
            // A drag of 60 pixels to the right, as the application reports it.
            control.pan_floor_plane(mode, -60.0, 0.0);
            let after = ResolvedCamera::resolve(&control.camera(&framed, mode), mode, 1.6, bounds)
                .project(probe, 1600.0, 900.0)
                .expect("still in frame");
            assert!(
                after.0 > before.0 + 5.0,
                "{mode:?} moved the stage the wrong way: {before:?} then {after:?}"
            );
        }
    }

    #[test]
    fn adopting_a_camera_reproduces_its_position() {
        let camera = Camera {
            position: Vec3::new(4.0, 5.0, 9.0),
            target: Vec3::new(0.0, 2.0, 0.0),
            ..Camera::default()
        };
        let mut control = CameraControl::default();
        control.adopt(&camera);
        let restored = control.camera(&camera, ViewMode::Full3d);
        assert!((restored.position - camera.position).length() < 1e-3);
    }

    #[test]
    fn orthographic_projection_is_selected_for_named_plan_views() {
        let bounds = Aabb {
            min: Vec3::splat(-4.0),
            max: Vec3::splat(4.0),
        };
        let camera = Camera::framed(ViewMode::TopDown, bounds);
        let resolved = ResolvedCamera::resolve(&camera, ViewMode::TopDown, 1.6, bounds);
        assert!(resolved.orthographic);
        let perspective = ResolvedCamera::resolve(&camera, ViewMode::Full3d, 1.6, bounds);
        assert!(!perspective.orthographic);
    }

    #[test]
    fn a_long_virtual_lens_keeps_its_narrow_field_of_view() {
        let camera = Camera {
            fov_degrees: 1.145_877,
            ..Camera::default()
        };
        let resolved =
            ResolvedCamera::resolve(&camera, ViewMode::Full3d, 16.0 / 9.0, Aabb::default());
        let expected = 1.0 / (camera.fov_degrees.to_radians() * 0.5).tan();
        assert!((resolved.projection.y_axis.y - expected).abs() < 1e-3);
    }

    #[test]
    fn a_plan_view_keeps_its_exact_axis_however_the_operator_moves() {
        let mut control = control();
        control.orbit(1.2, 0.8);
        control.pan_floor_plane(ViewMode::TopDown, 50.0, 30.0);
        control.zoom(0.5);
        let camera = control.camera(&Camera::default(), ViewMode::TopDown);
        let direction = (camera.target - camera.position).normalize();
        assert!(
            (direction - Vec3::NEG_Y).length() < 1e-5,
            "top down must look straight down, got {direction:?}"
        );
        let front = control.camera(&Camera::default(), ViewMode::FrontToBack);
        let front_direction = (front.target - front.position).normalize();
        assert!((front_direction - Vec3::NEG_Z).length() < 1e-5);
    }

    /// Left-dragging pans and tilts on the spot. An operator who has walked to a position expects
    /// to look around from exactly there, not to be swung somewhere else.
    #[test]
    fn looking_turns_the_camera_without_moving_it() {
        let mut control = control();
        let before = control.eye(ViewMode::Full3d);
        let direction = control.view_direction(ViewMode::Full3d);
        control.look(ViewMode::Full3d, 0.4, 0.2);
        let after = control.eye(ViewMode::Full3d);
        assert!(
            (after - before).length() < 1e-3,
            "the camera moved while looking around: {before:?} then {after:?}"
        );
        assert!(
            control
                .view_direction(ViewMode::Full3d)
                .dot(direction)
                .abs()
                < 0.999,
            "looking around has to change where the camera points"
        );
    }

    /// The signs a right-button drag depends on: turning the camera to the right sweeps the scene
    /// to the left, and tilting it down sweeps the scene up, the way it does for someone standing
    /// in the room who turns their head.
    #[test]
    fn a_turn_sweeps_the_scene_the_other_way() {
        let bounds = Aabb {
            min: Vec3::new(-6.0, 0.0, -4.0),
            max: Vec3::new(6.0, 8.0, 4.0),
        };
        let base = Camera::default();
        let probe = Vec3::new(0.0, 4.0, 0.0);
        let project = |control: &CameraControl| {
            ResolvedCamera::resolve(
                &control.camera(&base, ViewMode::Full3d),
                ViewMode::Full3d,
                1.6,
                bounds,
            )
            .project(probe, 1600.0, 900.0)
            .expect("the probe stays in frame")
        };

        let control = control();
        let before = project(&control);

        // A drag to the right, as the application passes it in.
        let mut turned_right = control;
        turned_right.look(ViewMode::Full3d, -0.3, 0.0);
        let after = project(&turned_right);
        assert!(
            after.0 < before.0 - 5.0,
            "turning right must sweep the scene left: {before:?} then {after:?}"
        );

        // A drag downwards.
        let mut tilted_down = control;
        tilted_down.look(ViewMode::Full3d, 0.0, 0.3);
        let after = project(&tilted_down);
        assert!(
            after.1 < before.1 - 5.0,
            "tilting down must sweep the scene up: {before:?} then {after:?}"
        );
    }

    /// The pitch clamp holds for looking as well, or the picture rolls over at the poles.
    #[test]
    fn looking_stops_short_of_straight_up_and_straight_down() {
        let mut control = control();
        for _ in 0..40 {
            control.look(ViewMode::Full3d, 0.0, 0.2);
        }
        assert!(control.pitch <= 1.53);
        for _ in 0..80 {
            control.look(ViewMode::Full3d, 0.0, -0.2);
        }
        assert!(control.pitch >= -1.53);
    }

    /// The whole point of orbiting what was clicked: that element stays under the cursor while the
    /// rig turns around it.
    #[test]
    fn orbiting_a_clicked_point_keeps_it_where_it_is_on_screen() {
        let bounds = Aabb {
            min: Vec3::new(-8.0, 0.0, -6.0),
            max: Vec3::new(8.0, 9.0, 6.0),
        };
        let base = Camera::default();
        let mut control = control();
        // A point well off to one side, as if the operator clicked a fixture near the edge.
        let pivot = Vec3::new(5.0, 6.0, -2.0);
        let before = ResolvedCamera::resolve(
            &control.camera(&base, ViewMode::Full3d),
            ViewMode::Full3d,
            1.6,
            bounds,
        )
        .project(pivot, 1600.0, 900.0)
        .expect("the pivot starts in frame");
        for _ in 0..12 {
            control.orbit_about(ViewMode::Full3d, pivot, 0.05, 0.02);
        }
        let after = ResolvedCamera::resolve(
            &control.camera(&base, ViewMode::Full3d),
            ViewMode::Full3d,
            1.6,
            bounds,
        )
        .project(pivot, 1600.0, 900.0)
        .expect("the pivot stays in frame");
        assert!(
            (after.0 - before.0).abs() < 2.0 && (after.1 - before.1).abs() < 2.0,
            "the clicked point slid across the picture: {before:?} then {after:?}"
        );
        assert!(
            (control.eye(ViewMode::Full3d) - pivot).length() > 0.5,
            "the camera has to keep its distance from what it orbits"
        );
    }

    /// Orbiting the point the camera already looks at is the plain turntable, unchanged.
    #[test]
    fn orbiting_the_current_target_matches_the_plain_orbit() {
        let mut turntable = control();
        let mut about_target = control();
        let pivot = about_target.target;
        turntable.orbit(0.3, 0.1);
        about_target.orbit_about(ViewMode::Full3d, pivot, 0.3, 0.1);
        assert!((turntable.target - about_target.target).length() < 1e-3);
        assert!(
            (turntable.eye(ViewMode::Full3d) - about_target.eye(ViewMode::Full3d)).length() < 1e-3
        );
    }

    #[test]
    fn walking_never_changes_height() {
        let mut control = control();
        let before = control.target.y;
        control.walk_floor_plane(ViewMode::Full3d, 3.0, 2.0);
        assert_eq!(control.target.y, before);
    }

    #[test]
    fn walking_forward_follows_where_the_camera_points() {
        let mut straight = control();
        // Looking down the negative Z axis from the audience.
        straight.walk_floor_plane(ViewMode::Full3d, 2.0, 0.0);
        assert!(
            straight.target.z < -1.9,
            "forward must move upstage, got {}",
            straight.target.z
        );
        let mut turned = control();
        turned.orbit(std::f32::consts::FRAC_PI_2, 0.0);
        turned.walk_floor_plane(ViewMode::Full3d, 2.0, 0.0);
        assert!(
            turned.target.x.abs() > 1.9,
            "after a quarter turn forward must move across the stage"
        );
    }

    #[test]
    fn the_camera_plane_pan_can_change_height_but_the_floor_pan_cannot() {
        let mut screen = control();
        screen.orbit(0.0, 0.4);
        let before = screen.target.y;
        screen.pan_camera_plane(ViewMode::Full3d, 0.0, 100.0);
        assert!(
            (screen.target.y - before).abs() > 1e-3,
            "dragging up in the camera plane must lift the view"
        );

        let mut floor = control();
        let height = floor.target.y;
        floor.pan_floor_plane(ViewMode::Full3d, 100.0, 100.0);
        assert_eq!(floor.target.y, height);
    }

    #[test]
    fn zooming_stays_within_usable_bounds() {
        let mut control = control();
        for _ in 0..200 {
            control.zoom(0.5);
        }
        assert!(control.distance >= 0.4);
        assert!(control.orthographic_size >= 0.2);
        for _ in 0..200 {
            control.zoom(2.0);
        }
        assert!(control.distance <= 600.0);
        assert!(control.orthographic_size <= 600.0);
    }

    #[test]
    fn a_drag_moves_further_when_zoomed_out() {
        let mut near = control();
        near.zoom(0.25);
        let near_before = near.target;
        near.pan_floor_plane(ViewMode::Full3d, 100.0, 0.0);
        let near_moved = (near.target - near_before).length();

        let mut far = control();
        far.zoom(4.0);
        let far_before = far.target;
        far.pan_floor_plane(ViewMode::Full3d, 100.0, 0.0);
        assert!((far.target - far_before).length() > near_moved);
    }
}
