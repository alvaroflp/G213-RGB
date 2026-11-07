//! Offline product artwork; kept separate from the live lighting diagram.

use egui::{ColorImage, TextureHandle, TextureOptions, Ui, vec2};

pub struct ProductImage(TextureHandle);

impl ProductImage {
    pub fn load(ctx: &egui::Context) -> Self {
        let pixels = image::load_from_memory(include_bytes!("../assets/devices/g213.png"))
            .expect("bundled G213 product image must be a valid PNG")
            .into_rgba8();
        let size = [pixels.width() as usize, pixels.height() as usize];
        Self(ctx.load_texture(
            "g213-product",
            ColorImage::from_rgba_unmultiplied(size, &pixels),
            TextureOptions::LINEAR,
        ))
    }

    pub fn show(&self, ui: &mut Ui, width: f32) {
        // Crop only the transparent margins in the view, retaining the original asset.
        ui.add(
            egui::Image::new(&self.0)
                .uv(egui::Rect::from_min_max(
                    egui::pos2(0.0, 0.18),
                    egui::pos2(1.0, 0.83),
                ))
                .maintain_aspect_ratio(false)
                .fit_to_exact_size(vec2(width, width * 1112.0 / 1510.0 * 0.65))
                .alt_text(
                    "Logitech G213 Prodigy, teclado com apoio para as mãos e controles de mídia",
                ),
        );
    }
}
