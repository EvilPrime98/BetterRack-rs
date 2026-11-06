use std::time::Duration;

use gpui::Animation;

pub const FAST: Duration = Duration::from_millis(120);
pub const BASE: Duration = Duration::from_millis(200);
pub const SLOW: Duration = Duration::from_millis(300);

pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

pub fn once(duration: Duration) -> Animation {
    Animation::new(duration).with_easing(ease_out_cubic)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_hits_endpoints_and_is_monotonic() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        assert!(ease_out_cubic(0.5) > 0.5);
    }
}
