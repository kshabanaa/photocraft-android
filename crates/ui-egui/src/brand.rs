//! The brand mark at the left of the title bar, and which brand a build carries.
//!
//! Official builds show the PhotoCraft app icon there (the kitsune, `assets/app-icon/`, see its
//! README), where Photoshop shows its "Ps" tile. The PNG carries the icon's rounded corners; it is
//! decoded once per context into a mipmapped texture, so it stays crisp at the title bar's size on
//! any display scale.
//!
//! Independent builds of a fork (the `unbranded` cargo feature, on for `apps/photocraft-android`)
//! carry none of the ArtCraft marks (`docs/brand/LICENSE-brand.txt`, "Forks and modified
//! versions"): the kitsune is not compiled in, the title bar shows a neutral mark drawn from the
//! theme, and the ArtCraft community links give way to a plain-text credit ([`crate::links`]).

#[cfg(not(feature = "unbranded"))]
use egui::{Color32, Context, TextureHandle, TextureOptions, pos2};
use egui::{Id, Rect, Ui};
#[cfg(not(feature = "unbranded"))]
use photocraft_codecs::{ChannelLayout, SampleType};

/// Which brand a build carries: [`Brand::BUILD`], kept in [`crate::PhotocraftApp::brand`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Brand {
    /// An official build: the kitsune in the title bar, and the ArtCraft community links (Discord,
    /// getartcraft.com) in Help, About, the title bar and the start screen.
    Official,
    /// An independent build of a fork: no ArtCraft marks, a neutral title-bar mark, and a
    /// plain-text "based on PhotoCraft by the ArtCraft team" credit instead of the community links.
    Independent,
}

impl Brand {
    /// This build's brand: [`Brand::Independent`] with the `unbranded` feature, else [`Brand::Official`].
    pub const BUILD: Brand = if cfg!(feature = "unbranded") { Brand::Independent } else { Brand::Official };

    /// Does this brand show the ArtCraft marks? Never in an `unbranded` build, whatever the value:
    /// those builds leave the marks out entirely.
    pub const fn is_official(self) -> bool {
        matches!(self, Brand::Official) && !cfg!(feature = "unbranded")
    }
}

/// 128 px: sharp at 20 pt on a 3× display, and the mipmaps keep it clean at 1×.
#[cfg(not(feature = "unbranded"))]
const ICON_PNG: &[u8] = include_bytes!("../../../assets/app-icon/hicolor/128x128/apps/ai.storyteller.photocraft.png");

/// The icon's pixels (one transparent pixel if it couldn't be decoded, which a test rules out).
#[cfg(not(feature = "unbranded"))]
fn decode() -> egui::ColorImage {
    let rgba = photocraft_codecs::decode(ICON_PNG).ok().map(|img| img.convert(ChannelLayout::Rgba, SampleType::U8));
    match rgba {
        Some(img) if img.data().len() == img.width() as usize * img.height() as usize * 4 => {
            egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.data())
        }
        _ => egui::ColorImage::filled([1, 1], Color32::TRANSPARENT),
    }
}

/// The context-data key of the icon texture.
#[cfg(not(feature = "unbranded"))]
fn texture_id() -> Id {
    Id::new("photocraft-brand-mark")
}

/// The icon texture, uploaded on first use and kept in the context.
#[cfg(not(feature = "unbranded"))]
fn texture(ctx: &Context) -> TextureHandle {
    let id = texture_id();
    if let Some(tex) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return tex;
    }
    let options = TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..TextureOptions::LINEAR };
    let tex = ctx.load_texture("photocraft-brand-mark", decode(), options);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    tex
}

/// Where the mark was painted this frame (tests and screenshots).
fn rect_id() -> Id {
    Id::new("photocraft-brand-mark-rect")
}

/// Paint the mark into `r` (square): the kitsune for an official build, the neutral mark otherwise.
pub fn paint_mark(ui: &Ui, r: Rect, brand: Brand) {
    if !paint_kitsune(ui, r, brand) {
        paint_neutral(ui, r);
    }
    ui.ctx().data_mut(|d| d.insert_temp(rect_id(), r));
}

/// Paint the kitsune if `brand` is official; false when it isn't.
#[cfg(not(feature = "unbranded"))]
fn paint_kitsune(ui: &Ui, r: Rect, brand: Brand) -> bool {
    if brand.is_official() {
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        ui.painter().image(texture(ui.ctx()).id(), r, uv, Color32::WHITE);
    }
    brand.is_official()
}

/// Unbranded builds have no kitsune to paint.
#[cfg(feature = "unbranded")]
fn paint_kitsune(_: &Ui, _: Rect, _: Brand) -> bool {
    false
}

/// The independent builds' mark: a picture glyph on a tile in the theme's primary-button colours,
/// with no artwork of its own.
fn paint_neutral(ui: &Ui, r: Rect) {
    let t = crate::theme::Tokens::get(ui.ctx());
    ui.painter().rect_filled(r, t.radius, t.primary_bg);
    crate::icons::paint(ui, r, "image", r.width() * 0.7, t.primary_text);
}

/// The rect the mark was last painted into.
#[cfg(test)]
pub fn mark_rect(ctx: &egui::Context) -> Option<Rect> {
    ctx.data(|d| d.get_temp(rect_id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The title bar as the app draws it (not the system's), one frame: the 128 px textures
    /// uploaded in it.
    fn title_bar_uploads(ctx: &egui::Context, app: &mut crate::PhotocraftApp) -> usize {
        // The system title bar hides the in-app mark (the OS draws its own icon), so paint the
        // app-drawn bar here.
        app.custom_titlebar = true;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| crate::panels::title_bar(app, ui));
        let uploads = out.textures_delta.set.values().flat_map(|d| d.iter()).filter(|d| d.image.size() == [128, 128]).count();
        out.textures_delta.clear();
        uploads
    }

    fn app(ctx: &egui::Context) -> crate::PhotocraftApp {
        crate::PhotocraftApp::setup_context(ctx, crate::theme::ThemeKind::ALL[0]);
        crate::PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default())
    }

    #[test]
    fn the_build_brand_follows_the_unbranded_feature() {
        assert_eq!(Brand::BUILD == Brand::Independent, cfg!(feature = "unbranded"));
        assert_eq!(Brand::BUILD.is_official(), !cfg!(feature = "unbranded"));
        assert!(!Brand::Independent.is_official());
        // An unbranded build can't show the marks, even when asked to.
        assert_eq!(Brand::Official.is_official(), !cfg!(feature = "unbranded"));
        assert_eq!(app(&egui::Context::default()).brand, Brand::BUILD, "a new app carries the build's brand");
    }

    #[cfg(not(feature = "unbranded"))]
    #[test]
    fn the_icon_decodes_to_a_rounded_128_px_tile() {
        let img = decode();
        assert_eq!(img.size, [128, 128]);
        // Rounded corners: the corner pixel is transparent, the centre opaque.
        assert_eq!(img.pixels[0].a(), 0);
        assert_eq!(img.pixels[64 * 128 + 64].a(), 255);
    }

    #[cfg(not(feature = "unbranded"))]
    #[test]
    fn the_title_bar_paints_the_icon_from_one_cached_texture() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        app.brand = Brand::Official;
        assert_eq!(title_bar_uploads(&ctx, &mut app), 1, "uploaded on first use");
        assert_eq!(title_bar_uploads(&ctx, &mut app), 0, "then reused");
        assert!(mark_rect(&ctx).is_some());
    }

    #[test]
    fn independent_builds_paint_a_neutral_mark_without_the_kitsune() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        app.brand = Brand::Independent;
        assert_eq!(title_bar_uploads(&ctx, &mut app), 0, "no icon texture");
        #[cfg(not(feature = "unbranded"))]
        assert!(ctx.data(|d| d.get_temp::<egui::TextureHandle>(texture_id())).is_none(), "the kitsune is never loaded");
        assert!(mark_rect(&ctx).is_some(), "a mark still holds the icon's place");
    }
}
