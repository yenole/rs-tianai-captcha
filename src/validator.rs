//! Type-specific answer comparison logic.
use crate::{CaptchaError, Result, model::UserTrack};

pub const DEFAULT_TOLERANT: f32 = 0.02;

/// Java SimpleImageCaptchaValidator.checkPercentage: inclusive symmetric range.
pub fn check_percentage(actual: f32, expected: f32, tolerant: f32) -> bool {
    actual.is_finite()
        && expected.is_finite()
        && tolerant.is_finite()
        && actual >= expected - tolerant
        && actual <= expected + tolerant
}

pub fn validate_slider_percentage(
    track: &UserTrack,
    expected: f32,
    tolerant: Option<f32>,
) -> Result<bool> {
    if track.bg_image_width == 0 {
        return Err(CaptchaError::InvalidInput(
            "bgImageWidth must be positive".into(),
        ));
    }
    let (Some(first), Some(last)) = (track.track_list.first(), track.track_list.last()) else {
        return Ok(false);
    };
    let actual = (last.x - first.x) / track.bg_image_width as f32;
    Ok(check_percentage(
        actual,
        expected,
        tolerant.unwrap_or(DEFAULT_TOLERANT),
    ))
}

/// 基础轨迹规则，对应 Java BasicCaptchaTrackValidator 的滑动检查。
pub fn validate_slider_track(track: &UserTrack) -> Result<bool> {
    let width = track.bg_image_width;
    if width == 0 {
        return Err(CaptchaError::InvalidInput(
            "bgImageWidth must be positive".into(),
        ));
    }
    if track.bg_image_height.unwrap_or(0) == 0 {
        return Err(CaptchaError::InvalidInput(
            "bgImageHeight is required".into(),
        ));
    }
    let (Some(start), Some(stop)) = (track.start_time, track.stop_time) else {
        return Err(CaptchaError::InvalidInput(
            "startTime and stopTime are required".into(),
        ));
    };
    let points = &track.track_list;
    if points.is_empty() {
        return Err(CaptchaError::InvalidInput("trackList is required".into()));
    }
    if stop < start + 300 || points.len() < 10 || points.len() > width as usize * 5 {
        return Ok(false);
    }
    let first = &points[0];
    if first.x > 10.0 || first.x < -10.0 || first.y > 10.0 || first.y < -10.0 {
        return Ok(false);
    }
    let mut same_y = 0usize;
    let mut beyond_width = 0usize;
    for pair in points.windows(2) {
        let p = &pair[1];
        if p.y == first.y {
            same_y += 1;
        }
        if p.x >= width as f32 {
            beyond_width += 1;
        }
        if p.x - pair[0].x > 50.0 || p.y - pair[0].y > 50.0 {
            return Ok(false);
        }
    }
    // Java counts only points after the initial down event for constant-y detection,
    // but compares the count to the entire list length.
    if same_y == points.len() || beyond_width > 200 {
        return Ok(false);
    }
    let split = (points.len() as f32 * 0.7) as usize;
    if split == 0 || split >= points.len() {
        return Ok(false);
    }
    let start_avg = points[split - 1].t / split as f32;
    // Keep Java's cumulative-last-time calculation (rather than time delta).
    let end_avg = points.last().unwrap().t / (points.len() - split) as f32;
    Ok(end_avg > start_avg)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn percentage_boundaries_are_inclusive() {
        assert!(check_percentage(0.52, 0.5, 0.02));
        assert!(check_percentage(0.48, 0.5, 0.02));
        assert!(!check_percentage(f32::NAN, 0.5, 0.02));
    }
}
