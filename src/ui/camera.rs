#[derive(PartialEq, Eq)]
pub enum CameraRecenterKind {
    None,
    OnScrollBorder,
    OnCursor,
}

pub const RECENTER_DISTANCE: usize = 3;
const SCROLL_SPEED: f32 = 30.0;
const SCROLL_FRICTION: f32 = 0.0001;

pub struct Camera {
    y: f32,
    velocity_y: f32,
    recenter_kind: CameraRecenterKind,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            y: 0.0,
            velocity_y: 0.0,
            recenter_kind: CameraRecenterKind::None,
        }
    }

    pub fn is_moving(&self) -> bool {
        self.velocity_y != 0.0
    }

    pub fn update(
        &mut self,
        target_y: f32,
        max_y: f32,
        height: f32,
        scroll_border_top: f32,
        scroll_border_bottom: f32,
        can_recenter: bool,
        dt: f32,
    ) {
        if can_recenter {
            let is_target_outside_border =
                target_y < scroll_border_top || target_y > scroll_border_bottom;

            self.recenter_kind = if is_target_outside_border {
                CameraRecenterKind::OnScrollBorder
            } else {
                CameraRecenterKind::None
            };
        }

        if self.recenter_kind != CameraRecenterKind::None {
            let visual_distance = match self.recenter_kind {
                CameraRecenterKind::OnScrollBorder => {
                    if target_y < height / 2.0 {
                        target_y - scroll_border_top
                    } else {
                        target_y - scroll_border_bottom
                    }
                }
                _ => target_y - height / 2.0,
            };

            // We can't move the camera past the top of the document,
            // (eg. if the cursor is on the first line, it might be too close to the edge of the
            // screen according to RECENTER_DISTANCE, but there's nothing we can do about it, so stop animating).
            let visual_distance = (visual_distance + self.y).max(0.0) - self.y;

            self.scroll_visual_distance(visual_distance);
        }

        self.velocity_y *= SCROLL_FRICTION.powf(dt);

        // We want the velocity to eventually be exactly zero so that we can stop animating.
        if self.velocity_y.abs() < 0.5 {
            self.velocity_y = 0.0;

            // If we're recentering the camera then we must be done at this point.
            self.recenter_kind = CameraRecenterKind::None;
        }

        self.y += self.velocity_y * dt;
        self.y = self.y.clamp(0.0, max_y);
    }

    pub fn recenter(&mut self, recenter_kind: CameraRecenterKind) {
        self.recenter_kind = recenter_kind;
    }

    pub fn scroll_visual_distance(&mut self, visual_distance: f32) {
        let f = SCROLL_FRICTION;
        let t = 1.0; // Time to scroll to destination.

        // Velocity of the camera is (v = starting velocity, f = friction factor): v * f^t
        // Integrate to get position: y = (v * f^t) / ln(f)
        // Add term so we start at zero: y = (v * f^t) / ln(f) - v / ln(f)
        // Solve for v:
        let v = (visual_distance * f.ln()) / (f.powf(t) - 1.0);

        self.velocity_y = v;
    }

    pub fn scroll(&mut self, delta: f32) {
        self.recenter_kind = CameraRecenterKind::None;
        self.velocity_y -= delta * SCROLL_SPEED;
    }

    pub fn reset(&mut self) {
        self.y = 0.0;
        self.velocity_y = 0.0;
        self.recenter_kind = CameraRecenterKind::None;
    }

    pub fn y(&self) -> f32 {
        self.y
    }
}
