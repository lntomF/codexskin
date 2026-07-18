use crate::{
    error::CommandError,
    models::{Theme, ThemeColors, ThemeLayers, ThemeSource},
    storage::backgrounds::{import_background_bytes, validate_background_bytes},
};
use image::RgbaImage;

const MAX_PALETTE_DIMENSION: u32 = 64;
const MIN_VISIBLE_ALPHA: u8 = 32;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

pub fn generate_wallpaper_theme(
    wallpaper_path: String,
    display_name: String,
    image_bytes: &[u8],
) -> Result<Theme, CommandError> {
    let average = average_visible_rgb(image_bytes)?;
    let is_dark = relative_luminance(average) < 0.5;
    let (foreground, muted, ambient_overlay_opacity, focus_overlay_opacity) = if is_dark {
        ("#F4F7FF", "#AAB4C7", 0.18, 0.76)
    } else {
        ("#172033", "#536174", 0.28, 0.82)
    };

    let surface = if is_dark {
        blend(average, [244, 247, 255], 0.10)
    } else {
        blend(average, [23, 32, 51], 0.08)
    };

    Ok(Theme {
        id: format!("wallpaper-{:016x}", stable_bytes_hash(image_bytes)),
        name: display_name,
        description: "由壁纸离线取色生成。".into(),
        colors: ThemeColors {
            accent: css_color(saturated_accent(average, is_dark)),
            background: css_color(average),
            surface: css_color(surface),
            foreground: foreground.into(),
            muted: muted.into(),
        },
        background_image: Some(wallpaper_path),
        source: ThemeSource::Wallpaper,
        layers: ThemeLayers {
            ambient_overlay_opacity,
            focus_overlay_opacity,
            sidebar_opacity: 0.58,
            card_opacity: 0.46,
        },
    })
}

pub fn import_wallpaper_theme(bytes: &[u8], display_name: &str) -> Result<Theme, CommandError> {
    // Validate before palette generation so size and format errors keep their background-specific codes.
    validate_background_bytes(bytes)?;
    // Decode before persisting so malformed input never leaves an orphaned file.
    generate_wallpaper_theme(String::new(), display_name.into(), bytes)?;
    let wallpaper_path = import_background_bytes(bytes)?;
    generate_wallpaper_theme(wallpaper_path, display_name.into(), bytes)
}

fn average_visible_rgb(image_bytes: &[u8]) -> Result<[u8; 3], CommandError> {
    let image = image::load_from_memory(image_bytes)
        .map_err(|error| CommandError::new("background_decode_failed", error.to_string()))?;
    let pixels = image
        .thumbnail(MAX_PALETTE_DIMENSION, MAX_PALETTE_DIMENSION)
        .to_rgba8();
    average_visible_pixels(&pixels)
}

fn average_visible_pixels(pixels: &RgbaImage) -> Result<[u8; 3], CommandError> {
    let mut totals = [0_u64; 3];
    let mut count = 0_u64;

    for pixel in pixels.pixels() {
        if pixel[3] < MIN_VISIBLE_ALPHA {
            continue;
        }
        totals[0] += u64::from(pixel[0]);
        totals[1] += u64::from(pixel[1]);
        totals[2] += u64::from(pixel[2]);
        count += 1;
    }

    if count == 0 {
        return Err(CommandError::new(
            "background_decode_failed",
            "背景图没有 alpha 值至少为 32 的像素。",
        ));
    }

    Ok([
        (totals[0] / count) as u8,
        (totals[1] / count) as u8,
        (totals[2] / count) as u8,
    ])
}

fn relative_luminance(rgb: [u8; 3]) -> f64 {
    let channel = |value: u8| {
        let normalized = f64::from(value) / 255.0;
        if normalized <= 0.04045 {
            normalized / 12.92
        } else {
            ((normalized + 0.055) / 1.055).powf(2.4)
        }
    };

    0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2])
}

fn saturated_accent(rgb: [u8; 3], is_dark: bool) -> [u8; 3] {
    let red = f64::from(rgb[0]) / 255.0;
    let green = f64::from(rgb[1]) / 255.0;
    let blue = f64::from(rgb[2]) / 255.0;
    let maximum = red.max(green).max(blue);
    let minimum = red.min(green).min(blue);
    let delta = maximum - minimum;

    let hue = if delta == 0.0 {
        220.0
    } else if maximum == red {
        60.0 * ((green - blue) / delta).rem_euclid(6.0)
    } else if maximum == green {
        60.0 * (((blue - red) / delta) + 2.0)
    } else {
        60.0 * (((red - green) / delta) + 4.0)
    };
    let saturation = if maximum == 0.0 { 0.0 } else { delta / maximum };
    let target_saturation = (saturation + 0.25).clamp(0.62, 0.88);
    let target_value = if is_dark {
        maximum.max(0.68)
    } else {
        maximum.min(0.70).max(0.48)
    };

    hsv_to_rgb(hue, target_saturation, target_value)
}

fn hsv_to_rgb(hue: f64, saturation: f64, value: f64) -> [u8; 3] {
    let chroma = value * saturation;
    let sector = hue / 60.0;
    let second = chroma * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());
    let (red, green, blue) = match sector.floor() as i32 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let offset = value - chroma;

    [
        ((red + offset) * 255.0).round() as u8,
        ((green + offset) * 255.0).round() as u8,
        ((blue + offset) * 255.0).round() as u8,
    ]
}

fn blend(base: [u8; 3], overlay: [u8; 3], overlay_opacity: f64) -> [u8; 3] {
    [
        (f64::from(base[0]) * (1.0 - overlay_opacity) + f64::from(overlay[0]) * overlay_opacity)
            .round() as u8,
        (f64::from(base[1]) * (1.0 - overlay_opacity) + f64::from(overlay[1]) * overlay_opacity)
            .round() as u8,
        (f64::from(base[2]) * (1.0 - overlay_opacity) + f64::from(overlay[2]) * overlay_opacity)
            .round() as u8,
    ]
}

fn css_color(rgb: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

fn stable_bytes_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(FNV_OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

#[cfg(test)]
mod tests {
    use super::{generate_wallpaper_theme, import_wallpaper_theme};
    use crate::models::ThemeSource;
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    fn png_bytes(color: [u8; 4]) -> Vec<u8> {
        let image = RgbaImage::from_pixel(96, 48, Rgba(color));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .expect("encode PNG");
        bytes
    }

    fn assert_css_color(value: &str) {
        assert_eq!(value.len(), 7);
        assert!(value.starts_with('#'));
        assert!(value[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit()));
    }

    fn assert_valid_opacities(theme: &crate::models::Theme) {
        for opacity in [
            theme.layers.ambient_overlay_opacity,
            theme.layers.focus_overlay_opacity,
            theme.layers.sidebar_opacity,
            theme.layers.card_opacity,
        ] {
            assert!((0.0..=1.0).contains(&opacity));
        }
    }

    #[test]
    fn generates_dark_wallpaper_theme_from_opaque_pixels() {
        let bytes = png_bytes([10, 20, 30, 255]);
        let theme = generate_wallpaper_theme(
            "file:///C:/wallpapers/night.png".into(),
            "Night".into(),
            &bytes,
        )
        .expect("dark wallpaper theme");

        assert_eq!(theme.source, ThemeSource::Wallpaper);
        assert_eq!(
            theme.background_image.as_deref(),
            Some("file:///C:/wallpapers/night.png")
        );
        assert_eq!(theme.colors.foreground, "#F4F7FF");
        assert_eq!(theme.colors.muted, "#AAB4C7");
        assert_eq!(theme.layers.ambient_overlay_opacity, 0.18);
        assert_eq!(theme.layers.focus_overlay_opacity, 0.76);
        assert_css_color(&theme.colors.accent);
        assert_css_color(&theme.colors.background);
        assert_css_color(&theme.colors.surface);
        assert_valid_opacities(&theme);
        assert_eq!(
            theme.id,
            generate_wallpaper_theme("another-path".into(), "Renamed".into(), &bytes)
                .expect("same bytes")
                .id
        );
    }

    #[test]
    fn generates_light_wallpaper_theme_from_opaque_pixels() {
        let bytes = png_bytes([245, 240, 230, 255]);
        let theme =
            generate_wallpaper_theme("file:///C:/wallpapers/day.png".into(), "Day".into(), &bytes)
                .expect("light wallpaper theme");

        assert_eq!(theme.source, ThemeSource::Wallpaper);
        assert_eq!(theme.colors.foreground, "#172033");
        assert_eq!(theme.colors.muted, "#536174");
        assert_eq!(theme.layers.ambient_overlay_opacity, 0.28);
        assert_eq!(theme.layers.focus_overlay_opacity, 0.82);
        assert_valid_opacities(&theme);
    }

    #[test]
    fn preserves_background_validation_codes_before_palette_generation() {
        let empty = import_wallpaper_theme(&[], "Empty")
            .expect_err("empty wallpaper input must fail before palette generation");
        assert_eq!(empty.code, "background_empty");

        let too_large = vec![0_u8; 12 * 1024 * 1024 + 1];
        let oversized = import_wallpaper_theme(&too_large, "Too large")
            .expect_err("oversized wallpaper input must fail before palette generation");
        assert_eq!(oversized.code, "background_too_large");
    }

    #[test]
    fn reports_background_decode_failed_for_malformed_bytes() {
        let bytes = b"not an image";
        let error = generate_wallpaper_theme("file:///bad.png".into(), "Bad".into(), bytes)
            .expect_err("malformed image must fail");
        assert_eq!(error.code, "background_decode_failed");

        let import_error =
            import_wallpaper_theme(bytes, "Bad").expect_err("malformed wallpaper import must fail");
        assert_eq!(import_error.code, "background_decode_failed");
    }
}
