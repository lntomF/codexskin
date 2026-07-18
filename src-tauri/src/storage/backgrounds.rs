use crate::error::CommandError;
use image::ImageFormat;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_BACKGROUND_BYTES: usize = 12 * 1024 * 1024;
const MAX_BACKGROUND_DIMENSION: u32 = 8192;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const MAX_OUTPUT_PATH_ATTEMPTS: u32 = 128;

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

pub fn import_background_bytes(bytes: &[u8]) -> Result<String, CommandError> {
    let root = strict_wallpaper_root()?;
    import_background_bytes_to_root(bytes, &root)
}

fn import_background_bytes_to_root(bytes: &[u8], root: &Path) -> Result<String, CommandError> {
    let format = validate_background_bytes(bytes)?;
    let sequence = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| CommandError::new("background_timestamp_failed", error.to_string()))?
        .as_millis();
    import_background_bytes_to_root_with_name_parts(
        bytes,
        root,
        sequence,
        format,
        stable_bytes_hash(bytes),
    )
}

fn import_background_bytes_to_root_with_name_parts(
    bytes: &[u8],
    root: &Path,
    sequence: u128,
    format: ImageFormat,
    bytes_hash: u64,
) -> Result<String, CommandError> {
    import_validated_background_bytes_to_root(bytes, root, sequence, format, bytes_hash)
}

fn import_validated_background_bytes_to_root(
    bytes: &[u8],
    root: &Path,
    sequence: u128,
    format: ImageFormat,
    bytes_hash: u64,
) -> Result<String, CommandError> {
    fs::create_dir_all(root).map_err(|error| {
        CommandError::new("background_directory_create_failed", error.to_string())
    })?;

    for attempt in 0..MAX_OUTPUT_PATH_ATTEMPTS {
        let path = wallpaper_output_path(root, sequence, format, bytes_hash, attempt);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(bytes) {
                    let _ = fs::remove_file(&path);
                    return Err(CommandError::new(
                        "background_write_failed",
                        error.to_string(),
                    ));
                }

                let normalized = path.to_string_lossy().replace('\\', "/");
                return Ok(format!("file:///{normalized}"));
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(CommandError::new(
                    "background_write_failed",
                    error.to_string(),
                ))
            }
        }
    }

    Err(CommandError::new(
        "background_name_collision",
        "无法为背景图分配唯一文件名。",
    ))
}

fn strict_wallpaper_root() -> Result<PathBuf, CommandError> {
    wallpaper_directory_from_local_app_data(std::env::var_os("LOCALAPPDATA"))
}
fn wallpaper_directory_from_local_app_data(
    local_app_data: Option<std::ffi::OsString>,
) -> Result<PathBuf, CommandError> {
    let local_app_data = local_app_data
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            CommandError::new(
                "background_local_app_data_unavailable",
                "无法读取 LOCALAPPDATA，不能确定壁纸存储目录。",
            )
        })?;
    Ok(PathBuf::from(local_app_data)
        .join("CodeSkin")
        .join("wallpapers"))
}

fn wallpaper_output_path(
    directory: &Path,
    sequence: u128,
    format: ImageFormat,
    bytes_hash: u64,
    attempt: u32,
) -> PathBuf {
    let collision_suffix = if attempt == 0 {
        String::new()
    } else {
        format!("-{attempt}")
    };
    directory.join(format!(
        "wallpaper-{sequence}-{bytes_hash:016x}{collision_suffix}.{}",
        extension_for(format)
    ))
}

fn extension_for(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::WebP => "webp",
        _ => unreachable!("validated formats are exhaustive"),
    }
}

fn stable_bytes_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(FNV_OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

#[cfg(test)]
mod tests {
    use super::{
        import_background_bytes_to_root, import_background_bytes_to_root_with_name_parts,
        strict_wallpaper_root, validate_background_bytes, wallpaper_directory_from_local_app_data,
        wallpaper_output_path, MAX_BACKGROUND_DIMENSION,
    };
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use std::{
        ffi::OsString,
        fs,
        io::Cursor,
        path::{Path, PathBuf},
        process,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEST_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn image_bytes(format: ImageFormat) -> Vec<u8> {
        let image = RgbaImage::from_pixel(3, 2, Rgba([12, 34, 56, 255]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), format)
            .expect("encode test image");
        bytes
    }

    fn directory_entries(directory: &Path) -> Vec<PathBuf> {
        let mut paths = fs::read_dir(directory)
            .expect("read isolated wallpaper directory")
            .map(|entry| {
                entry
                    .expect("read isolated wallpaper directory entry")
                    .path()
            })
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }

    fn file_path_from_url(file_url: &str) -> PathBuf {
        PathBuf::from(
            file_url
                .strip_prefix("file:///")
                .expect("background import returns a file URL"),
        )
    }

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let sequence = TEST_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock must be after the Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "codeskin-background-tests-{}-{timestamp}-{sequence}",
                process::id()
            ));
            fs::create_dir(&path).expect("create unique isolated wallpaper directory");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn rejects_large_background_input_with_the_existing_code() {
        let directory = TestDirectory::new();
        let too_large = vec![0_u8; 12 * 1024 * 1024 + 1];

        let error = validate_background_bytes(&too_large).expect_err("oversized input must fail");
        assert_eq!(error.code, "background_too_large");

        let error = import_background_bytes_to_root(&too_large, directory.path())
            .expect_err("oversized import must fail");
        assert_eq!(error.code, "background_too_large");
        assert!(directory_entries(directory.path()).is_empty());
    }

    #[test]
    fn rejects_dimensions_larger_than_the_limit() {
        let directory = TestDirectory::new();
        let oversized = RgbaImage::new(MAX_BACKGROUND_DIMENSION + 1, 1);
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(oversized)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .expect("encode oversized PNG");

        let error = validate_background_bytes(&bytes).expect_err("oversized dimensions must fail");
        assert_eq!(error.code, "background_dimensions_too_large");

        let error = import_background_bytes_to_root(&bytes, directory.path())
            .expect_err("oversized dimension import must fail");
        assert_eq!(error.code, "background_dimensions_too_large");
        assert!(directory_entries(directory.path()).is_empty());
    }

    #[test]
    fn malformed_backgrounds_fail_without_creating_isolated_files() {
        let directory = TestDirectory::new();
        let truncated_png = b"\x89PNG\r\n\x1a\n";
        assert_eq!(
            image::guess_format(truncated_png).expect("PNG signature must be recognized"),
            ImageFormat::Png
        );

        for bytes in [b"not an image".as_slice(), truncated_png.as_slice()] {
            let error = import_background_bytes_to_root(bytes, directory.path())
                .expect_err("malformed image import must fail");
            assert_eq!(error.code, "background_decode_failed");
            assert!(directory_entries(directory.path()).is_empty());
        }
    }

    #[test]
    fn imports_recognized_formats_into_an_isolated_directory_with_matching_extensions() {
        let directory = TestDirectory::new();

        for (format, extension) in [
            (ImageFormat::Png, "png"),
            (ImageFormat::Jpeg, "jpg"),
            (ImageFormat::WebP, "webp"),
        ] {
            let bytes = image_bytes(format);
            assert_eq!(
                validate_background_bytes(&bytes).expect("recognized format"),
                format
            );

            let file_url = import_background_bytes_to_root(&bytes, directory.path())
                .expect("import background into isolated directory");
            let path = file_path_from_url(&file_url);

            assert!(path.starts_with(directory.path()));
            assert_eq!(path.parent(), Some(directory.path()));
            assert_eq!(
                path.extension().and_then(|extension| extension.to_str()),
                Some(extension)
            );
            assert_eq!(
                fs::read(path).expect("read isolated imported background"),
                bytes
            );
        }
    }

    #[test]
    fn retries_when_the_preferred_output_path_is_already_occupied() {
        let directory = TestDirectory::new();
        let bytes = image_bytes(ImageFormat::Png);
        let sequence = 42;
        let bytes_hash = 0x1111;
        let preferred =
            wallpaper_output_path(directory.path(), sequence, ImageFormat::Png, bytes_hash, 0);
        fs::write(&preferred, b"existing wallpaper").expect("occupy preferred wallpaper path");

        let file_url = import_background_bytes_to_root_with_name_parts(
            &bytes,
            directory.path(),
            sequence,
            ImageFormat::Png,
            bytes_hash,
        )
        .expect("retry an occupied wallpaper path");
        let imported = file_path_from_url(&file_url);
        let retry_path =
            wallpaper_output_path(directory.path(), sequence, ImageFormat::Png, bytes_hash, 1);

        assert_eq!(
            fs::read(&preferred).expect("read original wallpaper"),
            b"existing wallpaper"
        );
        assert_eq!(imported, retry_path);
        assert_eq!(fs::read(retry_path).expect("read retried wallpaper"), bytes);
    }

    #[test]
    fn does_not_fall_back_when_local_app_data_is_unavailable() {
        let error = wallpaper_directory_from_local_app_data(None)
            .expect_err("wallpapers require LOCALAPPDATA");
        assert_eq!(error.code, "background_local_app_data_unavailable");
    }

    #[test]
    fn strict_wallpaper_root_uses_the_current_local_app_data_without_mutating_environment() {
        let current_local_app_data =
            std::env::var_os("LOCALAPPDATA").filter(|value| !value.is_empty());

        match (current_local_app_data, strict_wallpaper_root()) {
            (Some(local_app_data), Ok(root)) => {
                assert_eq!(
                    root,
                    PathBuf::from(local_app_data)
                        .join("CodeSkin")
                        .join("wallpapers")
                );
            }
            (None, Err(error)) => assert_eq!(error.code, "background_local_app_data_unavailable"),
            (Some(_), Err(error)) => panic!("LOCALAPPDATA was available: {error:?}"),
            (None, Ok(root)) => panic!("missing LOCALAPPDATA unexpectedly produced {root:?}"),
        }
    }

    #[test]
    fn builds_unique_generated_names_for_same_timestamp_and_format() {
        let root = PathBuf::from("C:/LocalAppData/CodeSkin/wallpapers");
        let first = wallpaper_output_path(&root, 42, ImageFormat::WebP, 0x1111, 0);
        let second = wallpaper_output_path(&root, 42, ImageFormat::WebP, 0x2222, 0);

        assert_ne!(first, second);
        assert_eq!(first, root.join("wallpaper-42-0000000000001111.webp"));
        assert_eq!(second, root.join("wallpaper-42-0000000000002222.webp"));
    }

    #[test]
    fn builds_the_strict_wallpaper_directory_from_local_app_data() {
        assert_eq!(
            wallpaper_directory_from_local_app_data(Some(OsString::from("C:/LocalAppData")))
                .expect("derive strict wallpaper directory"),
            Path::new("C:/LocalAppData")
                .join("CodeSkin")
                .join("wallpapers")
        );
    }
}
