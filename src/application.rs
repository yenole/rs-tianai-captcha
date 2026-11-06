use crate::{
    cache::MemoryCache,
    generator::{CaptchaResponse, GenerateParam, ResourceSet, generate},
    model::UserTrack,
    validator,
};
use rand::RngExt;
use std::time::Duration;

#[derive(Debug)]
pub enum CheckResult {
    Success,
    Failed,
    Expired,
}

#[derive(Clone)]
pub struct CaptchaService {
    resources: ResourceSet,
    cache: MemoryCache,
    ttl: Duration,
    behavior_check: bool,
}
impl CaptchaService {
    pub fn new(resources: ResourceSet) -> Self {
        Self {
            resources,
            cache: MemoryCache::default(),
            ttl: Duration::from_secs(20),
            behavior_check: false,
        }
    }
    pub fn from_shared(resources: ResourceSet, cache: MemoryCache, ttl: Duration) -> Self {
        Self {
            resources,
            cache,
            ttl,
            behavior_check: false,
        }
    }
    /// Enable TAC-style heuristic track checks. Off by default to match Java's
    /// `SimpleImageCaptchaValidator`; opt in to `BasicCaptchaTrackValidator` rules.
    pub fn with_behavior_check(mut self, enabled: bool) -> Self {
        self.behavior_check = enabled;
        self
    }
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }
    pub fn generate(&self, param: GenerateParam) -> crate::Result<CaptchaResponse> {
        let id = format!(
            "{}_{:032x}",
            param.r#type.as_str(),
            rand::rng().random::<u128>()
        );
        let generated = generate(&id, &param, &self.resources)?;
        self.cache.put(id.clone(), generated.answer, self.ttl);
        Ok(generated.response)
    }
    /// Consumes the captcha before checking it, including for malformed or incorrect submissions.
    pub fn check(&self, id: &str, track: &UserTrack) -> crate::Result<CheckResult> {
        let Some(answer) = self.cache.take(id) else {
            return Ok(CheckResult::Expired);
        };
        let kind = id.split('_').next().unwrap_or("SLIDER");
        let expected = answer
            .get("percentage")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0) as f32;
        let valid = match kind {
            "SLIDER" | "ROTATE" | "CONCAT" => {
                let tolerant = answer
                    .get("tolerant")
                    .and_then(serde_json::Value::as_f64)
                    .map(|n| n as f32)
                    .unwrap_or(validator::DEFAULT_TOLERANT);
                let mut normalized = track.clone();
                let travel_width = normalized.bg_image_width;
                if kind == "ROTATE" {
                    // tac.min.js replaces bgImageWidth with the slider travel range
                    // (`end`, about 242px), while Java validates against image width.
                    let image_width = answer
                        .get("backgroundWidth")
                        .and_then(serde_json::Value::as_u64)
                        .map(|w| w as f32)
                        .unwrap_or(0.0);
                    if image_width > 0.0 && travel_width > 0 {
                        let scale = image_width / travel_width as f32;
                        for point in &mut normalized.track_list {
                            point.x *= scale;
                        }
                        normalized.bg_image_width = image_width.round() as u32;
                    }
                }
                (!self.behavior_check || validator::validate_slider_track(&normalized)?)
                    && validator::validate_slider_percentage(&normalized, expected, Some(tolerant))?
            }
            _ => false,
        };
        Ok(if valid {
            CheckResult::Success
        } else {
            CheckResult::Failed
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TrackPoint;
    use serde_json::json;

    fn realistic_track(width: u32, distance: f32) -> UserTrack {
        let points = (0..12)
            .map(|i| TrackPoint {
                x: distance * i as f32 / 11.0,
                y: (i % 3) as f32,
                t: if i < 8 {
                    i as f32 * 20.0
                } else {
                    160.0 + (i - 7) as f32 * 70.0
                },
                r#type: if i == 0 {
                    "down"
                } else if i == 11 {
                    "up"
                } else {
                    "move"
                }
                .into(),
            })
            .collect();
        UserTrack {
            bg_image_width: width,
            bg_image_height: Some(100),
            template_image_width: Some(20),
            template_image_height: Some(20),
            start_time: Some(0),
            stop_time: Some(650),
            left: None,
            top: None,
            track_list: points,
            data: None,
        }
    }

    #[test]
    fn slider_correct_answer_passes_and_consumes_token() {
        let app = CaptchaService::new(ResourceSet::default());
        let id = "SLIDER_test";
        app.cache.put(
            id.into(),
            json!({"percentage":0.25}),
            Duration::from_secs(10),
        );
        let track = realistic_track(160, 40.0);
        assert!(matches!(
            app.check(id, &track).unwrap(),
            CheckResult::Success
        ));
        assert!(matches!(
            app.check(id, &track).unwrap(),
            CheckResult::Expired
        ));
    }

    #[test]
    fn incorrect_slider_answer_fails_even_with_valid_human_track() {
        let app = CaptchaService::new(ResourceSet::default());
        let id = "SLIDER_wrong";
        app.cache.put(
            id.into(),
            json!({"percentage":0.5}),
            Duration::from_secs(10),
        );
        assert!(matches!(
            app.check(id, &realistic_track(160, 40.0)).unwrap(),
            CheckResult::Failed
        ));
    }
}
