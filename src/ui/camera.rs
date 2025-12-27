use crate::geometry::visual_position::VisualPosition;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CameraRecenterKind {
    #[default]
    None,
    OnScrollBorder,
    OnCursor,
}

pub struct CameraRecenterRequest {
    pub can_start: bool,
    pub target_position: f32,
    pub scroll_border: f32,
}

impl CameraRecenterRequest {
    fn needs_recenter(&self, view_size: f32) -> bool {
        self.can_start && needs_recenter(self.target_position, self.scroll_border, view_size)
    }
}

#[derive(Debug, Clone, Copy)]
enum CameraState {
    Locked,
    MovingWithLerp {
        target_position: f32,
    },
    MovingWithVelocity,
    NeedsRecenter {
        kind: CameraRecenterKind,
    },
    Recentering {
        kind: CameraRecenterKind,
        target_position: f32,
        scroll_border: f32,
    },
}

pub const RECENTER_DISTANCE: usize = 4;
const SCROLL_SPEED: f32 = 30.0;
const PRECISE_SCROLL_SCALE: f32 = 0.1;
const PRECISE_SCROLL_SPEED: f32 = 50.0;
const SCROLL_FRICTION: f32 = 0.0001;

pub struct CameraAxis {
    position: f32,
    max_position: f32,
    velocity: f32,
    state: CameraState,
}

impl CameraAxis {
    pub fn new() -> Self {
        Self {
            position: 0.0,
            max_position: 0.0,
            velocity: 0.0,
            state: CameraState::MovingWithVelocity,
        }
    }

    pub fn is_moving(&self) -> bool {
        self.velocity != 0.0 || matches!(self.state, CameraState::MovingWithLerp { .. })
    }

    pub fn animate(
        &mut self,
        recenter_request: CameraRecenterRequest,
        max_position: f32,
        view_size: f32,
        dt: f32,
    ) {
        if matches!(self.state, CameraState::Locked) {
            self.reset();
            self.state = CameraState::Locked;
            return;
        }

        self.max_position = max_position;

        self.handle_recenter_request(recenter_request, view_size);
        self.handle_recenter(max_position, view_size);

        if let CameraState::MovingWithLerp { target_position } = self.state {
            self.velocity = 0.0;
            self.position += (target_position - self.position) * dt * PRECISE_SCROLL_SPEED;

            if (self.position - target_position).abs() < 0.5
                || (target_position < 0.0 && self.position < 0.0)
                || (target_position > max_position && self.position > max_position)
            {
                self.state = CameraState::MovingWithVelocity;
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
                self.state = CameraState::MovingWithVelocity;
            }
        }

        self.position = self.position.clamp(0.0, max_position);
    }

    fn handle_recenter_request(&mut self, recenter_request: CameraRecenterRequest, view_size: f32) {
        let target_position = self.position + recenter_request.target_position;
        let scroll_border = recenter_request.scroll_border;

        self.state = match self.state {
            CameraState::MovingWithVelocity | CameraState::Recentering { .. }
                if recenter_request.needs_recenter(view_size) =>
            {
                CameraState::Recentering {
                    kind: CameraRecenterKind::OnScrollBorder,
                    target_position,
                    scroll_border,
                }
            }
            CameraState::NeedsRecenter { kind } => CameraState::Recentering {
                kind,
                target_position,
                scroll_border,
            },
            state => state,
        };
    }

    fn handle_recenter(&mut self, max_position: f32, view_size: f32) {
        let CameraState::Recentering {
            kind,
            target_position,
            scroll_border,
        } = self.state
        else {
            return;
        };

        if !needs_recenter(target_position - self.position, scroll_border, view_size) {
            self.state = CameraState::MovingWithVelocity;
            return;
        }

        let target_position = target_position - self.position;

        let scroll_border_min = scroll_border;
        let scroll_border_max = view_size - scroll_border;

        let visual_distance = match kind {
            CameraRecenterKind::OnScrollBorder if scroll_border_min < scroll_border_max => {
                if target_position < view_size / 2.0 {
                    target_position - scroll_border_min
                } else {
                    target_position - scroll_border_max
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

    pub fn recenter(&mut self, kind: CameraRecenterKind) {
        self.state = CameraState::NeedsRecenter { kind };
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
        if is_precise {
            self.velocity = 0.0;

            let previous_target_position =
                if let CameraState::MovingWithLerp { target_position } = self.state {
                    target_position
                } else {
                    self.position
                };

            self.state = CameraState::MovingWithLerp {
                target_position: previous_target_position - delta * PRECISE_SCROLL_SCALE,
            };
        } else {
            self.velocity -= delta * SCROLL_SPEED;
            self.state = CameraState::MovingWithVelocity;
        }
    }

    pub fn jump_visual_distance(&mut self, visual_distance: f32) {
        self.position += visual_distance;
    }

    pub fn reset(&mut self) {
        self.position = 0.0;
        self.max_position = 0.0;
        self.velocity = 0.0;
        self.state = CameraState::MovingWithVelocity;
    }

    pub fn reset_velocity(&mut self) {
        self.velocity = 0.0;
        self.state = CameraState::MovingWithVelocity;
    }

    pub fn set_locked(&mut self, is_locked: bool) {
        if is_locked {
            self.state = CameraState::Locked;
            return;
        }

        if matches!(self.state, CameraState::Locked) {
            self.state = CameraState::MovingWithVelocity;
        }
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

    pub fn x(&self) -> f32 {
        self.horizontal.position
    }

    pub fn y(&self) -> f32 {
        self.vertical.position
    }

    pub fn position(&self) -> VisualPosition {
        VisualPosition::new(self.horizontal.position, self.vertical.position)
    }
}

fn needs_recenter(target_position: f32, scroll_border: f32, view_size: f32) -> bool {
    let scroll_border_min = scroll_border;
    let scroll_border_max = view_size - scroll_border;

    target_position < scroll_border_min || target_position > scroll_border_max
}
