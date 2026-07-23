use crate::{MovementIntent, WorldPose};

/// Fixed simulation frequency for the deliberately narrow ground-movement
/// capability.  The worker drives this one tick at a time; it never derives
/// displacement from render-frame duration.
pub const PREDICTION_HZ: u32 = 60;
const PREDICTION_STEP_SECONDS: f32 = 1.0 / 60.0;
pub const HEARTBEAT_TICKS: u32 = PREDICTION_HZ / 10;
pub const REFERENCE_MOVEMENT_ENVELOPE_METRES: f32 = 5.0;

/// Deterministic, heading-aligned prediction around one authoritative entry
/// anchor.  It intentionally models neither terrain nor vertical motion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GroundPrediction {
    anchor: WorldPose,
    predicted: WorldPose,
    run_speed: f32,
    moving: bool,
    moving_ticks: u32,
}

impl GroundPrediction {
    pub(crate) fn new(anchor: WorldPose, run_speed: f32) -> Option<Self> {
        (run_speed.is_finite() && run_speed > 0.0).then_some(Self {
            anchor,
            predicted: anchor,
            run_speed,
            moving: false,
            moving_ticks: 0,
        })
    }

    pub(crate) const fn predicted(self) -> WorldPose {
        self.predicted
    }

    pub(crate) fn align_heading(&mut self, intent: MovementIntent) {
        if intent.engaged() {
            self.predicted.orientation = intent.east().atan2(intent.north());
        }
    }

    /// Advance exactly one 60 Hz tick and report the transition and any
    /// coalescible heartbeat due after the tick.
    pub(crate) fn tick(&mut self, intent: MovementIntent) -> MovementTick {
        let started = !self.moving && intent.engaged();
        let stopped = self.moving && !intent.engaged();
        self.moving = intent.engaged();

        if self.moving {
            let heading = intent.east().atan2(intent.north());
            let distance = self.run_speed * PREDICTION_STEP_SECONDS;
            let mut east = self.predicted.east + intent.east() * distance;
            let mut north = self.predicted.north + intent.north() * distance;
            let delta_east = east - self.anchor.east;
            let delta_north = north - self.anchor.north;
            let length = delta_east.hypot(delta_north);
            if length > REFERENCE_MOVEMENT_ENVELOPE_METRES {
                let scale = REFERENCE_MOVEMENT_ENVELOPE_METRES / length;
                east = self.anchor.east + delta_east * scale;
                north = self.anchor.north + delta_north * scale;
            }
            self.predicted.east = east;
            self.predicted.north = north;
            self.predicted.elevation = self.anchor.elevation;
            self.predicted.orientation = heading;
            self.moving_ticks = self.moving_ticks.saturating_add(1);
        } else {
            self.moving_ticks = 0;
        }

        MovementTick {
            pose: self.predicted,
            started,
            stopped,
            heartbeat: self.moving && self.moving_ticks.is_multiple_of(HEARTBEAT_TICKS),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MovementTick {
    pub(crate) pose: WorldPose,
    pub(crate) started: bool,
    pub(crate) stopped: bool,
    pub(crate) heartbeat: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        GroundPrediction, HEARTBEAT_TICKS, PREDICTION_HZ, REFERENCE_MOVEMENT_ENVELOPE_METRES,
    };
    use crate::{MovementIntent, WorldPose};

    fn anchor() -> WorldPose {
        WorldPose {
            map_id: 0,
            east: 10.0,
            north: -4.0,
            elevation: 83.5,
            orientation: 0.0,
        }
    }

    #[test]
    fn prediction_is_heading_aligned_fixed_rate_and_preserves_anchor_height() {
        let mut prediction = GroundPrediction::new(anchor(), 4.0).unwrap();
        let intent = MovementIntent::planar(3.0, 4.0).unwrap();
        for _ in 0..PREDICTION_HZ {
            prediction.tick(intent);
        }
        let pose = prediction.predicted();
        assert!((pose.east - 12.4).abs() < 0.000_1);
        assert!((pose.north - -0.8).abs() < 0.000_1);
        assert!((pose.orientation - 3.0_f32.atan2(4.0)).abs() < 0.000_1);
        assert!((pose.elevation - anchor().elevation).abs() < f32::EPSILON);
    }

    #[test]
    fn envelope_transitions_and_heartbeats_are_deterministic() {
        let mut prediction = GroundPrediction::new(anchor(), 60.0).unwrap();
        let forward = MovementIntent::planar(1.0, 0.0).unwrap();
        let first = prediction.tick(forward);
        assert!(first.started);
        assert!(!first.heartbeat);
        for _ in 1..HEARTBEAT_TICKS - 1 {
            assert!(!prediction.tick(forward).heartbeat);
        }
        assert!(prediction.tick(forward).heartbeat);
        for _ in 0..600 {
            prediction.tick(forward);
        }
        let pose = prediction.predicted();
        assert!((pose.east - anchor().east).abs() <= REFERENCE_MOVEMENT_ENVELOPE_METRES + 0.000_1);
        assert!(
            (pose.north - anchor().north).abs() <= REFERENCE_MOVEMENT_ENVELOPE_METRES + 0.000_1
        );
        assert!(prediction.tick(MovementIntent::idle()).stopped);
    }
}
