use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CaptchaType {
    Slider,
    Rotate,
    Concat,
}
impl CaptchaType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Slider => "SLIDER",
            Self::Rotate => "ROTATE",
            Self::Concat => "CONCAT",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackPoint {
    pub x: f32,
    pub y: f32,
    pub t: f32,
    #[serde(default = "default_move")]
    pub r#type: String,
}
fn default_move() -> String {
    "MOVE".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserTrack {
    pub bg_image_width: u32,
    #[serde(default)]
    pub bg_image_height: Option<u32>,
    #[serde(default)]
    pub template_image_width: Option<u32>,
    #[serde(default)]
    pub template_image_height: Option<u32>,
    #[serde(default)]
    pub start_time: Option<i64>,
    #[serde(default)]
    pub stop_time: Option<i64>,
    #[serde(default)]
    pub left: Option<i32>,
    #[serde(default)]
    pub top: Option<i32>,
    pub track_list: Vec<TrackPoint>,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}
