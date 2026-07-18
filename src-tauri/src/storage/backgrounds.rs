use crate::error::CommandError;
use image::{
    codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage, GenericImageView, ImageFormat,
};
use std::{
    fs::{self, OpenOptions},
    io::{Cursor, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_BACKGROUND_BYTES: usize = 12 * 1024 * 1024;
const MAX_BACKGROUND_DIMENSION: u32 = 8192;
const DISPLAY_WIDTH: u32 = 2560;
const DISPLAY_HEIGHT: u32 = 1440;
const JPEG_QUALITY: u8 = 90;
const PREVIEW_WIDTH: u32 = 480;
const PREVIEW_HEIGHT: u32 = 270;
const PREVIEW_JPEG_QUALITY: u8 = 78;
const MAX_PREVIEW_BYTES: usize = 512 * 1024;
const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const MAX_OUTPUT_PATH_ATTEMPTS: u32 = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedBackground {
    pub source_image: String,
    pub display_image: String,
}

pub fn validate_background_bytes(bytes: &[u8]) -> Result<ImageFormat, CommandError> {
    if bytes.is_empty() {
        return Err(CommandError::new("background_empty", "背景图文件为空。"));
    }
    if bytes.len() > MAX_BACKGROUND_BYTES {
        return Err(CommandError::new(
            "background_too_large",
            "背景图不能超过 12 MiB。",
        ));
    }

    let format = image::guess_format(bytes)
        .map_err(|error| CommandError::new("background_decode_failed", error.to_string()))?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    ) {
        return Err(CommandError::new(
            "background_format_unsupported",
            "仅支持 PNG、JPEG 或 WebP 背景图。",
        ));
    }

    let decoded = image::load_from_memory(bytes)
        .map_err(|error| CommandError::new("background_decode_failed", error.to_string()))?;
    if decoded.width() > MAX_BACKGROUND_DIMENSION || decoded.height() > MAX_BACKGROUND_DIMENSION {
        return Err(CommandError::new(
            "background_dimensions_too_large",
            "背景图宽高均不能超过 8192 像素。",
        ));
    }
    Ok(format)
}

/// Retains the uploaded source and creates a CodeSkin-owned 2560 x 1440 JPEG.
/// Non-16:9 sources are cropped from the centre before resampling.
pub fn import_background_bytes(bytes: &[u8]) -> Result<ImportedBackground, CommandError> {
    let format = validate_background_bytes(bytes)?;
    let decoded = image::load_from_memory(bytes)
        .map_err(|error| CommandError::new("background_decode_failed", error.to_string()))?;
    let display = crop_center_to_16_by_9(&decoded).resize_exact(
        DISPLAY_WIDTH,
        DISPLAY_HEIGHT,
        FilterType::Lanczos3,
    );
    let root = strict_wallpaper_root()?;
    fs::create_dir_all(&root).map_err(|error| {
        CommandError::new("background_directory_create_failed", error.to_string())
    })?;

    let sequence = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| CommandError::new("background_timestamp_failed", error.to_string()))?
        .as_millis();
    let hash = stable_bytes_hash(bytes);
    let extension = extension_for(format);

    for attempt in 0..MAX_OUTPUT_PATH_ATTEMPTS {
        let (source_path, display_path) = output_paths(&root, sequence, hash, extension, attempt);
        let mut source_file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&source_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(CommandError::new(
                    "background_write_failed",
                    error.to_string(),
                ))
            }
        };
        if let Err(error) = source_file.write_all(bytes) {
            let _ = fs::remove_file(&source_path);
            return Err(CommandError::new(
                "background_write_failed",
                error.to_string(),
            ));
        }
        drop(source_file);

        let write_result = (|| -> Result<(), CommandError> {
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&display_path)
                .map_err(|error| CommandError::new("background_write_failed", error.to_string()))?;
            let mut encoder = JpegEncoder::new_with_quality(file, JPEG_QUALITY);
            encoder
                .encode_image(&display)
                .map_err(|error| CommandError::new("background_derive_failed", error.to_string()))
        })();
        match write_result {
            Ok(()) => {
                return Ok(ImportedBackground {
                    source_image: file_url(&source_path),
                    display_image: file_url(&display_path),
                })
            }
            Err(error) if display_path.exists() => {
                let _ = fs::remove_file(&source_path);
                let _ = fs::remove_file(&display_path);
                if error.message.contains("already exists") {
                    continue;
                }
                return Err(error);
            }
            Err(error) => {
                let _ = fs::remove_file(&source_path);
                return Err(error);
            }
        }
    }
    Err(CommandError::new(
        "background_name_collision",
        "无法为背景图分配唯一文件名。",
    ))
}

/// Reads one CodeSkin-managed wallpaper after resolving it inside the managed
/// wallpaper directory. This is intentionally not a general local-file reader:
/// persisted theme data must never turn into permission to read arbitrary paths.
pub(crate) fn read_managed_background_bytes(file_url: &str) -> Result<Vec<u8>, CommandError> {
    let root = strict_wallpaper_root()?;
    read_managed_background_bytes_from_root(file_url, &root)
}

fn read_managed_background_bytes_from_root(
    file_url: &str,
    root: &Path,
) -> Result<Vec<u8>, CommandError> {
    let path = local_file_url_path(file_url)
        .ok_or_else(|| CommandError::new("background_read_url_invalid", "背景图路径格式无效。"))?;
    let canonical_root = root.canonicalize().map_err(|error| {
        CommandError::new(
            "background_storage_unavailable",
            format!("无法访问 CodeSkin 壁纸目录：{error}"),
        )
    })?;
    let canonical_path = path.canonicalize().map_err(|error| {
        CommandError::new(
            "background_file_unavailable",
            format!("无法访问背景图：{error}"),
        )
    })?;
    if !canonical_path.starts_with(&canonical_root) || !canonical_path.is_file() {
        return Err(CommandError::new(
            "background_not_managed",
            "只允许读取 CodeSkin 管理的本地背景图。",
        ));
    }
    fs::read(&canonical_path).map_err(|error| {
        CommandError::new("background_read_failed", format!("读取背景图失败：{error}"))
    })
}

/// Creates a small, in-memory JPEG preview for the Tauri UI. The file URL is
/// only read after it has been canonicalized inside CodeSkin's wallpaper folder;
/// the WebView never gets a filesystem path or a broad local-file permission.
pub(crate) fn wallpaper_preview_data_url(background_image: Option<&str>) -> Option<String> {
    let root = strict_wallpaper_root().ok()?;
    background_image.and_then(|url| managed_preview_data_url_from_root(url, &root).ok())
}

fn managed_preview_data_url_from_root(file_url: &str, root: &Path) -> Result<String, CommandError> {
    let path = local_file_url_path(file_url).ok_or_else(|| {
        CommandError::new("background_preview_url_invalid", "背景预览路径格式无效。")
    })?;
    let canonical_root = root.canonicalize().map_err(|error| {
        CommandError::new(
            "background_storage_unavailable",
            format!("无法访问 CodeSkin 壁纸目录：{error}"),
        )
    })?;
    let canonical_path = path.canonicalize().map_err(|error| {
        CommandError::new(
            "background_file_unavailable",
            format!("无法访问背景预览图片：{error}"),
        )
    })?;
    if !canonical_path.starts_with(&canonical_root) || !canonical_path.is_file() {
        return Err(CommandError::new(
            "background_not_managed",
            "只允许预览 CodeSkin 管理的本地背景图。",
        ));
    }

    let bytes = fs::read(&canonical_path).map_err(|error| {
        CommandError::new(
            "background_read_failed",
            format!("读取背景预览失败：{error}"),
        )
    })?;
    if bytes.is_empty() || bytes.len() > MAX_BACKGROUND_BYTES {
        return Err(CommandError::new(
            "background_preview_size_invalid",
            "背景预览源文件为空或超过允许大小。",
        ));
    }
    if image::guess_format(&bytes).ok() != Some(ImageFormat::Jpeg) {
        return Err(CommandError::new(
            "background_preview_format_invalid",
            "背景预览只能来自 CodeSkin 派生的 JPEG。",
        ));
    }
    let preview = image::load_from_memory(&bytes)
        .map_err(|error| CommandError::new("background_decode_failed", error.to_string()))?
        .resize_to_fill(PREVIEW_WIDTH, PREVIEW_HEIGHT, FilterType::Triangle);
    let mut encoded = Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut encoded, PREVIEW_JPEG_QUALITY)
        .encode_image(&preview)
        .map_err(|error| {
            CommandError::new("background_preview_encode_failed", error.to_string())
        })?;
    let encoded = encoded.into_inner();
    if encoded.len() > MAX_PREVIEW_BYTES {
        return Err(CommandError::new(
            "background_preview_too_large",
            "生成的背景缩略图超过允许大小。",
        ));
    }
    Ok(format!(
        "data:image/jpeg;base64,{}",
        base64_encode(&encoded)
    ))
}

fn base64_encode(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = *chunk.get(1).unwrap_or(&0);
        let third = *chunk.get(2).unwrap_or(&0);
        output.push(BASE64_ALPHABET[(first >> 2) as usize] as char);
        output
            .push(BASE64_ALPHABET[(((first & 0b0000_0011) << 4) | (second >> 4)) as usize] as char);
        match chunk.len() {
            1 => output.push_str("=="),
            2 => {
                output.push(
                    BASE64_ALPHABET[(((second & 0b0000_1111) << 2) | (third >> 6)) as usize]
                        as char,
                );
                output.push('=');
            }
            _ => {
                output.push(
                    BASE64_ALPHABET[(((second & 0b0000_1111) << 2) | (third >> 6)) as usize]
                        as char,
                );
                output.push(BASE64_ALPHABET[(third & 0b0011_1111) as usize] as char);
            }
        }
    }
    output
}

/// Deletes only files under `%LOCALAPPDATA%\\CodeSkin\\wallpapers`.
pub fn delete_managed_background_files(
    urls: impl IntoIterator<Item = Option<String>>,
) -> Result<(), CommandError> {
    let root = strict_wallpaper_root()?;
    let canonical_root = root.canonicalize().unwrap_or(root);
    for url in urls.into_iter().flatten() {
        let Some(path) = local_file_url_path(&url) else {
            continue;
        };
        let Ok(candidate) = path.canonicalize() else {
            continue;
        };
        if !candidate.starts_with(&canonical_root) {
            continue;
        }
        if candidate.is_file() {
            fs::remove_file(&candidate).map_err(|error| {
                CommandError::new("background_delete_failed", error.to_string())
            })?;
        }
    }
    Ok(())
}

fn crop_center_to_16_by_9(image: &DynamicImage) -> DynamicImage {
    let (width, height) = image.dimensions();
    // Use the largest exact 16:9 rectangle with integer pixel dimensions. This
    // avoids a one-pixel aspect-ratio drift for inputs such as 900 x 1600.
    let scale = (width / 16).min(height / 9);
    if scale == 0 {
        return image.clone();
    }
    let crop_width = scale * 16;
    let crop_height = scale * 9;
    image.crop_imm(
        (width - crop_width) / 2,
        (height - crop_height) / 2,
        crop_width,
        crop_height,
    )
}

pub(crate) fn wallpaper_root() -> Result<PathBuf, CommandError> {
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            CommandError::new(
                "background_local_app_data_unavailable",
                "无法读取 LOCALAPPDATA，不能确定壁纸存储目录。",
            )
        })?;
    let root = PathBuf::from(local_app_data)
        .join("CodeSkin")
        .join("wallpapers");
    fs::create_dir_all(&root).map_err(|error| {
        CommandError::new("background_directory_create_failed", error.to_string())
    })?;
    Ok(root)
}

fn strict_wallpaper_root() -> Result<PathBuf, CommandError> {
    wallpaper_root()
}

fn output_paths(
    root: &Path,
    sequence: u128,
    hash: u64,
    extension: &str,
    attempt: u32,
) -> (PathBuf, PathBuf) {
    let suffix = if attempt == 0 {
        String::new()
    } else {
        format!("-{attempt}")
    };
    let stem = format!("background-{sequence}-{hash:016x}{suffix}");
    (
        root.join(format!("{stem}-source.{extension}")),
        root.join(format!("{stem}-display.jpg")),
    )
}

fn extension_for(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::WebP => "webp",
        _ => unreachable!(),
    }
}

fn file_url(path: &Path) -> String {
    format!("file:///{}", path.to_string_lossy().replace('\\', "/"))
}

fn local_file_url_path(url: &str) -> Option<PathBuf> {
    let path = url.strip_prefix("file:///")?;
    Some(PathBuf::from(path.replace('/', "\\")))
}

fn stable_bytes_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(FNV_OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

#[cfg(test)]
mod tests {
    use super::{
        crop_center_to_16_by_9, managed_preview_data_url_from_root, validate_background_bytes,
        DISPLAY_HEIGHT, DISPLAY_WIDTH,
    };
    use image::{
        codecs::jpeg::JpegEncoder, DynamicImage, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage,
    };
    use std::{
        fs,
        io::Cursor,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temporary_wallpaper_root() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir()
            .join(format!("codeskin-preview-test-{unique}"))
            .join("wallpapers");
        fs::create_dir_all(&root).expect("create test wallpaper root");
        root
    }

    fn file_url(path: &std::path::Path) -> String {
        format!("file:///{}", path.to_string_lossy().replace('\\', "/"))
    }

    #[test]
    fn managed_preview_uses_a_small_jpeg_data_url() {
        let root = temporary_wallpaper_root();
        let display = root.join("display.jpg");
        let image = RgbImage::from_pixel(1280, 720, Rgb([32, 64, 128]));
        JpegEncoder::new_with_quality(fs::File::create(&display).unwrap(), 90)
            .encode_image(&image)
            .expect("write fixture JPEG");

        let preview =
            managed_preview_data_url_from_root(&file_url(&display), &root).expect("create preview");
        assert!(preview.starts_with("data:image/jpeg;base64,/9j/"));
        assert!(preview.len() < 512 * 1024);

        fs::remove_dir_all(root.parent().expect("test root parent")).expect("remove test root");
    }

    #[test]
    fn managed_preview_rejects_a_file_outside_the_wallpaper_root() {
        let root = temporary_wallpaper_root();
        let outside = root.parent().expect("test root parent").join("outside.jpg");
        fs::write(&outside, [0xFF, 0xD8, 0xFF, 0xD9]).expect("write outside JPEG");

        let error = managed_preview_data_url_from_root(&file_url(&outside), &root)
            .expect_err("outside files must be rejected");
        assert_eq!(error.code, "background_not_managed");

        fs::remove_dir_all(root.parent().expect("test root parent")).expect("remove test root");
    }

    #[test]
    fn accepts_supported_image_formats() {
        let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(16, 9, Rgba([1, 2, 3, 255])));
        for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP] {
            let mut bytes = Vec::new();
            image
                .write_to(&mut Cursor::new(&mut bytes), format)
                .unwrap();
            assert_eq!(validate_background_bytes(&bytes).unwrap(), format);
        }
    }

    #[test]
    fn center_crops_portrait_input_to_display_ratio() {
        let image =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(900, 1600, Rgba([1, 2, 3, 255])));
        let cropped = crop_center_to_16_by_9(&image);
        assert_eq!(
            cropped.width() * DISPLAY_HEIGHT,
            cropped.height() * DISPLAY_WIDTH
        );
    }
}
