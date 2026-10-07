//! Volume / mute / seek helpers (pure, headless-testable).

use std::time::Duration;

/// Volume in `0.0..=1.0` with mute memory.
#[derive(Debug, Clone, Copy)]
pub struct VolumeControl {
    volume: f32,
    last_unmuted: f32,
    muted: bool,
}

impl VolumeControl {
    pub fn new(volume: f32) -> Self {
        let v = Self::clamp(volume);
        Self {
            volume: v,
            last_unmuted: if v > 0.0 { v } else { 1.0 },
            muted: false,
        }
    }

    /// Clamp a raw volume into `0.0..=1.0`.
    pub fn clamp(v: f32) -> f32 {
        if v.is_nan() {
            return 0.0;
        }
        v.clamp(0.0, 1.0)
    }

    pub fn set(&mut self, v: f32) {
        let v = Self::clamp(v);
        self.volume = v;
        if v > 0.0 {
            self.last_unmuted = v;
            self.muted = false;
        }
    }

    pub fn volume(&self) -> f32 {
        if self.muted { 0.0 } else { self.volume }
    }

    pub fn stored_volume(&self) -> f32 {
        self.volume
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    pub fn toggle_mute(&mut self) {
        if self.muted {
            self.muted = false;
            self.volume = self.last_unmuted;
        } else {
            if self.volume > 0.0 {
                self.last_unmuted = self.volume;
            }
            self.muted = true;
        }
    }
}

/// Compute a forward seek target, saturating at `total` when known.
///
/// mpv IPC seeking lands in Agent 2; this pure computation is used for
/// UI position display.
pub fn seek_forward_target(
    current: Duration,
    delta: Duration,
    total: Option<Duration>,
) -> Duration {
    let target = current.saturating_add(delta);
    match total {
        Some(t) => target.min(t),
        None => target,
    }
}

/// Compute a backward seek target, saturating at zero.
///
/// Same fallback note as [`seek_forward_target`].
pub fn seek_backward_target(current: Duration, delta: Duration) -> Duration {
    current.checked_sub(delta).unwrap_or(Duration::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_bounds_volume() {
        assert_eq!(VolumeControl::clamp(-1.0), 0.0);
        assert_eq!(VolumeControl::clamp(2.0), 1.0);
        assert_eq!(VolumeControl::clamp(0.5), 0.5);
        assert_eq!(VolumeControl::clamp(f32::NAN), 0.0);
    }

    #[test]
    fn mute_stores_last_volume() {
        let mut v = VolumeControl::new(0.7);
        v.toggle_mute();
        assert!(v.is_muted());
        assert_eq!(v.volume(), 0.0);
        v.toggle_mute();
        assert!(!v.is_muted());
        assert!((v.volume() - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn set_zero_keeps_restore_level() {
        let mut v = VolumeControl::new(0.6);
        v.toggle_mute();
        v.toggle_mute();
        assert!((v.volume() - 0.6).abs() < f32::EPSILON);
        v.set(0.0);
        assert_eq!(v.volume(), 0.0);
    }

    #[test]
    fn seek_targets_saturate() {
        let cur = Duration::from_secs(10);
        assert_eq!(
            seek_forward_target(cur, Duration::from_secs(5), Some(Duration::from_secs(12))),
            Duration::from_secs(12)
        );
        assert_eq!(
            seek_forward_target(cur, Duration::from_secs(5), None),
            Duration::from_secs(15)
        );
        assert_eq!(
            seek_backward_target(cur, Duration::from_secs(20)),
            Duration::ZERO
        );
        assert_eq!(
            seek_backward_target(cur, Duration::from_secs(4)),
            Duration::from_secs(6)
        );
    }
}
