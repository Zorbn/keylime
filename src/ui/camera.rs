use std::ops::RangeInclusive;

use crate::geometry::visual_position::VisualPosition;

#[derive(PartialEq, Eq)]
pub enum CameraRecenterKind {
    None,
    OnScrollBorder,
    OnCursor,
}

pub const RECENTER_DISTANCE: usize = 4;
const SCROLL_SPEED: f32 = 30.0;
const PRECISE_SCROLL_SCALE: f32 = 0.1;
const PRECISE_SCROLL_SPEED: f32 = 50.0;
const SCROLL_FRICTION: f32 = 0.0001;

pub struct CameraAxis {
    pub is_locked: bool,
    position: f32,
    target_position: Option<f32>,
    max_position: f32,
    velocity: f32,
    recenter_kind: CameraRecenterKind,
}

impl CameraAxis {
    pub fn new() -> Self {
        Self {
            is_locked: false,
            position: 0.0,
            target_position: None,
            max_position: 0.0,
            velocity: 0.0,
            recenter_kind: CameraRecenterKind::None,
        }
    }

    pub fn is_moving(&self) -> bool {
        self.velocity != 0.0 || self.target_position.is_some()
    }

    pub fn update(
        &mut self,
        target_position: f32,
        max_position: f32,
        view_size: f32,
        scroll_border: RangeInclusive<f32>,
        can_recenter: bool,
        dt: f32,
    ) {
        let can_recenter = can_recenter && self.recenter_kind == CameraRecenterKind::None;

        self.max_position = max_position;

        if self.is_locked {
            self.reset();
            return;
        }

        let min_scroll_border = *scroll_border.start();
        let max_scroll_border = *scroll_border.end();

        if can_recenter || self.recenter_kind == CameraRecenterKind::OnScrollBorder {
            let is_target_outside_border =
                target_position < min_scroll_border || target_position > max_scroll_border;

            self.recenter_kind = if is_target_outside_border {
                CameraRecenterKind::OnScrollBorder
            } else {
                CameraRecenterKind::None
            };
        }

        if self.recenter_kind != CameraRecenterKind::None {
            let visual_distance = match self.recenter_kind {
                CameraRecenterKind::OnScrollBorder if min_scroll_border < max_scroll_border => {
                    if target_position < view_size / 2.0 {
                        target_position - min_scroll_border
                    } else {
                        target_position - max_scroll_border
                    }
                }
                _ => target_position - view_size / 2.0,
            };

            // We can't move the camera past the top of the document,
            // (eg. if the cursor is on the first line, it might be too close to the edge of the
            // screen according to RECENTER_DISTANCE, but there's nothing we can do about it, so stop animating).
            let visual_distance =
                (visual_distance + self.position).clamp(0.0, max_position) - self.position;

            self.scroll_visual_distance(visual_distance);
        }

        if let Some(target_position) = self.target_position {
            self.velocity = 0.0;
            self.position += (target_position - self.position) * dt * PRECISE_SCROLL_SPEED;

            if (self.position - target_position).abs() < 0.5
                || (target_position < 0.0 && self.position < 0.0)
                || (target_position > max_position && self.position > max_position)
            {
                self.target_position = None;
            }
        } else {
            self.velocity *= SCROLL_FRICTION.powf(dt);
            self.position += self.velocity * dt;

            // We want the velocity to eventually be exactly zero so that we can stop animating.
            if self.velocity.abs() < 0.5
                || (self.velocity < 0.0 && self.position < 0.0)
                || (self.velocity > 0.0 && self.position > max_position)
            {
                self.velocity = 0.0;

                // If we're recentering the camera then we must be done at this point.
                self.recenter_kind = CameraRecenterKind::None;
            }
        }

        self.position = self.position.clamp(0.0, max_position);
    }

    pub fn recenter(&mut self, recenter_kind: CameraRecenterKind) {
        self.recenter_kind = recenter_kind;
    }

    pub fn scroll_visual_distance(&mut self, visual_distance: f32) {
        let f = SCROLL_FRICTION;

        // Velocity of the camera is (v = starting velocity, f = friction factor): v * f^t
        // Integrate to get position: y = (v * f^t) / ln(f)
        // Add term so we start at zero: y = (v * f^t) / ln(f) - v / ln(f)
        // Solve for v: v = (y * ln(f)) / (f^t - 1)
        // Limit as t approaches infinity:
        let v = visual_distance * f.ln() / -1.0;

        self.velocity = v;
    }

    pub fn scroll(&mut self, delta: f32, is_precise: bool) {
        self.recenter_kind = CameraRecenterKind::None;

        if is_precise {
            self.velocity = 0.0;

            let previous_target_position = self.target_position.unwrap_or(self.position);
            self.target_position = Some(previous_target_position - delta * PRECISE_SCROLL_SCALE);
        } else {
            self.velocity -= delta * SCROLL_SPEED;
            self.target_position = None;
        }

        self.position = self.position.clamp(0.0, self.max_position);
    }

    pub fn jump_visual_distance(&mut self, visual_distance: f32) {
        self.position += visual_distance;
    }

    pub fn reset(&mut self) {
        self.position = 0.0;
        self.max_position = 0.0;
        self.velocity = 0.0;
        self.recenter_kind = CameraRecenterKind::None;
    }

    pub fn reset_velocity(&mut self) {
        self.velocity = 0.0;
        self.recenter_kind = CameraRecenterKind::None;
    }
}

pub struct Camera {
    pub horizontal: CameraAxis,
    pub vertical: CameraAxis,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            horizontal: CameraAxis::new(),
            vertical: CameraAxis::new(),
        }
    }

    pub fn is_moving(&self) -> bool {
        self.horizontal.is_moving() || self.vertical.is_moving()
    }

    pub fn recenter(&mut self) {
        self.vertical.recenter(CameraRecenterKind::OnCursor);
        self.horizontal.recenter(CameraRecenterKind::OnScrollBorder);
    }

    pub fn reset(&mut self) {
        self.horizontal.reset();
        self.vertical.reset();
    }

    pub fn y(&self) -> f32 {
        self.vertical.position
    }

    pub fn position(&self) -> VisualPosition {
        VisualPosition::new(self.horizontal.position, self.vertical.position)
    }
}
