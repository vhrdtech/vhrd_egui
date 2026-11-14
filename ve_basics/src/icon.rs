//! Decode a PNG into an egui texture, for an app's own icon (BAS-9).

use egui::{ColorImage, Context, TextureHandle, TextureOptions};

/// Decodes `png_bytes` and loads it as a texture named `name` (egui's texture debug name, e.g. `"app-icon"`);
/// `None` if the bytes are not a valid PNG. Pure-Rust decode, the same on native and the web, so an app's icon
/// PNG can feed both its window icon and its top bar's brand strip ([`crate::AppMenu`], `ve_theme::topbar`).
pub fn load_icon(ctx: &Context, name: &str, png_bytes: &[u8]) -> Option<TextureHandle> {
    let rgba = image::load_from_memory_with_format(png_bytes, image::ImageFormat::Png)
        .ok()?
        .into_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture(name, color, TextureOptions::LINEAR))
}
