//! Turns image files and pasted pictures into images for the AI chat. Pasted pictures
//! are shrunk until they fit; files go as they are, within the size limit.

use std::io::Cursor;
use std::path::Path;

use image::imageops::FilterType;
use image::RgbaImage;

use crate::domain::AiImage;
use crate::shared::fmt_bytes;

/// Largest single image sent.
pub(super) const MAX_IMAGE_BYTES: usize = 2_000_000;
/// Longest side a pasted picture starts at (what vision models work with best).
const MAX_SIDE: u32 = 1568;
/// Smallest longest side a pasted picture is shrunk to before giving up.
const MIN_SIDE: u32 = 400;

/// An image file as it is: PNG, JPEG, GIF or WebP, at most [`MAX_IMAGE_BYTES`].
pub(super) fn image_from_file(path: &Path) -> Result<AiImage, String> {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.display().to_string());
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_lowercase).unwrap_or_default();
    let media_type = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return Err(format!("{} is not a PNG, JPEG, GIF or WebP image", name)),
    };
    let data = std::fs::read(path).map_err(|e| format!("Could not read {}: {}", name, e))?;
    if data.len() > MAX_IMAGE_BYTES {
        let sizes = (fmt_bytes(data.len() as u64), fmt_bytes(MAX_IMAGE_BYTES as u64));
        return Err(format!("{} is {}; the limit is {}. Paste it as a screenshot instead: pasted images are shrunk to fit.", name, sizes.0, sizes.1));
    }
    Ok(AiImage { name, media_type: media_type.into(), data })
}

/// A pasted picture (`width` × `height` RGBA pixels) as PNG, shrunk until it fits.
pub(super) fn image_from_pixels((width, height): (u32, u32), rgba: Vec<u8>, name: String) -> Result<AiImage, String> {
    let img = RgbaImage::from_raw(width, height, rgba).ok_or("The pasted image could not be read")?;
    let mut side = MAX_SIDE;
    loop {
        let png = encode_png(&fit(&img, side))?;
        if png.len() <= MAX_IMAGE_BYTES {
            return Ok(AiImage { name, media_type: "image/png".into(), data: png });
        }
        if side <= MIN_SIDE {
            return Err("The pasted image is too detailed to send, even shrunk".into());
        }
        side = side * 3 / 4;
    }
}

/// `img` with its longest side at most `side`.
fn fit(img: &RgbaImage, side: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let longest = w.max(h);
    if longest <= side {
        return img.clone();
    }
    let scale = f64::from(side) / f64::from(longest);
    let size = |v: u32| ((f64::from(v) * scale).round() as u32).max(1);
    image::imageops::resize(img, size(w), size(h), FilterType::Triangle)
}

fn encode_png(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).map_err(|e| format!("Could not encode the image: {}", e))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::{image_from_file, image_from_pixels};

    #[test]
    fn big_pictures_are_shrunk_to_the_longest_side() {
        let img = image_from_pixels((3000, 1500), vec![200; 3000 * 1500 * 4], "pasted-1.png".into()).expect("fits");
        let decoded = image::load_from_memory(&img.data).expect("valid png");
        assert_eq!((decoded.width(), decoded.height()), (1568, 784));
        assert_eq!(img.media_type, "image/png");
    }

    #[test]
    fn wrong_pixel_count_is_an_error() {
        assert!(image_from_pixels((10, 10), vec![0; 7], "x.png".into()).is_err());
    }

    #[test]
    fn files_need_an_image_extension() {
        assert!(image_from_file(std::path::Path::new("notes.txt")).is_err());
    }
}
