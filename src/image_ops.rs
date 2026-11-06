//! Shared pixel-level image operations used by the captcha generators.
use crate::{CaptchaError, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use std::io::Cursor;

pub fn decode(bytes: &[u8]) -> Result<RgbaImage> {
    Ok(image::load_from_memory(bytes)?.to_rgba8())
}

pub fn encode_data_uri(image: &RgbaImage, format: &str) -> Result<String> {
    let fmt = match format.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "png" => ImageFormat::Png,
        "webp" => ImageFormat::WebP,
        other => {
            return Err(CaptchaError::InvalidInput(format!(
                "unsupported image format: {other}"
            )));
        }
    };
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image.clone()).write_to(&mut bytes, fmt)?;
    let normalized = format.to_ascii_lowercase();
    let mime = if normalized == "jpg" {
        "jpeg"
    } else {
        &normalized
    };
    Ok(format!(
        "data:image/{mime};base64,{}",
        STANDARD.encode(bytes.into_inner())
    ))
}

/// Copy source pixels where mask alpha > 100, matching TAC cutImage.
pub fn cut_by_mask(source: &RgbaImage, mask: &RgbaImage, x: u32, y: u32) -> Result<RgbaImage> {
    if x + mask.width() > source.width() || y + mask.height() > source.height() {
        return Err(CaptchaError::InvalidInput(
            "mask crop is outside background bounds".into(),
        ));
    }
    let mut out = RgbaImage::new(mask.width(), mask.height());
    for my in 0..mask.height() {
        for mx in 0..mask.width() {
            if mask.get_pixel(mx, my)[3] > 100 {
                out.put_pixel(mx, my, *source.get_pixel(x + mx, y + my));
            }
        }
    }
    Ok(out)
}

/// Alpha composite a source image over the destination with clipping.
pub fn overlay(base: &mut RgbaImage, cover: &RgbaImage, x: i32, y: i32) {
    for cy in 0..cover.height() {
        for cx in 0..cover.width() {
            let (dx, dy) = (x + cx as i32, y + cy as i32);
            if dx < 0 || dy < 0 || dx >= base.width() as i32 || dy >= base.height() as i32 {
                continue;
            }
            let src = cover.get_pixel(cx, cy).0;
            let dst = base.get_pixel(dx as u32, dy as u32).0;
            let a = src[3] as u32;
            let inv = 255 - a;
            let out_a = a + (dst[3] as u32 * inv + 127) / 255;
            let mut out = [0; 4];
            for c in 0..3 {
                let premul = src[c] as u32 * a + (dst[c] as u32 * dst[3] as u32 * inv + 127) / 255;
                out[c] = (premul + out_a / 2)
                    .checked_div(out_a)
                    .unwrap_or(0)
                    .min(255) as u8;
            }
            out[3] = out_a.min(255) as u8;
            base.put_pixel(dx as u32, dy as u32, Rgba(out));
        }
    }
}

/// Bilinear affine rotation around the image center (AWT-style interpolation).
pub fn rotate_same_size(src: &RgbaImage, degrees: f64) -> RgbaImage {
    let (w, h) = (src.width(), src.height());
    let mut out = RgbaImage::new(w, h);
    let (cx, cy) = ((w as f64 - 1.0) / 2.0, (h as f64 - 1.0) / 2.0);
    let (sin, cos) = degrees.to_radians().sin_cos();
    for y in 0..h {
        for x in 0..w {
            let dx = x as f64 - cx;
            let dy = y as f64 - cy;
            let sx = cos * dx + sin * dy + cx;
            let sy = -sin * dx + cos * dy + cy;
            let (x0, y0) = (sx.floor() as i32, sy.floor() as i32);
            let (fx, fy) = (sx - sx.floor(), sy - sy.floor());
            let mut accum = [0.0f64; 4];
            for (ox, wx) in [(0, 1.0 - fx), (1, fx)] {
                for (oy, wy) in [(0, 1.0 - fy), (1, fy)] {
                    let px = x0 + ox;
                    let py = y0 + oy;
                    if px < 0 || py < 0 || px >= w as i32 || py >= h as i32 {
                        continue;
                    }
                    let p = src.get_pixel(px as u32, py as u32).0;
                    let weight = wx * wy;
                    let alpha = p[3] as f64 / 255.0 * weight;
                    for c in 0..3 {
                        accum[c] += p[c] as f64 * alpha;
                    }
                    accum[3] += p[3] as f64 * weight;
                }
            }
            let alpha = accum[3].clamp(0.0, 255.0);
            let mut rgba = [0; 4];
            if alpha > 0.0 {
                for c in 0..3 {
                    rgba[c] = (accum[c] / (alpha / 255.0)).round().clamp(0.0, 255.0) as u8;
                }
            }
            rgba[3] = alpha.round() as u8;
            out.put_pixel(x, y, Rgba(rgba));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cut_respects_alpha_threshold() {
        let src = RgbaImage::from_pixel(2, 1, Rgba([10, 20, 30, 255]));
        let mut mask = RgbaImage::new(2, 1);
        mask.put_pixel(0, 0, Rgba([0, 0, 0, 101]));
        mask.put_pixel(1, 0, Rgba([0, 0, 0, 100]));
        let cut = cut_by_mask(&src, &mask, 0, 0).unwrap();
        assert_eq!(*cut.get_pixel(0, 0), Rgba([10, 20, 30, 255]));
        assert_eq!(*cut.get_pixel(1, 0), Rgba([0, 0, 0, 0]));
    }
    #[test]
    fn rotate_preserves_dimensions() {
        let mut src = RgbaImage::new(5, 5);
        src.put_pixel(2, 0, Rgba([1, 2, 3, 255]));
        let rotated = rotate_same_size(&src, 90.0);
        assert_eq!(rotated.dimensions(), (5, 5));
        assert_eq!(rotated.get_pixel(4, 2)[3], 255);
    }
}
