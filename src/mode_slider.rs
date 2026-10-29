// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors

use eframe::egui;

pub(crate) const DURATION: f64 = 0.2;

#[cfg(target_os = "windows")]
fn system_reduce_motion() -> bool {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn SystemParametersInfoW(
            action: u32,
            parameter: u32,
            result: *mut std::ffi::c_void,
            flags: u32,
        ) -> i32;
    }
    // SPI_GETCLIENTAREAANIMATION reports the user's Windows UI animation preference.
    let mut animations_enabled = 1_i32;
    let query_succeeded = unsafe {
        SystemParametersInfoW(0x1042, 0, (&mut animations_enabled as *mut i32).cast(), 0) != 0
    };
    query_succeeded && animations_enabled == 0
}

#[cfg(not(target_os = "windows"))]
fn system_reduce_motion() -> bool {
    false
}

#[derive(Clone, Copy)]
struct Motion {
    from: f32,
    target: f32,
    started_at: f64,
}

impl Motion {
    fn settled(target: f32, now: f64) -> Self {
        Self {
            from: target,
            target,
            started_at: now,
        }
    }

    fn sample(self, now: f64) -> f32 {
        let elapsed = ((now - self.started_at) / DURATION).clamp(0.0, 1.0) as f32;
        // Cubic ease-out keeps the first movement responsive and lands gently.
        let eased = 1.0 - (1.0 - elapsed).powi(3);
        self.from + (self.target - self.from) * eased
    }

    fn retarget(&mut self, target: f32, now: f64) {
        if self.target != target {
            *self = Self {
                from: self.sample(now),
                target,
                started_at: now,
            };
        }
    }

    fn moving(self, now: f64) -> bool {
        self.sample(now) != self.target
    }
}

/// Slider coordinates are segment indices, so resizing cannot leave a pixel offset stale.
pub(crate) fn position(ctx: &egui::Context, target: f32, reduce_motion: bool) -> f32 {
    let id = egui::Id::new("mode-indicator-motion");
    let now = ctx.input(|input| input.time);
    let mut motion = ctx
        .data(|data| data.get_temp::<Motion>(id))
        .unwrap_or_else(|| Motion::settled(target, now));
    if reduce_motion || system_reduce_motion() {
        motion = Motion::settled(target, now);
    } else {
        motion.retarget(target, now);
    }
    let current = motion.sample(now);
    ctx.data_mut(|data| data.insert_temp(id, motion));
    if motion.moving(now) {
        ctx.request_repaint();
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_starts_at_visible_position_and_finishes_at_latest_target() {
        let mut motion = Motion::settled(0.0, 0.0);
        motion.retarget(2.0, 1.0);
        let visible = motion.sample(1.06);
        assert!(visible > 0.0 && visible < 2.0);
        motion.retarget(0.0, 1.06);
        assert!((motion.sample(1.06) - visible).abs() < f32::EPSILON);
        assert!(motion.sample(1.16) < visible);
        assert_eq!(motion.sample(1.26), 0.0);
    }

    #[test]
    fn easing_lands_at_target_in_two_hundred_milliseconds() {
        let mut motion = Motion::settled(0.0, 0.0);
        motion.retarget(1.0, 1.0);
        assert_eq!(motion.sample(1.0), 0.0);
        assert!(motion.sample(1.05) > 0.25);
        assert_eq!(motion.sample(1.2), 1.0);
        assert!(!motion.moving(1.2));
    }
}
