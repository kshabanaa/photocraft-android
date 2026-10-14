//! Touch screens: the controls where a finger's drag belongs to the control in every direction (a
//! colour field, an angle dial), so a vertical swipe that starts on one must not scroll the panel
//! or dialog around it. A touch shell (the Android app) reads [`pads`] when it tells a swipe from
//! a drag; on desktop and the web nothing reads it.

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
