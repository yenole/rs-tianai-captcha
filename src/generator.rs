use crate::{
    CaptchaError, Result,
    image_ops::{cut_by_mask, encode_data_uri, overlay},
    model::CaptchaType,
};
use image::RgbaImage;
use rand::RngExt;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct ImageResource {
    pub tag: String,
    pub bytes: Vec<u8>,
}
#[derive(Clone)]
pub struct SliderTemplate {
    pub tag: String,
    pub active: Vec<u8>,
    pub fixed: Vec<u8>,
    pub mask: Option<Vec<u8>>,
}
#[derive(Default, Clone)]
pub struct ResourceSet {
    pub backgrounds: Vec<ImageResource>,
    pub slider_templates: Vec<SliderTemplate>,
    pub rotate_templates: Vec<SliderTemplate>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptchaResponse {
    pub id: String,
    pub r#type: String,
    pub background_image: Option<String>,
    pub template_image: Option<String>,
    pub background_image_tag: Option<String>,
    pub template_image_tag: Option<String>,
    pub background_image_width: Option<u32>,
    pub background_image_height: Option<u32>,
    pub template_image_width: Option<u32>,
    pub template_image_height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}
#[derive(Debug, Clone)]
pub struct Generated {
    pub response: CaptchaResponse,
    pub answer: serde_json::Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateParam {
    pub r#type: CaptchaType,
    #[serde(default = "default_bg")]
    pub background_format_name: String,
    #[serde(default = "default_tpl")]
    pub template_format_name: String,
    #[serde(default)]
    pub obfuscate: bool,
}
fn default_bg() -> String {
    "jpeg".into()
}
fn default_tpl() -> String {
    "png".into()
}
impl Default for GenerateParam {
    fn default() -> Self {
        Self {
            r#type: CaptchaType::Slider,
            background_format_name: default_bg(),
            template_format_name: default_tpl(),
            obfuscate: false,
        }
    }
}

fn choose<T>(items: &[T]) -> Result<&T> {
    if items.is_empty() {
        return Err(CaptchaError::InvalidInput(
            "required image resource list is empty".into(),
        ));
    }
    Ok(&items[rand::rng().random_range(0..items.len())])
}
fn encode_image(img: &RgbaImage, fmt: &str) -> Result<String> {
    encode_data_uri(img, fmt)
}
#[allow(clippy::too_many_arguments)]
fn response(
    id: &str,
    kind: CaptchaType,
    bg: RgbaImage,
    tpl: Option<RgbaImage>,
    bg_tag: Option<String>,
    tpl_tag: Option<String>,
    bg_fmt: &str,
    tpl_fmt: &str,
    data: Option<serde_json::Value>,
) -> Result<CaptchaResponse> {
    let (tw, th, turl) = if let Some(t) = tpl.as_ref() {
        (
            Some(t.width()),
            Some(t.height()),
            Some(encode_image(t, tpl_fmt)?),
        )
    } else {
        (None, None, None)
    };
    Ok(CaptchaResponse {
        id: id.into(),
        r#type: kind.as_str().into(),
        background_image: Some(encode_image(&bg, bg_fmt)?),
        template_image: turl,
        background_image_tag: bg_tag,
        template_image_tag: tpl_tag,
        background_image_width: Some(bg.width()),
        background_image_height: Some(bg.height()),
        template_image_width: tw,
        template_image_height: th,
        data,
    })
}

pub fn generate(id: &str, param: &GenerateParam, resources: &ResourceSet) -> Result<Generated> {
    match param.r#type {
        CaptchaType::Slider => gen_slider(id, param, resources, false),
        CaptchaType::Rotate => gen_slider(id, param, resources, true),
        CaptchaType::Concat => gen_concat(id, param, resources),
    }
}
fn make_obfuscate(fixed: &RgbaImage) -> RgbaImage {
    let scale_delta = rand::rng().random_range(-15i32..=15);
    let (w, h) = if rand::rng().random_bool(0.5) {
        (
            (fixed.width() as i32 + scale_delta).max(1) as u32,
            fixed.height(),
        )
    } else {
        (
            fixed.width(),
            (fixed.height() as i32 + scale_delta).max(1) as u32,
        )
    };
    let alpha = rand::rng().random_range(128u16..=204u16) as u8;
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let sx = (x as f32 * fixed.width() as f32 / w as f32)
                .floor()
                .min((fixed.width() - 1) as f32) as u32;
            let sy = (y as f32 * fixed.height() as f32 / h as f32)
                .floor()
                .min((fixed.height() - 1) as f32) as u32;
            let mut pixel = *fixed.get_pixel(sx, sy);
            pixel[3] = ((pixel[3] as u16 * alpha as u16) / 255) as u8;
            out.put_pixel(x, y, pixel);
        }
    }
    out
}

fn gen_slider(id: &str, p: &GenerateParam, r: &ResourceSet, rotate: bool) -> Result<Generated> {
    let bg_res = choose(&r.backgrounds)?;
    let source = crate::image_ops::decode(&bg_res.bytes)?;
    let tm = choose(if rotate {
        &r.rotate_templates
    } else {
        &r.slider_templates
    })?;
    let fixed = crate::image_ops::decode(&tm.fixed)?;
    let active = crate::image_ops::decode(&tm.active)?;
    let mask = if let Some(b) = &tm.mask {
        crate::image_ops::decode(b)?
    } else {
        fixed.clone()
    };
    if fixed.width().saturating_mul(2).saturating_add(15) >= source.width()
        || fixed.height() > source.height()
    {
        return Err(CaptchaError::InvalidInput(
            "template does not fit background".into(),
        ));
    }
    let mut bg = source.clone();
    let (x, y) = if rotate {
        (
            source.width() / 2 - fixed.width() / 2,
            source.height() / 2 - fixed.height() / 2,
        )
    } else {
        (
            rand::rng().random_range(fixed.width() + 5..source.width() - fixed.width() - 10),
            rand::rng().random_range(0..=source.height() - fixed.height()),
        )
    };
    let mut cut = cut_by_mask(&source, &mask, x, y)?;
    let (answer, strip) = if rotate {
        let random_x = rand::rng().random_range(fixed.width() + 10..source.width() - 10);
        let degree = 360.0 - random_x as f64 / (source.width() as f64 / 360.0);
        let fixed_rotated = if p.obfuscate {
            crate::image_ops::rotate_same_size(&fixed, rand::rng().random_range(10..350) as f64)
        } else {
            fixed.clone()
        };
        let active_rotated = if p.obfuscate {
            crate::image_ops::rotate_same_size(&active, rand::rng().random_range(10..350) as f64)
        } else {
            active.clone()
        };
        overlay(&mut bg, &fixed_rotated, x as i32, y as i32);
        overlay(&mut cut, &active_rotated, 0, 0);
        let mut strip = RgbaImage::new(cut.width(), source.height());
        let rotated = crate::image_ops::rotate_same_size(&cut, degree);
        overlay(
            &mut strip,
            &rotated,
            0,
            (source.height() as i32 - cut.height() as i32) / 2,
        );
        // Java sets degree = 360 - randomX * 360 / bgWidth. The SDK rotates
        // positively as the user drags, so the required drag fraction is
        // degree / 360 = 1 - randomX / bgWidth (not randomX / bgWidth).
        let expected_slider_percent = 1.0 - random_x as f32 / source.width() as f32;
        (
            serde_json::json!({"percentage":expected_slider_percent,"degree":degree,"tolerant":0.03,"backgroundWidth":source.width()}),
            strip,
        )
    } else {
        overlay(&mut bg, &fixed, x as i32, y as i32);
        if p.obfuscate {
            let fake = make_obfuscate(&fixed);
            let fake_w = fake.width();
            let (min_x, max_x) = if x + fixed.width() / 2 < source.width() / 2 {
                (x + fixed.width(), source.width().saturating_sub(fake_w))
            } else {
                (fixed.width(), x.saturating_sub(fixed.width()))
            };
            if max_x > min_x {
                let fake_x = rand::rng().random_range(min_x..max_x);
                overlay(&mut bg, &fake, fake_x as i32, y as i32);
            }
        }
        overlay(&mut cut, &active, 0, 0);
        let mut strip = RgbaImage::new(cut.width(), source.height());
        overlay(&mut strip, &cut, 0, y as i32);
        (
            serde_json::json!({"percentage":x as f32/source.width() as f32,"tolerant":0.02}),
            strip,
        )
    };
    let result = response(
        id,
        if rotate {
            CaptchaType::Rotate
        } else {
            CaptchaType::Slider
        },
        bg,
        Some(strip),
        Some(bg_res.tag.clone()),
        Some(tm.tag.clone()),
        &p.background_format_name,
        &p.template_format_name,
        None,
    )?;
    Ok(Generated {
        response: result,
        answer,
    })
}
fn concat_reorder(src: &RgbaImage, x: u32, y: u32) -> RgbaImage {
    // Mirror Java exactly:
    // splitImage(y, horizontal=true) => top H-y and bottom y;
    // splitImage(x, horizontal=false) on top => left x, right W-x;
    // concatImage(horizontal=true, right, left), then concatImage(vertical=true, top, bottom).
    let width = src.width();
    let height = src.height();
    assert!(x > 0 && x < width && y > 0 && y < height);
    let top_height = height - y;
    let mut out = RgbaImage::new(width, height);

    for row in 0..top_height {
        // Java concatenates the right-hand piece first, followed by the left.
        for col in 0..(width - x) {
            out.put_pixel(col, row, *src.get_pixel(x + col, row));
        }
        for col in 0..x {
            out.put_pixel(width - x + col, row, *src.get_pixel(col, row));
        }
    }
    // Append the original bottom strip unchanged.
    for row in 0..y {
        for col in 0..width {
            out.put_pixel(col, top_height + row, *src.get_pixel(col, top_height + row));
        }
    }
    out
}

fn add_concat_waveform_seam(image: &mut RgbaImage, seam_y: u32, accent: [u8; 3]) {
    // The meaningful join is horizontal: the reordered top section is
    // concatenated above the original bottom strip. Cover it edge-to-edge with
    // a thin, dense waveform so the seam doesn't read as a straight cut.
    let width = image.width();
    let seam_y = seam_y.min(image.height().saturating_sub(1));
    for x in 0..width {
        // Higher base frequency gives a shorter wavelength (about 4px per cycle).
        let phase = x as f32 * 1.5;
        let offset = 2.7 * phase.sin()
            + 1.25 * (phase * 2.15 + 0.6).sin()
            + 0.55 * (phase * 3.2 + 1.3).sin();
        let wave_y = seam_y as f32 + offset;
        for dy in -10..=10 {
            let py = wave_y.round() as i32 + dy;
            if py < 0 || py >= image.height() as i32 {
                continue;
            }
            let dist = (py as f32 - wave_y).abs();
            let lighten = |amount: f32| -> [u8; 3] {
                std::array::from_fn(|c| {
                    (accent[c] as f32 + (255.0 - accent[c] as f32) * amount).round() as u8
                })
            };
            let darken = |amount: f32| -> [u8; 3] {
                std::array::from_fn(|c| (accent[c] as f32 * (1.0 - amount)).round() as u8)
            };
            let (rgb, alpha) = if dist <= 1.0 {
                (lighten(0.68), 0.92)
            } else if dist <= 3.0 {
                (accent, 0.88)
            } else if dist <= 5.5 {
                (darken(0.68), 0.48)
            } else if dist <= 9.0 {
                (darken(0.68), 0.18)
            } else {
                continue;
            };
            let src = *image.get_pixel(x, py as u32);
            let mut out = [0u8; 4];
            for c in 0..3 {
                out[c] = ((src[c] as f32 * (1.0 - alpha)) + rgb[c] as f32 * alpha).round() as u8;
            }
            out[3] = src[3];
            image.put_pixel(x, py as u32, image::Rgba(out));
        }
    }
}

fn gen_concat(id: &str, p: &GenerateParam, r: &ResourceSet) -> Result<Generated> {
    let src_res = choose(&r.backgrounds)?;
    let src = crate::image_ops::decode(&src_res.bytes)?;
    if src.width() < 16 || src.height() < 4 {
        return Err(CaptchaError::InvalidInput(
            "background too small for CONCAT".into(),
        ));
    }
    let y = rand::rng().random_range(src.height() / 4..src.height() - src.height() / 4);
    let x = rand::rng().random_range(src.width() / 8..src.width() - src.width() / 5);
    // TAC swaps the top section's two horizontal pieces, then appends the bottom.
    let mut out = concat_reorder(&src, x, y);
    if p.obfuscate {
        let accent_pixel = src.get_pixel(
            rand::rng().random_range(0..src.width()),
            rand::rng().random_range(0..src.height()),
        );
        add_concat_waveform_seam(
            &mut out,
            src.height() - y,
            [accent_pixel[0], accent_pixel[1], accent_pixel[2]],
        );
    }
    let answer = serde_json::json!({"percentage":x as f32/src.width() as f32,"y":y,"tolerant":0.05,"backgroundWidth":src.width()});
    let resp = response(
        id,
        CaptchaType::Concat,
        out,
        None,
        Some(src_res.tag.clone()),
        None,
        &p.background_format_name,
        &p.template_format_name,
        Some(serde_json::json!({"randomY":y})),
    )?;
    Ok(Generated {
        response: resp,
        answer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concat_swaps_top_left_and_right_and_keeps_bottom() {
        let src = RgbaImage::from_fn(6, 4, |x, y| image::Rgba([x as u8, y as u8, 0, 255]));
        let out = concat_reorder(&src, 2, 1);
        // Top is height 3; Java places right [2..6) then left [0..2).
        assert_eq!(out.get_pixel(0, 0)[0], 2);
        assert_eq!(out.get_pixel(3, 0)[0], 5);
        assert_eq!(out.get_pixel(4, 0)[0], 0);
        assert_eq!(out.get_pixel(5, 0)[0], 1);
        // The bottom strip begins at the split and is appended unchanged.
        assert_eq!(out.get_pixel(0, 3), src.get_pixel(0, 3));
        assert_eq!(out.get_pixel(5, 3), src.get_pixel(5, 3));
    }

    #[test]
    fn concat_waveform_covers_horizontal_join_across_image() {
        let original = RgbaImage::from_pixel(64, 48, image::Rgba([120, 130, 140, 255]));
        let mut with_seam = original.clone();
        add_concat_waveform_seam(&mut with_seam, 24, [30, 180, 155]);
        assert_ne!(with_seam.get_pixel(0, 24), original.get_pixel(0, 24));
        assert_ne!(with_seam.get_pixel(63, 24), original.get_pixel(63, 24));
        assert_eq!(with_seam.get_pixel(32, 2), original.get_pixel(32, 2));
        assert_eq!(with_seam.get_pixel(32, 40), original.get_pixel(32, 40));
    }

    #[test]
    fn obfuscate_creates_semitransparent_mask_at_randomized_size() {
        let fixed = RgbaImage::from_pixel(112, 112, image::Rgba([255, 255, 255, 255]));
        let fake = make_obfuscate(&fixed);
        assert!((97..=127).contains(&fake.width()));
        assert!((97..=127).contains(&fake.height()));
        assert!(fake.pixels().any(|p| p[3] > 0 && p[3] < 255));
    }
}
