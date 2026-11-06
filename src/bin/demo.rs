use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::Html,
    response::IntoResponse,
    routing::{get, post},
};
use image::{ImageFormat, Rgba, RgbaImage};
use serde::Deserialize;
use std::{io::Cursor, net::SocketAddr, sync::Arc, time::Duration};
use tianai_captcha::{
    CaptchaService, CheckResult, GenerateParam, ImageResource, ResourceSet, SliderTemplate,
    UserTrack,
};

#[derive(Clone)]
struct AppState {
    captcha: CaptchaService,
}
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct GenRequest {
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    obfuscate: bool,
}
#[derive(Deserialize)]
struct MatchRequest {
    id: String,
    data: UserTrack,
}
#[derive(serde::Serialize)]
struct Api<T> {
    code: i32,
    msg: &'static str,
    data: T,
}

async fn generate(
    State(state): State<Arc<AppState>>,
    Json(request): Json<GenRequest>,
) -> impl IntoResponse {
    let kind = request.r#type.as_deref().unwrap_or("SLIDER");
    let kind = match kind.to_ascii_uppercase().as_str() {
        "SLIDER" => tianai_captcha::CaptchaType::Slider,
        "ROTATE" => tianai_captcha::CaptchaType::Rotate,
        "CONCAT" => tianai_captcha::CaptchaType::Concat,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(Api {
                    code: 400,
                    msg: "unsupported captcha type",
                    data: serde_json::Value::Null,
                }),
            )
                .into_response();
        }
    };
    match state.captcha.generate(GenerateParam {
        r#type: kind,
        obfuscate: request.obfuscate,
        ..Default::default()
    }) {
        Ok(data) => (
            StatusCode::OK,
            Json(Api {
                code: 200,
                msg: "OK",
                data,
            }),
        )
            .into_response(),
        Err(_err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(Api {
                code: 500,
                msg: "captcha generation failed",
                data: serde_json::Value::Null,
            }),
        )
            .into_response(),
    }
}
fn normalize_track_origin(mut track: UserTrack) -> UserTrack {
    if let Some(first) = track.track_list.first() {
        let (origin_x, origin_y) = (first.x, first.y);
        for point in &mut track.track_list {
            point.x -= origin_x;
            point.y -= origin_y;
        }
    }
    track
}

async fn check(
    State(state): State<Arc<AppState>>,
    Json(input): Json<MatchRequest>,
) -> impl IntoResponse {
    // The official SDK reports pageX/pageY. BasicCaptchaTrackValidator expects
    // drag-relative coordinates whose first point is near (0, 0).
    let track = normalize_track_origin(input.data);
    match state.captcha.check(&input.id, &track) {
        Ok(CheckResult::Success) => (
            StatusCode::OK,
            Json(serde_json::json!({"code":200,"msg":"OK","data":null})),
        ),
        Ok(CheckResult::Failed) => (
            StatusCode::OK,
            Json(serde_json::json!({"code":50001,"msg":"basic check fail","data":null})),
        ),
        Ok(CheckResult::Expired) => (
            StatusCode::OK,
            Json(serde_json::json!({"code":4001,"msg":"captcha expired","data":null})),
        ),
        Err(_) => (
            StatusCode::OK,
            Json(serde_json::json!({"code":400,"msg":"parameter error","data":null})),
        ),
    }
}
async fn index() -> Html<&'static str> {
    Html(include_str!("../../demo/index.html"))
}
async fn tac_js() -> ([(&'static str, &'static str); 1], &'static str) {
    (
        [("content-type", "application/javascript; charset=utf-8")],
        include_str!("../../demo/vendor/tac.min.js"),
    )
}
async fn tac_css() -> ([(&'static str, &'static str); 1], &'static str) {
    (
        [("content-type", "text/css; charset=utf-8")],
        include_str!("../../demo/vendor/tac.css"),
    )
}
async fn jquery_js() -> ([(&'static str, &'static str); 1], &'static str) {
    (
        [("content-type", "application/javascript; charset=utf-8")],
        include_str!("../../demo/vendor/jquery.min.js"),
    )
}
async fn demo_js() -> ([(&'static str, &'static str); 1], &'static str) {
    (
        [("content-type", "application/javascript; charset=utf-8")],
        include_str!("../../demo/demo.js"),
    )
}

fn encode(image: &RgbaImage, format: ImageFormat) -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image.clone())
        .write_to(&mut cursor, format)
        .expect("in-memory demo image encoding should succeed");
    cursor.into_inner()
}

/// Small synthetic assets let the demo run out-of-the-box. Supply real licensed assets
/// in CAPTCHA_RESOURCE_DIR to replace these placeholders.
fn builtin_resources() -> ResourceSet {
    let bg = RgbaImage::from_fn(320, 180, |x, y| {
        let tile = ((x / 24 + y / 24) % 2) as u8;
        let v: u8 = if tile == 0 { 205 } else { 225 };
        Rgba([
            v.saturating_sub((x % 60) as u8),
            v,
            v.saturating_add(10),
            255,
        ])
    });
    let mut fixed = RgbaImage::new(112, 112);
    let mut active = RgbaImage::new(112, 112);
    for y in 0..112 {
        for x in 0..112 {
            let dx = x as i32 - 56;
            let dy = y as i32 - 56;
            let distance = dx * dx + dy * dy;
            let inside = distance <= 54 * 54;
            // Large, thick red ring; its transparent center shows the sampled image.
            let ring = inside && distance >= 38 * 38;
            if inside {
                fixed.put_pixel(x, y, Rgba([80, 80, 80, 255]));
            }
            if ring {
                active.put_pixel(x, y, Rgba([245, 80, 70, 235]));
            }
        }
    }
    let bg_bytes = encode(&bg, ImageFormat::Jpeg);
    let rotate_fixed = include_bytes!("../../demo/resources/rotate/fixed.png").to_vec();
    let rotate = SliderTemplate {
        tag: "builtin".into(),
        active: include_bytes!("../../demo/resources/rotate/active.png").to_vec(),
        fixed: rotate_fixed.clone(),
        mask: Some(include_bytes!("../../demo/resources/rotate/mask.png").to_vec()),
    };
    let animal_templates = vec![
        SliderTemplate {
            tag: "bunny".into(),
            active: include_bytes!("../../demo/resources/slider/bunny/active.png").to_vec(),
            fixed: include_bytes!("../../demo/resources/slider/bunny/fixed.png").to_vec(),
            mask: Some(include_bytes!("../../demo/resources/slider/bunny/mask.png").to_vec()),
        },
        SliderTemplate {
            tag: "fox".into(),
            active: include_bytes!("../../demo/resources/slider/fox/active.png").to_vec(),
            fixed: include_bytes!("../../demo/resources/slider/fox/fixed.png").to_vec(),
            mask: Some(include_bytes!("../../demo/resources/slider/fox/mask.png").to_vec()),
        },
    ];
    let slider_templates = animal_templates;
    ResourceSet {
        backgrounds: vec![ImageResource {
            tag: "builtin".into(),
            bytes: bg_bytes,
        }],
        slider_templates,
        rotate_templates: vec![rotate],
    }
}

fn is_image_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "webp" | "bmp"
            )
        })
}

fn load_backgrounds(root: &str) -> Vec<ImageResource> {
    let root = std::path::Path::new(root);
    let mut paths = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_image_file(&path) {
                paths.push(path);
            } else if path.is_dir()
                && path.file_name().and_then(|n| n.to_str()) == Some("backgrounds")
                && let Ok(files) = std::fs::read_dir(path)
            {
                paths.extend(
                    files
                        .flatten()
                        .map(|file| file.path())
                        .filter(|p| p.is_file() && is_image_file(p)),
                );
            }
        }
    }
    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| {
            let bytes = std::fs::read(&path).ok()?;
            let image = image::load_from_memory(&bytes).ok()?;
            if image.width() < 239 || image.height() < 128 {
                eprintln!(
                    "Skipping small background image {} ({}x{})",
                    path.display(),
                    image.width(),
                    image.height()
                );
                return None;
            }
            let tag = path.file_stem()?.to_string_lossy().into_owned();
            Some(ImageResource { tag, bytes })
        })
        .collect()
}

fn load_slider_templates(root: &str) -> Vec<SliderTemplate> {
    let slider_root = std::path::Path::new(root).join("slider");
    let mut templates = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&slider_root) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            if let (Ok(active), Ok(fixed)) = (
                std::fs::read(dir.join("active.png")),
                std::fs::read(dir.join("fixed.png")),
            ) {
                let tag = dir
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                templates.push(SliderTemplate {
                    tag,
                    active,
                    fixed,
                    mask: std::fs::read(dir.join("mask.png")).ok(),
                });
            }
        }
    }
    if let (Ok(active), Ok(fixed)) = (
        std::fs::read(slider_root.join("active.png")),
        std::fs::read(slider_root.join("fixed.png")),
    ) {
        templates.push(SliderTemplate {
            tag: "default".into(),
            active,
            fixed,
            mask: std::fs::read(slider_root.join("mask.png")).ok(),
        });
    }
    templates
}

fn load_rotate_templates(root: &str, fallback: &[SliderTemplate]) -> Vec<SliderTemplate> {
    let dir = std::path::Path::new(root).join("rotate");
    match (
        std::fs::read(dir.join("active.png")),
        std::fs::read(dir.join("fixed.png")),
    ) {
        (Ok(active), Ok(fixed)) => vec![SliderTemplate {
            tag: "rotate".into(),
            active,
            fixed,
            mask: std::fs::read(dir.join("mask.png")).ok(),
        }],
        _ => fallback.to_vec(),
    }
}

fn load_resources() -> ResourceSet {
    let root = std::env::var("CAPTCHA_RESOURCE_DIR").unwrap_or_else(|_| {
        format!(
            "{}/coding/rs-tianai-captcha/demo/resources",
            std::env::var("HOME").unwrap_or_default()
        )
    });
    let mut backgrounds = load_backgrounds(&root);
    let user_default = std::env::var("CAPTCHA_BACKGROUND_IMAGE").unwrap_or_else(|_| {
        format!(
            "{}/download/1.jpg",
            std::env::var("HOME").unwrap_or_default()
        )
    });
    if let Ok(bytes) = std::fs::read(&user_default)
        && image::load_from_memory(&bytes)
            .is_ok_and(|img| img.width() >= 239 && img.height() >= 128)
    {
        backgrounds.push(ImageResource {
            tag: "user-default".into(),
            bytes,
        });
    }

    let mut slider_templates = load_slider_templates(&root);
    let rotate_templates = load_rotate_templates(&root, &slider_templates);
    if backgrounds.is_empty() || slider_templates.is_empty() {
        eprintln!("Resources under {root} are incomplete; adding built-in fallbacks");
        let fallback = builtin_resources();
        if backgrounds.is_empty() {
            backgrounds = fallback.backgrounds;
        }
        if slider_templates.is_empty() {
            slider_templates = fallback.slider_templates;
        }
        return ResourceSet {
            backgrounds,
            slider_templates,
            rotate_templates: fallback.rotate_templates,
        };
    }
    eprintln!(
        "Loaded {} backgrounds and {} slider templates from {root}; each captcha selects randomly",
        backgrounds.len(),
        slider_templates.len()
    );
    ResourceSet {
        backgrounds,
        slider_templates,
        rotate_templates,
    }
}

#[tokio::main]
async fn main() {
    let state = Arc::new(AppState {
        captcha: CaptchaService::new(load_resources())
            .with_ttl(Duration::from_secs(120))
            // Enable BasicCaptchaTrackValidator-style curve heuristics after converting
            // SDK pageX/pageY points to drag-relative coordinates in /check.
            .with_behavior_check(true),
    });
    let app = Router::new()
        .route("/", get(index))
        .route("/gen", post(generate))
        .route("/valid", post(check))
        .route("/check", post(check))
        .route("/static/tac.min.js", get(tac_js))
        .route("/static/tac.css", get(tac_css))
        .route("/static/jquery.min.js", get(jquery_js))
        .route("/static/demo.js", get(demo_js))
        .with_state(state);
    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    println!("Rust tianai-captcha demo: http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod track_tests {
    use super::*;
    use tianai_captcha::TrackPoint;

    #[test]
    fn sdk_page_coordinates_are_normalized_to_drag_origin() {
        let track = UserTrack {
            bg_image_width: 320,
            bg_image_height: Some(180),
            template_image_width: None,
            template_image_height: None,
            start_time: Some(1),
            stop_time: Some(500),
            left: None,
            top: None,
            track_list: vec![
                TrackPoint {
                    x: 725.0,
                    y: 410.0,
                    t: 0.0,
                    r#type: "down".into(),
                },
                TrackPoint {
                    x: 736.0,
                    y: 414.0,
                    t: 100.0,
                    r#type: "move".into(),
                },
            ],
            data: None,
        };
        let normalized = normalize_track_origin(track);
        assert_eq!(
            (normalized.track_list[0].x, normalized.track_list[0].y),
            (0.0, 0.0)
        );
        assert_eq!(
            (normalized.track_list[1].x, normalized.track_list[1].y),
            (11.0, 4.0)
        );
    }

    #[test]
    fn discovers_all_packaged_background_images() {
        let backgrounds = load_backgrounds("demo/resources");
        assert!(
            backgrounds.len() >= 3,
            "expected multiple packaged backgrounds"
        );
        assert!(backgrounds.iter().any(|bg| bg.tag == "634-300x175"));
    }
}
