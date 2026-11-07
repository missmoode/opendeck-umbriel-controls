use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{DynamicImage, ImageFormat, RgbaImage, imageops};
use std::{io::Cursor, path::Path};

const SIZE: u32 = 64;
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// Normalise a local bitmap/SVG to a transparent 64×64 PNG data URL.
pub fn png_data_url(path: &Path) -> Option<String> {
	let metadata = std::fs::metadata(path).ok()?;
	if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
		return None;
	}
	let bytes = std::fs::read(path).ok()?;
	let extension = path.extension()?.to_str()?.to_ascii_lowercase();

	let png = match extension.as_str() {
		"svg" => svg_to_png(&bytes, path.parent())?,
		"png" | "jpg" | "jpeg" | "webp" => raster_to_png(&bytes)?,
		_ => return None, // XPM and other uncommon formats can be added later.
	};

	Some(format!("data:image/png;base64,{}", STANDARD.encode(png)))
}

fn raster_to_png(bytes: &[u8]) -> Option<Vec<u8>> {
	let decoded = image::load_from_memory(bytes).ok()?;
	let resized = decoded.thumbnail(SIZE, SIZE).to_rgba8();
	let mut canvas = RgbaImage::new(SIZE, SIZE);
	let x = (SIZE - resized.width()) / 2;
	let y = (SIZE - resized.height()) / 2;
	imageops::overlay(&mut canvas, &resized, i64::from(x), i64::from(y));

	let mut bytes = Cursor::new(Vec::new());
	DynamicImage::ImageRgba8(canvas)
		.write_to(&mut bytes, ImageFormat::Png)
		.ok()?;
	Some(bytes.into_inner())
}

fn svg_to_png(bytes: &[u8], directory: Option<&Path>) -> Option<Vec<u8>> {
	let mut options = resvg::usvg::Options {
		resources_dir: directory.map(Path::to_path_buf),
		..Default::default()
	};
	options.fontdb_mut().load_system_fonts();
	let tree = resvg::usvg::Tree::from_data(bytes, &options).ok()?;

	let width = tree.size().width();
	let height = tree.size().height();
	if width <= 0.0 || height <= 0.0 {
		return None;
	}
	let scale = (SIZE as f32 / width).min(SIZE as f32 / height);
	let x = (SIZE as f32 - width * scale) / 2.0;
	let y = (SIZE as f32 - height * scale) / 2.0;
	let transform = resvg::tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, x, y);

	let mut pixmap = resvg::tiny_skia::Pixmap::new(SIZE, SIZE)?;
	resvg::render(&tree, transform, &mut pixmap.as_mut());
	pixmap.encode_png().ok()
}

#[cfg(test)]
mod tests {
	use super::*;
	use image::Rgba;

	#[test]
	fn svg_is_scaled_into_a_transparent_64px_canvas() {
		let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"
            width="24" height="12"><rect width="24" height="12" fill="red"/></svg>"#;
		let png = svg_to_png(svg, None).unwrap();
		let image = image::load_from_memory(&png).unwrap().to_rgba8();
		assert_eq!(image.dimensions(), (64, 64));
		assert_eq!(image.get_pixel(32, 32)[3], 255);
		assert_eq!(image.get_pixel(32, 0)[3], 0);
	}

	#[test]
	fn bitmap_is_centered_with_transparent_padding() {
		let mut source = Cursor::new(Vec::new());
		let source_image = RgbaImage::from_pixel(12, 6, Rgba([255, 0, 0, 255]));
		DynamicImage::ImageRgba8(source_image)
			.write_to(&mut source, ImageFormat::Png)
			.unwrap();
		let png = raster_to_png(&source.into_inner()).unwrap();
		let image = image::load_from_memory(&png).unwrap().to_rgba8();
		assert_eq!(image.dimensions(), (64, 64));
		assert_eq!(*image.get_pixel(32, 32), Rgba([255, 0, 0, 255]));
		assert_eq!(image.get_pixel(32, 0)[3], 0);
	}
}
