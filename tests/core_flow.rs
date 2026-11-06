use image::{ImageFormat, Rgba, RgbaImage};
use std::{io::Cursor, time::Duration};
use tianai_captcha::{
    CaptchaService, CaptchaType, CheckResult, GenerateParam, ImageResource, ResourceSet,
    SliderTemplate, UserTrack,
};

fn png(img: &RgbaImage) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img.clone())
        .write_to(&mut out, ImageFormat::Png)
        .unwrap();
    out.into_inner()
}
fn resources() -> ResourceSet {
    let bg = RgbaImage::from_fn(160, 100, |x, y| {
        Rgba([(x % 255) as u8, (y % 255) as u8, 120, 255])
    });
    let mask = RgbaImage::from_fn(24, 24, |x, y| {
        if (x as i32 - 12).pow(2) + (y as i32 - 12).pow(2) < 110 {
            Rgba([0, 0, 0, 255])
        } else {
            Rgba([0, 0, 0, 0])
        }
    });
    let active = RgbaImage::from_fn(24, 24, |x, y| {
        let p = mask.get_pixel(x, y);
        if p[3] > 0 {
            Rgba([240, 240, 240, 255])
        } else {
            Rgba([0, 0, 0, 0])
        }
    });
    ResourceSet {
        backgrounds: vec![ImageResource {
            tag: "default".into(),
            bytes: png(&bg),
        }],
        slider_templates: vec![SliderTemplate {
            tag: "default".into(),
            active: png(&active),
            fixed: png(&mask),
            mask: None,
        }],
        rotate_templates: vec![SliderTemplate {
            tag: "default".into(),
            active: png(&active),
            fixed: png(&mask),
            mask: None,
        }],
    }
}

#[test]
fn slider_response_is_compatible_and_check_consumes_ticket() {
    let app = CaptchaService::new(resources()).with_ttl(Duration::from_secs(2));
    let response = app.generate(GenerateParam::default()).unwrap();
    assert_eq!(response.r#type, "SLIDER");
    assert!(
        response
            .background_image
            .as_ref()
            .unwrap()
            .starts_with("data:image/jpeg;base64,")
    );
    assert!(
        response
            .template_image
            .as_ref()
            .unwrap()
            .starts_with("data:image/png;base64,")
    );
    let json = serde_json::to_value(&response).unwrap();
    assert!(json.get("backgroundImageWidth").is_some());
    let track = UserTrack {
        bg_image_width: 160,
        bg_image_height: Some(100),
        template_image_width: None,
        template_image_height: None,
        start_time: Some(0),
        stop_time: Some(400),
        left: None,
        top: None,
        track_list: vec![
            tianai_captcha::TrackPoint {
                x: 0.0,
                y: 0.0,
                t: 0.0,
                r#type: "down".into(),
            },
            tianai_captcha::TrackPoint {
                x: 5.0,
                y: 1.0,
                t: 40.0,
                r#type: "move".into(),
            },
            tianai_captcha::TrackPoint {
                x: 10.0,
                y: 2.0,
                t: 80.0,
                r#type: "move".into(),
            },
            tianai_captcha::TrackPoint {
                x: 15.0,
                y: 1.0,
                t: 120.0,
                r#type: "move".into(),
            },
            tianai_captcha::TrackPoint {
                x: 20.0,
                y: 3.0,
                t: 160.0,
                r#type: "move".into(),
            },
            tianai_captcha::TrackPoint {
                x: 25.0,
                y: 2.0,
                t: 200.0,
                r#type: "move".into(),
            },
            tianai_captcha::TrackPoint {
                x: 30.0,
                y: 4.0,
                t: 240.0,
                r#type: "move".into(),
            },
            tianai_captcha::TrackPoint {
                x: 35.0,
                y: 3.0,
                t: 280.0,
                r#type: "move".into(),
            },
            tianai_captcha::TrackPoint {
                x: 40.0,
                y: 4.0,
                t: 320.0,
                r#type: "move".into(),
            },
            tianai_captcha::TrackPoint {
                x: 45.0,
                y: 5.0,
                t: 400.0,
                r#type: "up".into(),
            },
        ],
        data: None,
    };
    let mut wrong_track = track.clone();
    // A stationary but otherwise human-like track cannot match the positive slider target.
    for point in &mut wrong_track.track_list {
        point.x = 0.0;
    }
    assert!(matches!(
        app.check(&response.id, &wrong_track).unwrap(),
        CheckResult::Failed
    ));
    assert!(matches!(
        app.check(&response.id, &track).unwrap(),
        CheckResult::Expired
    ));
}

#[test]
fn rotate_and_concat_generate_with_compatible_response_fields() {
    let app = CaptchaService::new(resources());
    for kind in [CaptchaType::Rotate, CaptchaType::Concat] {
        let response = app
            .generate(GenerateParam {
                r#type: kind,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(response.r#type, kind.as_str());
        assert!(
            response
                .background_image
                .as_ref()
                .unwrap()
                .starts_with("data:image/jpeg;base64,")
        );
        assert_eq!(response.background_image_width, Some(160));
        if kind == CaptchaType::Rotate {
            assert!(response.template_image.is_some());
        } else {
            assert!(response.template_image.is_none());
        }
    }
}
