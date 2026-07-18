use crate::{
    error::CommandError,
    models::{ContrastRegion, Theme, ThemeColors, ThemeContrast, ThemeLayers},
    storage::backgrounds::{
        import_background_bytes, read_managed_background_bytes, validate_background_bytes,
    },
};
use image::RgbaImage;

const MAX_PALETTE_DIMENSION: u32 = 64;
const MAX_CONTRAST_DIMENSION: u32 = 320;
const MIN_VISIBLE_ALPHA: u8 = 32;
const PALETTE_BINS: usize = 16 * 16 * 16;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const MIN_FOREGROUND_CONTRAST: f64 = 4.5;
const MIN_MUTED_CONTRAST: f64 = 3.0;
const MIN_TINT_SATURATION: f64 = 0.08;
const SAFE_LIGHT_FOREGROUND: [u8; 3] = [255, 255, 255];
const SAFE_DARK_FOREGROUND: [u8; 3] = [0, 0, 0];

#[derive(Clone, Copy, Default)]
struct ColorBin {
    weight: u64,
    totals: [u64; 3],
}

#[derive(Clone, Copy)]
struct PaletteSample {
    average: [u8; 3],
    dominant: [u8; 3],
    accent: [u8; 3],
    secondary: [u8; 3],
}

pub fn generate_wallpaper_theme(
    display_image: String,
    source_image: String,
    display_name: String,
    image_bytes: &[u8],
) -> Result<Theme, CommandError> {
    let (palette, contrast) = extract_visuals(image_bytes)?;
    let mut theme = Theme::wallpaper(
        format!("wallpaper-{:016x}", stable_bytes_hash(image_bytes)),
        display_name,
        "由上传图片离线取色生成：主色、辅色、面板与文字颜色均来自壁纸。".into(),
        theme_colors_from_palette(palette),
        display_image,
        source_image,
        ThemeLayers::wallpaper(),
    );
    theme.contrast = Some(contrast);
    Ok(theme)
}

pub fn import_wallpaper_theme(bytes: &[u8], display_name: &str) -> Result<Theme, CommandError> {
    validate_background_bytes(bytes)?;
    // Decode and palette-check before writing so malformed files leave no managed assets.
    let mut provisional =
        generate_wallpaper_theme(String::new(), String::new(), display_name.into(), bytes)?;
    let imported = import_background_bytes(bytes)?;
    // Analyse the exact, derived 16:9 JPEG that Codex receives. This matters for
    // portrait inputs because centre cropping changes which pixels sit under each UI area.
    let display_bytes = read_managed_background_bytes(&imported.display_image)?;
    refresh_wallpaper_theme_visuals(&mut provisional, &display_bytes)?;
    Ok(Theme {
        background_image: Some(imported.display_image),
        source_image: Some(imported.source_image),
        ..provisional
    })
}

/// Refreshes colours and regional readability data from the actual derived wallpaper.
/// The caller must supply bytes read from CodeSkin's managed wallpaper directory.
pub(crate) fn refresh_wallpaper_theme_visuals(
    theme: &mut Theme,
    image_bytes: &[u8],
) -> Result<(), CommandError> {
    let (palette, contrast) = extract_visuals(image_bytes)?;
    theme.colors = theme_colors_from_palette(palette);
    theme.contrast = Some(contrast);
    Ok(())
}

fn extract_visuals(image_bytes: &[u8]) -> Result<(PaletteSample, ThemeContrast), CommandError> {
    let image = image::load_from_memory(image_bytes)
        .map_err(|error| CommandError::new("background_decode_failed", error.to_string()))?;
    let palette = palette_from_pixels(&analysis_pixels(&image, MAX_PALETTE_DIMENSION))?;
    let contrast = contrast_from_pixels(&analysis_pixels(&image, MAX_CONTRAST_DIMENSION))?;
    Ok((palette, contrast))
}

fn analysis_pixels(image: &image::DynamicImage, max_dimension: u32) -> RgbaImage {
    // `thumbnail` may upscale a small image. That is counterproductive here:
    // interpolation erases photo detail, so a busy sidebar would be misclassified
    // as a simple surface. Only reduce an image that exceeds the analysis cap.
    if image.width() <= max_dimension && image.height() <= max_dimension {
        image.to_rgba8()
    } else {
        image.thumbnail(max_dimension, max_dimension).to_rgba8()
    }
}

fn theme_colors_from_palette(palette: PaletteSample) -> ThemeColors {
    let is_dark = relative_luminance(palette.average) < 0.46;
    let (foreground, muted) = if is_dark {
        ("#F4F7FF", "#BBC5D8")
    } else {
        ("#172033", "#536174")
    };
    // A panel base follows the image but is nudged toward the text contrast.
    // The injected CSS only uses it at low alpha on local UI surfaces.
    let surface = if is_dark {
        blend(palette.dominant, [244, 247, 255], 0.12)
    } else {
        blend(palette.dominant, [23, 32, 51], 0.10)
    };

    ThemeColors {
        accent: css_color(palette.accent),
        secondary: css_color(palette.secondary),
        background: css_color(palette.average),
        surface: css_color(surface),
        foreground: foreground.into(),
        muted: muted.into(),
    }
}

fn palette_from_pixels(pixels: &RgbaImage) -> Result<PaletteSample, CommandError> {
    let mut bins = [ColorBin::default(); PALETTE_BINS];
    let mut totals = [0_u64; 3];
    let mut total_weight = 0_u64;

    for pixel in pixels.pixels() {
        if pixel[3] < MIN_VISIBLE_ALPHA {
            continue;
        }
        let weight = u64::from(pixel[3]);
        let rgb = [pixel[0], pixel[1], pixel[2]];
        let bin = &mut bins[color_bin_index(rgb)];
        bin.weight += weight;
        for channel in 0..3 {
            let weighted = u64::from(rgb[channel]) * weight;
            totals[channel] += weighted;
            bin.totals[channel] += weighted;
        }
        total_weight += weight;
    }

    if total_weight == 0 {
        return Err(CommandError::new(
            "background_decode_failed",
            "背景图没有 alpha 值至少为 32 的像素。",
        ));
    }

    let average = weighted_rgb(totals, total_weight);
    let mut candidates: Vec<(ColorBin, [u8; 3])> = bins
        .into_iter()
        .filter(|bin| bin.weight > 0)
        .map(|bin| (bin, weighted_rgb(bin.totals, bin.weight)))
        .collect();
    candidates.sort_by(|(left, _), (right, _)| right.weight.cmp(&left.weight));
    let dominant = candidates.first().map(|(_, rgb)| *rgb).unwrap_or(average);

    let accent_candidate = candidates
        .iter()
        .max_by(|(left_bin, left_rgb), (right_bin, right_rgb)| {
            vibrant_score(*left_rgb, left_bin.weight, total_weight).total_cmp(&vibrant_score(
                *right_rgb,
                right_bin.weight,
                total_weight,
            ))
        })
        .map(|(_, rgb)| *rgb)
        .unwrap_or(dominant);
    let accent = accent_from_candidate(accent_candidate, relative_luminance(average) < 0.46);

    let secondary_candidate = candidates
        .iter()
        .filter(|(bin, rgb)| {
            color_distance(*rgb, accent_candidate) >= 52.0
                && vibrant_score(*rgb, bin.weight, total_weight) > 0.04
        })
        .max_by(|(left_bin, left_rgb), (right_bin, right_rgb)| {
            vibrant_score(*left_rgb, left_bin.weight, total_weight).total_cmp(&vibrant_score(
                *right_rgb,
                right_bin.weight,
                total_weight,
            ))
        })
        .map(|(_, rgb)| *rgb)
        .unwrap_or(dominant);
    let secondary = if color_distance(secondary_candidate, accent_candidate) < 30.0 {
        rotate_hue(accent, 34.0)
    } else {
        accent_from_candidate(secondary_candidate, relative_luminance(average) < 0.46)
    };

    Ok(PaletteSample {
        average,
        dominant,
        accent,
        secondary,
    })
}

fn contrast_from_pixels(pixels: &RgbaImage) -> Result<ThemeContrast, CommandError> {
    if pixels.width() == 0 || pixels.height() == 0 {
        return Err(CommandError::new(
            "background_decode_failed",
            "背景图没有可用于计算区域对比度的像素。",
        ));
    }

    Ok(ThemeContrast {
        sidebar: contrast_region(pixels, RegionBounds::new(0.00, 0.00, 0.16, 1.00))?,
        content: contrast_region(pixels, RegionBounds::new(0.16, 0.00, 0.82, 0.18))?,
        // The Codex application-menu/title strip is the full-width topmost band
        // (about 39 px in the current renderer). It must not inherit the content
        // sample because header pixels can have a different hue and luminance.
        header: Some(contrast_region(
            pixels,
            RegionBounds::new(0.00, 0.00, 1.00, 0.09),
        )?),
        info_panel: contrast_region(pixels, RegionBounds::new(0.82, 0.08, 1.00, 0.50))?,
        composer: contrast_region(pixels, RegionBounds::new(0.36, 0.80, 0.78, 1.00))?,
    })
}

#[derive(Clone, Copy)]
struct RegionBounds {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl RegionBounds {
    const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }
}

fn contrast_region(
    pixels: &RgbaImage,
    bounds: RegionBounds,
) -> Result<ContrastRegion, CommandError> {
    let width = pixels.width();
    let height = pixels.height();
    let left = ((bounds.left * width as f32).floor() as u32).min(width.saturating_sub(1));
    let top = ((bounds.top * height as f32).floor() as u32).min(height.saturating_sub(1));
    let right = ((bounds.right * width as f32).ceil() as u32).clamp(left + 1, width);
    let bottom = ((bounds.bottom * height as f32).ceil() as u32).clamp(top + 1, height);

    let mut luminance_sum = 0.0_f64;
    let mut rgb_sum = [0.0_f64; 3];
    let mut weight_sum = 0.0_f64;
    let mut complexity_sum = 0.0_f64;
    let mut complexity_samples = 0_u64;

    for y in top..bottom {
        for x in left..right {
            let pixel = pixels.get_pixel(x, y);
            if pixel[3] < MIN_VISIBLE_ALPHA {
                continue;
            }
            let rgb = [pixel[0], pixel[1], pixel[2]];
            let alpha = f64::from(pixel[3]) / 255.0;
            luminance_sum += relative_luminance(rgb) * alpha;
            for channel in 0..3 {
                rgb_sum[channel] += f64::from(rgb[channel]) * alpha;
            }
            weight_sum += alpha;

            for (next_x, next_y) in [(x + 1, y), (x, y + 1)] {
                if next_x >= right || next_y >= bottom {
                    continue;
                }
                let neighbour = pixels.get_pixel(next_x, next_y);
                if neighbour[3] < MIN_VISIBLE_ALPHA {
                    continue;
                }
                let neighbour_rgb = [neighbour[0], neighbour[1], neighbour[2]];
                complexity_sum +=
                    (perceived_brightness(rgb) - perceived_brightness(neighbour_rgb)).abs();
                complexity_samples += 1;
            }
        }
    }

    if weight_sum == 0.0 {
        return Err(CommandError::new(
            "background_decode_failed",
            "背景图指定区域没有可见像素。",
        ));
    }

    let average_rgb = [
        (rgb_sum[0] / weight_sum).round() as u8,
        (rgb_sum[1] / weight_sum).round() as u8,
        (rgb_sum[2] / weight_sum).round() as u8,
    ];
    let luminance = (luminance_sum / weight_sum) as f32;
    // Adjacent-pixel brightness differences express local visual detail. The scale
    // intentionally reaches its high-glass range for textured photos, not for a
    // clean gradient or a flat colour block.
    let complexity = if complexity_samples == 0 {
        0.0
    } else {
        ((complexity_sum / complexity_samples as f64) * 5.0).clamp(0.0, 1.0) as f32
    };
    let in_transition_band = (0.32..=0.62).contains(&luminance);
    let panel_opacity =
        (0.16 + complexity * 0.24 + if in_transition_band { 0.06 } else { 0.0 }).clamp(0.16, 0.45);
    let blur_px = (8.0 + complexity * 7.0 + if in_transition_band { 1.0 } else { 0.0 })
        .round()
        .clamp(8.0, 16.0) as u8;

    // Region foregrounds are deliberately derived from the pixels that actually
    // sit behind each Codex area. A hue-preserving candidate keeps a warm wallpaper
    // from producing the same text colour as a cool one; the contrast solver then
    // moves it toward a safe extreme without sacrificing WCAG-style readability.
    let prefer_light_text = contrast_ratio(SAFE_LIGHT_FOREGROUND, average_rgb)
        >= contrast_ratio(SAFE_DARK_FOREGROUND, average_rgb);
    let foreground =
        accessible_region_foreground(average_rgb, prefer_light_text, MIN_FOREGROUND_CONTRAST);
    let muted = accessible_region_muted(average_rgb, foreground, prefer_light_text);
    let panel_color = if prefer_light_text {
        "#12161D"
    } else {
        "#F7F4EE"
    };
    let text_shadow = text_shadow_for(foreground, complexity);

    Ok(ContrastRegion {
        luminance,
        complexity,
        foreground: css_color(foreground),
        muted: css_color(muted),
        panel_color: panel_color.into(),
        panel_opacity,
        blur_px,
        text_shadow,
    })
}

fn accessible_region_foreground(
    background: [u8; 3],
    prefer_light: bool,
    minimum_contrast: f64,
) -> [u8; 3] {
    let (hue, saturation, _) = rgb_to_hsv(background);
    if saturation < MIN_TINT_SATURATION {
        return safe_foreground(background, prefer_light, minimum_contrast);
    }

    // The foreground keeps the wallpaper's hue but caps saturation so text remains
    // calm. Its initial value deliberately leaves room for the contrast solver to
    // preserve more of that tint whenever the sampled area permits it.
    let candidate = hsv_to_rgb(
        hue,
        (0.15 + saturation * 0.42).clamp(0.15, 0.48),
        if prefer_light { 0.94 } else { 0.19 },
    );
    adjust_for_contrast(background, candidate, prefer_light, minimum_contrast)
}

fn accessible_region_muted(
    background: [u8; 3],
    foreground: [u8; 3],
    prefer_light: bool,
) -> [u8; 3] {
    // Muted text stays in the final foreground's colour family, then gets the same
    // deterministic contrast correction with the lower 3:1 secondary-text target.
    let candidate = blend(foreground, background, 0.26);
    adjust_for_contrast(background, candidate, prefer_light, MIN_MUTED_CONTRAST)
}

fn adjust_for_contrast(
    background: [u8; 3],
    candidate: [u8; 3],
    prefer_light: bool,
    minimum_contrast: f64,
) -> [u8; 3] {
    if contrast_ratio(candidate, background) >= minimum_contrast {
        return candidate;
    }

    let safe_extreme = if prefer_light {
        SAFE_LIGHT_FOREGROUND
    } else {
        SAFE_DARK_FOREGROUND
    };
    // Twenty-four bounded steps preserve as much wallpaper hue as possible while
    // guaranteeing a finite, deterministic fallback path.
    for step in 1..=24 {
        let adjusted = blend(candidate, safe_extreme, step as f64 / 24.0);
        if contrast_ratio(adjusted, background) >= minimum_contrast {
            return adjusted;
        }
    }
    safe_foreground(background, prefer_light, minimum_contrast)
}

fn safe_foreground(background: [u8; 3], prefer_light: bool, minimum_contrast: f64) -> [u8; 3] {
    let preferred = if prefer_light {
        SAFE_LIGHT_FOREGROUND
    } else {
        SAFE_DARK_FOREGROUND
    };
    if contrast_ratio(preferred, background) >= minimum_contrast {
        return preferred;
    }

    let alternate = if prefer_light {
        SAFE_DARK_FOREGROUND
    } else {
        SAFE_LIGHT_FOREGROUND
    };
    if contrast_ratio(alternate, background) >= minimum_contrast {
        return alternate;
    }

    // For an opaque sRGB background, black or white always provides the maximum
    // available contrast. Keep this deterministic branch as a defensive guard for
    // future changes to the threshold or input preprocessing.
    if contrast_ratio(SAFE_LIGHT_FOREGROUND, background)
        >= contrast_ratio(SAFE_DARK_FOREGROUND, background)
    {
        SAFE_LIGHT_FOREGROUND
    } else {
        SAFE_DARK_FOREGROUND
    }
}

fn contrast_ratio(left: [u8; 3], right: [u8; 3]) -> f64 {
    let left_luminance = relative_luminance(left);
    let right_luminance = relative_luminance(right);
    (left_luminance.max(right_luminance) + 0.05) / (left_luminance.min(right_luminance) + 0.05)
}

fn text_shadow_for(foreground: [u8; 3], complexity: f32) -> String {
    let alpha = (0.42 + f64::from(complexity) * 0.18).clamp(0.42, 0.60);
    let shadow = if relative_luminance(foreground) >= 0.5 {
        "0,0,0"
    } else {
        "255,255,255"
    };
    format!("0 1px 2px rgba({shadow},{alpha:.2})")
}

fn perceived_brightness(rgb: [u8; 3]) -> f64 {
    (0.299 * f64::from(rgb[0]) + 0.587 * f64::from(rgb[1]) + 0.114 * f64::from(rgb[2])) / 255.0
}

fn color_bin_index(rgb: [u8; 3]) -> usize {
    ((usize::from(rgb[0]) >> 4) << 8)
        | ((usize::from(rgb[1]) >> 4) << 4)
        | (usize::from(rgb[2]) >> 4)
}

fn weighted_rgb(totals: [u64; 3], weight: u64) -> [u8; 3] {
    [
        (totals[0] / weight) as u8,
        (totals[1] / weight) as u8,
        (totals[2] / weight) as u8,
    ]
}

fn vibrant_score(rgb: [u8; 3], weight: u64, total_weight: u64) -> f64 {
    let (_, saturation, value) = rgb_to_hsv(rgb);
    let coverage = (weight as f64 / total_weight as f64).sqrt();
    let middle_value = 1.0 - ((value - 0.60).abs() / 0.60).min(1.0);
    saturation * (0.45 + 0.55 * middle_value) * (0.25 + 0.75 * coverage)
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

fn accent_from_candidate(rgb: [u8; 3], is_dark: bool) -> [u8; 3] {
    let (hue, saturation, value) = rgb_to_hsv(rgb);
    hsv_to_rgb(
        hue,
        (saturation + 0.18).clamp(0.58, 0.90),
        if is_dark {
            value.max(0.66)
        } else {
            value.clamp(0.46, 0.72)
        },
    )
}

fn rotate_hue(rgb: [u8; 3], degrees: f64) -> [u8; 3] {
    let (hue, saturation, value) = rgb_to_hsv(rgb);
    hsv_to_rgb((hue + degrees).rem_euclid(360.0), saturation, value)
}

fn rgb_to_hsv(rgb: [u8; 3]) -> (f64, f64, f64) {
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
    (hue, saturation, maximum)
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

fn color_distance(left: [u8; 3], right: [u8; 3]) -> f64 {
    let red = f64::from(left[0]) - f64::from(right[0]);
    let green = f64::from(left[1]) - f64::from(right[1]);
    let blue = f64::from(left[2]) - f64::from(right[2]);
    (red * red + green * green + blue * blue).sqrt()
}

fn blend(base: [u8; 3], overlay: [u8; 3], opacity: f64) -> [u8; 3] {
    [
        (f64::from(base[0]) * (1.0 - opacity) + f64::from(overlay[0]) * opacity).round() as u8,
        (f64::from(base[1]) * (1.0 - opacity) + f64::from(overlay[1]) * opacity).round() as u8,
        (f64::from(base[2]) * (1.0 - opacity) + f64::from(overlay[2]) * opacity).round() as u8,
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
    use super::generate_wallpaper_theme;
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    fn png_bytes(color: [u8; 4]) -> Vec<u8> {
        let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(96, 48, Rgba(color)));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn png_bytes_from_image(image: RgbaImage) -> Vec<u8> {
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn wallpaper_with_header(header: [u8; 3], body: [u8; 3]) -> Vec<u8> {
        let mut image = RgbaImage::from_pixel(160, 90, Rgba([body[0], body[1], body[2], 255]));
        for (_, y, pixel) in image.enumerate_pixels_mut() {
            if y < 9 {
                *pixel = Rgba([header[0], header[1], header[2], 255]);
            }
        }
        png_bytes_from_image(image)
    }

    fn header_foreground(theme: &crate::models::Theme) -> [u8; 3] {
        let value = serde_json::to_value(theme).expect("theme serialization");
        parse_css_color(
            value["contrast"]["header"]["foreground"]
                .as_str()
                .expect("header foreground must serialize"),
        )
    }

    fn header_foreground_css(theme: &crate::models::Theme) -> String {
        let value = serde_json::to_value(theme).expect("theme serialization");
        value["contrast"]["header"]["foreground"]
            .as_str()
            .expect("header foreground must serialize")
            .to_owned()
    }

    #[test]
    fn header_foregrounds_follow_light_header_hue_without_fixed_global_foreground() {
        let warm_header = [246, 226, 201];
        let cool_header = [202, 226, 247];
        let warm = generate_wallpaper_theme(
            "file:///display.jpg".into(),
            "file:///source.png".into(),
            "Warm header".into(),
            &wallpaper_with_header(warm_header, [12, 28, 46]),
        )
        .unwrap();
        let cool = generate_wallpaper_theme(
            "file:///display.jpg".into(),
            "file:///source.png".into(),
            "Cool header".into(),
            &wallpaper_with_header(cool_header, [12, 28, 46]),
        )
        .unwrap();

        assert_ne!(header_foreground_css(&warm), "#172033");
        assert_ne!(header_foreground_css(&cool), "#172033");
        assert_ne!(header_foreground_css(&warm), header_foreground_css(&cool));
        assert!(contrast_ratio(header_foreground(&warm), warm_header) >= 4.5);
        assert!(contrast_ratio(header_foreground(&cool), cool_header) >= 4.5);
    }

    #[test]
    fn header_foregrounds_follow_dark_header_hue_without_fixed_global_foreground() {
        let wine_header = [50, 20, 34];
        let teal_header = [8, 39, 48];
        let wine = generate_wallpaper_theme(
            "file:///display.jpg".into(),
            "file:///source.png".into(),
            "Wine header".into(),
            &wallpaper_with_header(wine_header, [232, 230, 222]),
        )
        .unwrap();
        let teal = generate_wallpaper_theme(
            "file:///display.jpg".into(),
            "file:///source.png".into(),
            "Teal header".into(),
            &wallpaper_with_header(teal_header, [232, 230, 222]),
        )
        .unwrap();

        assert_ne!(header_foreground_css(&wine), "#F4F7FF");
        assert_ne!(header_foreground_css(&teal), "#F4F7FF");
        assert_ne!(header_foreground_css(&wine), header_foreground_css(&teal));
        assert!(contrast_ratio(header_foreground(&wine), wine_header) >= 4.5);
        assert!(contrast_ratio(header_foreground(&teal), teal_header) >= 4.5);
    }

    #[test]
    fn achromatic_or_complex_header_falls_back_to_stable_high_contrast_foreground() {
        let neutral_header = [128, 128, 128];
        let theme = generate_wallpaper_theme(
            "file:///display.jpg".into(),
            "file:///source.png".into(),
            "Neutral header".into(),
            &wallpaper_with_header(neutral_header, [30, 45, 60]),
        )
        .unwrap();
        let value = serde_json::to_value(&theme).expect("theme serialization");

        assert_eq!(header_foreground_css(&theme), "#000000");
        assert!(contrast_ratio(header_foreground(&theme), neutral_header) >= 4.5);
        assert!(value["contrast"]["header"]["textShadow"]
            .as_str()
            .expect("header shadow")
            .contains("rgba(255,255,255,"));
    }

    #[test]
    fn header_palette_serializes_with_generated_themes() {
        let theme = generate_wallpaper_theme(
            "file:///display.jpg".into(),
            "file:///source.png".into(),
            "Header serialization".into(),
            &wallpaper_with_header([200, 225, 245], [16, 34, 54]),
        )
        .unwrap();
        let value = serde_json::to_value(theme).expect("theme serialization");

        assert!(value["contrast"]["header"]["foreground"].is_string());
        assert!(value["contrast"]["header"]["muted"].is_string());
        assert!(value["contrast"]["header"]["textShadow"].is_string());
    }

    #[test]
    fn derives_light_and_dark_readability_from_uploaded_pixels() {
        let dark = generate_wallpaper_theme(
            "file:///display.jpg".into(),
            "file:///source.png".into(),
            "Dark".into(),
            &png_bytes([10, 20, 30, 255]),
        )
        .unwrap();
        let light = generate_wallpaper_theme(
            "file:///display.jpg".into(),
            "file:///source.png".into(),
            "Light".into(),
            &png_bytes([245, 240, 230, 255]),
        )
        .unwrap();
        assert_eq!(dark.colors.foreground, "#F4F7FF");
        assert_eq!(light.colors.foreground, "#172033");
        assert_eq!(
            dark.background_image.as_deref(),
            Some("file:///display.jpg")
        );
        assert_eq!(dark.source_image.as_deref(), Some("file:///source.png"));
    }

    #[test]
    fn derives_region_specific_contrast_for_dark_detailed_and_light_simple_areas() {
        let mut image = RgbaImage::from_pixel(160, 90, Rgba([242, 233, 220, 255]));
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            let in_sidebar = x < 26;
            let in_info_panel = x >= 131 && (7..45).contains(&y);
            if in_sidebar || in_info_panel {
                let value = if (x + y) % 2 == 0 { 12 } else { 58 };
                *pixel = Rgba([value, value + 6, value + 12, 255]);
            }
        }

        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        let theme = generate_wallpaper_theme(
            "file:///display.jpg".into(),
            "file:///source.png".into(),
            "Region contrast".into(),
            &bytes,
        )
        .unwrap();
        let region_contrast = theme.contrast.as_ref().unwrap();
        let value = serde_json::to_value(&theme).unwrap();

        assert!(
            super::relative_luminance(parse_css_color(&region_contrast.sidebar.foreground))
                > super::relative_luminance([35, 41, 47]),
            "dark sidebar backgrounds need a light, wallpaper-tinted foreground"
        );
        assert!(
            contrast_ratio(
                parse_css_color(&region_contrast.sidebar.foreground),
                [35, 41, 47]
            ) >= 4.5,
            "dark sidebar foreground remains WCAG-readable"
        );
        assert!(
            value["contrast"]["sidebar"]["panelOpacity"]
                .as_f64()
                .unwrap()
                >= 0.35,
            "detailed sidebar backgrounds need a stronger local glass panel"
        );
        assert!(
            value["contrast"]["sidebar"]["blurPx"].as_u64().unwrap() >= 12,
            "detailed sidebar backgrounds need stronger local blur"
        );
        assert!(
            super::relative_luminance(parse_css_color(&region_contrast.content.foreground))
                < super::relative_luminance([242, 233, 220]),
            "light top content backgrounds need a dark, wallpaper-tinted foreground"
        );
        assert!(
            contrast_ratio(
                parse_css_color(&region_contrast.content.foreground),
                [242, 233, 220]
            ) >= 4.5,
            "light content foreground remains WCAG-readable"
        );
        assert!(
            value["contrast"]["content"]["panelOpacity"]
                .as_f64()
                .unwrap()
                <= 0.27,
            "simple light content should keep the wallpaper visually open"
        );
        assert!(
            super::relative_luminance(parse_css_color(&region_contrast.info_panel.foreground))
                > super::relative_luminance([35, 41, 47]),
            "dark right information panel backgrounds need a light, wallpaper-tinted foreground"
        );
        assert!(
            contrast_ratio(
                parse_css_color(&region_contrast.info_panel.foreground),
                [35, 41, 47]
            ) >= 4.5,
            "information panel foreground remains WCAG-readable"
        );
    }

    fn parse_css_color(css: &str) -> [u8; 3] {
        assert!(
            css.len() == 7 && css.starts_with('#'),
            "expected #RRGGBB, got {css}"
        );
        [
            u8::from_str_radix(&css[1..3], 16).unwrap(),
            u8::from_str_radix(&css[3..5], 16).unwrap(),
            u8::from_str_radix(&css[5..7], 16).unwrap(),
        ]
    }

    fn contrast_ratio(left: [u8; 3], right: [u8; 3]) -> f64 {
        let left_luminance = super::relative_luminance(left);
        let right_luminance = super::relative_luminance(right);
        (left_luminance.max(right_luminance) + 0.05) / (left_luminance.min(right_luminance) + 0.05)
    }

    fn assert_region_foregrounds_are_accessible(theme: &crate::models::Theme, background: [u8; 3]) {
        let contrast = theme
            .contrast
            .as_ref()
            .expect("wallpaper themes include contrast data");
        for (name, region) in [
            ("sidebar", &contrast.sidebar),
            ("content", &contrast.content),
            ("info panel", &contrast.info_panel),
            ("composer", &contrast.composer),
        ] {
            assert!(
                contrast_ratio(parse_css_color(&region.foreground), background) >= 4.5,
                "{name} foreground {} must meet the minimum contrast against {}",
                region.foreground,
                super::css_color(background),
            );
        }
    }

    #[test]
    fn equally_light_wallpapers_with_different_hues_get_distinct_content_foregrounds() {
        let warm = generate_wallpaper_theme(
            "file:///display.jpg".into(),
            "file:///source.png".into(),
            "Warm light".into(),
            &png_bytes([242, 216, 216, 255]),
        )
        .unwrap();
        let cool = generate_wallpaper_theme(
            "file:///display.jpg".into(),
            "file:///source.png".into(),
            "Cool light".into(),
            &png_bytes([214, 228, 247, 255]),
        )
        .unwrap();

        let warm_content = &warm.contrast.as_ref().unwrap().content;
        let cool_content = &cool.contrast.as_ref().unwrap().content;
        assert_ne!(
            warm_content.foreground, cool_content.foreground,
            "two light regions with distinct hues must not collapse to one fixed dark foreground"
        );
        assert_region_foregrounds_are_accessible(&warm, [242, 216, 216]);
        assert_region_foregrounds_are_accessible(&cool, [214, 228, 247]);
    }

    #[test]
    fn equally_dark_wallpapers_with_different_hues_get_distinct_info_foregrounds() {
        let wine = generate_wallpaper_theme(
            "file:///display.jpg".into(),
            "file:///source.png".into(),
            "Wine dark".into(),
            &png_bytes([54, 14, 28, 255]),
        )
        .unwrap();
        let teal = generate_wallpaper_theme(
            "file:///display.jpg".into(),
            "file:///source.png".into(),
            "Teal dark".into(),
            &png_bytes([8, 39, 48, 255]),
        )
        .unwrap();

        let wine_info = &wine.contrast.as_ref().unwrap().info_panel;
        let teal_info = &teal.contrast.as_ref().unwrap().info_panel;
        assert_ne!(
            wine_info.foreground, teal_info.foreground,
            "two dark regions with distinct hues must not collapse to one fixed light foreground"
        );
        assert_region_foregrounds_are_accessible(&wine, [54, 14, 28]);
        assert_region_foregrounds_are_accessible(&teal, [8, 39, 48]);
    }

    #[test]
    fn achromatic_regions_fall_back_to_a_stable_wcag_safe_foreground() {
        let middle_gray = generate_wallpaper_theme(
            "file:///display.jpg".into(),
            "file:///source.png".into(),
            "Neutral gray".into(),
            &png_bytes([128, 128, 128, 255]),
        )
        .unwrap();
        let content = &middle_gray.contrast.as_ref().unwrap().content;

        assert_eq!(content.foreground, "#000000");
        assert!(contrast_ratio(parse_css_color(&content.foreground), [128, 128, 128]) >= 4.5);
    }

    #[test]
    fn derives_a_distinct_secondary_accent_from_a_two_colour_wallpaper() {
        let mut image = RgbaImage::new(96, 48);
        for (x, _, pixel) in image.enumerate_pixels_mut() {
            *pixel = if x < 48 {
                Rgba([23, 111, 214, 255])
            } else {
                Rgba([198, 72, 157, 255])
            };
        }
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        let theme = generate_wallpaper_theme(
            "file:///display.jpg".into(),
            "file:///source.png".into(),
            "Two tone".into(),
            &bytes,
        )
        .unwrap();
        assert_ne!(theme.colors.accent, theme.colors.secondary);
        assert!(theme.colors.accent.starts_with('#'));
        assert!(theme.colors.secondary.starts_with('#'));
    }
}
