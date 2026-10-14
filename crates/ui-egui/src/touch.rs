//! Touch screens: the controls where a finger's drag belongs to the control in every direction (a
//! colour field, an angle dial), so a vertical swipe that starts on one must not scroll the panel
//! or dialog around it. A touch shell (the Android app) reads [`pads`] when it tells a swipe from
//! a drag; on desktop and the web nothing reads it.

/// Whether the window is a short screen (a phone in landscape), under
/// [`crate::prefs_ui::SHORT_HEIGHT`] points of usable height.
pub fn short_screen(ctx: &egui::Context) -> bool {
    ctx.content_rect().height() < crate::prefs_ui::SHORT_HEIGHT
}

/// Whether the window is a small screen: short (a phone in landscape) or narrow (a phone in
/// portrait). This is a size class, like a website's breakpoint, not a platform check: a phone, a
/// desktop window made that small and the web build in a phone's browser all get the small-screen
/// layout (dialogs that scroll inside the screen, the compact toolbar and dock), and the offscreen
/// snapshot tool renders it on any machine.
pub fn small_screen(ctx: &egui::Context) -> bool {
    let room = ctx.content_rect();
    room.height() < crate::prefs_ui::SHORT_HEIGHT || room.width() < crate::prefs_ui::NARROW_WIDTH
}

/// Floating panels (Brush Settings, Glyphs, the Gradient Editor, ...) that fit a short screen.
pub trait FitScreen {
    /// On a short screen, a window taller than the screen scrolls as a whole instead of running off
    /// its bottom edge (egui keeps a window's position on screen, not its content). Elsewhere it is
    /// unchanged.
    fn fit_screen(self, ctx: &egui::Context) -> Self;
}

impl FitScreen for egui::Window<'_> {
    fn fit_screen(self, ctx: &egui::Context) -> Self {
        if short_screen(ctx) { self.vscroll(true).max_height(ctx.content_rect().height()) } else { self }
    }
}

#[derive(Clone, Default)]
struct Pads {
    /// The frame (`Context::cumulative_frame_nr`) the rectangles were drawn in.
    frame: u64,
    rects: Vec<egui::Rect>,
}

fn key() -> egui::Id {
    egui::Id::new("touch-pads")
}

/// Marks `rect` (screen points) as a control that takes drags in both directions, for this frame.
pub fn pad(ctx: &egui::Context, rect: egui::Rect) {
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|d| {
        let pads = d.get_temp_mut_or_default::<Pads>(key());
        if pads.frame != frame {
            pads.frame = frame;
            pads.rects.clear();
        }
        pads.rects.push(rect);
    });
}

/// The controls marked with [`pad`] in the last frame drawn (between frames, or during the next).
pub fn pads(ctx: &egui::Context) -> Vec<egui::Rect> {
    let frame = ctx.cumulative_frame_nr();
    ctx.data(|d| d.get_temp::<Pads>(key())).filter(|p| frame.saturating_sub(p.frame) <= 1).map(|p| p.rects).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pads_are_the_last_frames_only() {
        let ctx = egui::Context::default();
        let field = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(100.0, 80.0));
        let frame = |f: &mut dyn FnMut(&mut egui::Ui)| ctx.run_ui(Default::default(), |ui| f(ui)).textures_delta.clear();
        frame(&mut |ui| pad(ui.ctx(), field));
        // Between frames (where a shell filters input) and during the next one.
        assert_eq!(pads(&ctx), [field]);
        frame(&mut |ui| assert_eq!(pads(ui.ctx()), [field]));
        // A frame that drew no pad leaves none behind.
        assert!(pads(&ctx).is_empty());
    }
}
