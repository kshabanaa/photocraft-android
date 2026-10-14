//! Telling a finger's swipe from a drag. PhotoCraft's controls are desktop controls: a slider,
//! number field, scrubby label or list row takes every drag that starts on it. On a phone most of
//! a dialog or panel is such controls, so a swipe meant to scroll it moved a value instead.
//!
//! A finger that comes down on a small control is held back until it moves: a mostly vertical
//! swipe scrolls whatever is under the finger, as on any phone; a sideways move, or holding still
//! for [`HOLD`] seconds, hands the press on unchanged, so sliders, scrubbing and long-press drags
//! still work. A finger on anything else (the canvas, a colour field, a preview, a splitter, a
//! scroll bar) gets its press at once.

use egui::{Event, PointerButton, Pos2, TouchPhase};

/// How far (points) a finger moves before its gesture is decided.
pub const SLOP: f32 = 8.0;
/// How long (seconds) a finger holds still before its press goes to the control under it.
pub const HOLD: f64 = 0.3;
/// Below this speed (points per second) a fling stops.
const FLING_MIN: f32 = 40.0;
/// How quickly a fling slows (per second).
const FLING_DECAY: f32 = 4.0;

/// Whether a swipe that starts on an interactive widget with this hit rectangle may scroll: a
/// small control (a slider, field, button or list row). Big pads (the canvas, a colour field, a
/// preview), thin splitters and tall strips (a scroll bar, a hue strip) take every drag.
pub fn swipe_may_scroll(rect: egui::Rect) -> bool {
    (12.0..=48.0).contains(&rect.height()) && rect.width() >= rect.height() * 0.6
}

#[derive(Default)]
enum State {
    #[default]
    Idle,
    /// Down on a small control and not moved past [`SLOP`] yet: its press is held back.
    Pending { origin: Pos2, since: f64, held: Vec<Event> },
    /// Swiping: the finger scrolls. `velocity` is points per second, for the fling.
    Scroll { last: Pos2, time: f64, velocity: f32 },
    /// The finger's events go through unchanged until it lifts.
    Direct,
}

/// One finger's gesture, frame to frame.
#[derive(Default)]
pub struct Gestures {
    state: State,
    /// Vertical scroll (points, positive moves the content down) for this frame.
    pub scroll: f32,
    /// A fling's speed after the finger lifted (points per second), and when it last moved.
    fling: f32,
    fling_time: f64,
    /// The finger's `PointerGone`, held while a fling scrolls what was under it.
    gone: bool,
}

impl Gestures {
    /// Filters one frame of input (`now`: its time, seconds). `may_scroll(pos)` says whether a
    /// swipe starting at `pos` may scroll. Returns the events egui should see; what the finger
    /// scrolled this frame is added to `scroll`.
    pub fn filter(&mut self, events: Vec<Event>, now: f64, may_scroll: impl Fn(Pos2) -> bool) -> Vec<Event> {
        let mut out = Vec::with_capacity(events.len());
        // egui-winit sends a finger's Touch event just before the press it simulates; a mouse or
        // a hardware button press has none and always goes through.
        let mut touch_started = false;
        let mut frame_dy = 0.0;
        for event in events {
            if matches!(event, Event::Touch { phase: TouchPhase::Start, .. }) {
                touch_started = true;
            }
            self.state = match (std::mem::take(&mut self.state), &event) {
                (State::Idle, Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, .. }) if touch_started => {
                    touch_started = false;
                    // A finger stops a fling.
                    self.fling = 0.0;
                    if may_scroll(*pos) {
                        State::Pending { origin: *pos, since: now, held: vec![event] }
                    } else {
                        out.push(event);
                        State::Direct
                    }
                }
                (State::Pending { origin, since, mut held }, Event::PointerMoved(pos)) => {
                    let moved = *pos - origin;
                    // The pointer follows the finger either way, so hover and the scroll area
                    // under it are where the finger is.
                    out.push(event.clone());
                    if moved.length() < SLOP {
                        State::Pending { origin, since, held }
                    } else if moved.y.abs() > moved.x.abs() {
                        frame_dy += moved.y;
                        State::Scroll { last: *pos, time: now, velocity: 0.0 }
                    } else {
                        let at = out.len() - 1;
                        out.splice(at..at, held.drain(..));
                        State::Direct
                    }
                }
                (State::Pending { mut held, .. }, Event::PointerButton { pressed: false, .. }) => {
                    // A tap: the control gets its press and release.
                    out.append(&mut held);
                    out.push(event);
                    State::Idle
                }
                (State::Scroll { last, time, velocity }, Event::PointerMoved(pos)) => {
                    frame_dy += pos.y - last.y;
                    out.push(event.clone());
                    State::Scroll { last: *pos, time, velocity }
                }
                (State::Scroll { velocity, .. }, Event::PointerButton { pressed: false, .. }) => {
                    // The control never saw a press, so it gets no release either.
                    self.fling = velocity;
                    self.fling_time = now;
                    State::Idle
                }
                (State::Idle, Event::PointerGone) if self.fling != 0.0 => {
                    self.gone = true;
                    State::Idle
                }
                (State::Direct, Event::PointerButton { pressed: false, .. }) => {
                    out.push(event);
                    State::Idle
                }
                (state, _) => {
                    out.push(event);
                    state
                }
            };
        }
        self.scroll += frame_dy;
        match &mut self.state {
            State::Scroll { time, velocity, .. } => {
                let dt = (now - *time) as f32;
                if dt > 0.0 {
                    *velocity = *velocity * 0.4 + frame_dy / dt * 0.6;
                    *time = now;
                }
            }
            State::Pending { since, held, .. } if now - *since >= HOLD => {
                // Held still: a long press belongs to the control (a row to drag, a menu to hold).
                out.append(held);
                self.state = State::Direct;
            }
            _ => {}
        }
        if matches!(self.state, State::Idle) && self.fling != 0.0 {
            let dt = ((now - self.fling_time) as f32).clamp(0.0, 0.1);
            self.fling_time = now;
            self.scroll += self.fling * dt;
            self.fling *= (-FLING_DECAY * dt).exp();
            if self.fling.abs() < FLING_MIN {
                self.fling = 0.0;
                if std::mem::take(&mut self.gone) {
                    out.push(Event::PointerGone);
                }
            }
        }
        out
    }

    /// Whether a later frame has work to do without new input: a press waiting for [`HOLD`] or a
    /// fling still moving.
    pub fn busy(&self) -> bool {
        matches!(self.state, State::Pending { .. }) || self.fling != 0.0
    }

    /// Takes this frame's scroll.
    pub fn take_scroll(&mut self) -> f32 {
        std::mem::take(&mut self.scroll)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Modifiers, TouchDeviceId, TouchId, pos2};

    fn down(at: Pos2) -> Vec<Event> {
        vec![
            Event::Touch { device_id: TouchDeviceId(1), id: TouchId(0), phase: TouchPhase::Start, pos: at, force: None },
            Event::PointerMoved(at),
            Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE },
        ]
    }

    fn up(at: Pos2) -> Vec<Event> {
        vec![
            Event::Touch { device_id: TouchDeviceId(1), id: TouchId(0), phase: TouchPhase::End, pos: at, force: None },
            Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE },
            Event::PointerGone,
        ]
    }

    fn presses(events: &[Event]) -> Vec<bool> {
        events.iter().filter_map(|e| if let Event::PointerButton { pressed, .. } = e { Some(*pressed) } else { None }).collect()
    }

    #[test]
    fn a_tap_on_a_small_control_still_clicks_it() {
        let mut g = Gestures::default();
        assert!(presses(&g.filter(down(pos2(100.0, 100.0)), 0.0, |_| true)).is_empty());
        assert_eq!(presses(&g.filter(up(pos2(101.0, 101.0)), 0.05, |_| true)), [true, false]);
        assert_eq!(g.take_scroll(), 0.0);
    }

    #[test]
    fn a_vertical_swipe_scrolls_and_never_presses() {
        let mut g = Gestures::default();
        g.filter(down(pos2(100.0, 100.0)), 0.0, |_| true);
        let moved = g.filter(vec![Event::PointerMoved(pos2(102.0, 130.0))], 0.016, |_| true);
        assert!(presses(&moved).is_empty());
        assert_eq!(g.take_scroll(), 30.0);
        g.filter(vec![Event::PointerMoved(pos2(102.0, 110.0))], 0.032, |_| true);
        assert_eq!(g.take_scroll(), -20.0);
        let lifted = g.filter(up(pos2(102.0, 110.0)), 0.048, |_| true);
        assert!(presses(&lifted).is_empty());
    }

    #[test]
    fn a_sideways_drag_goes_to_the_control_from_where_it_started() {
        let mut g = Gestures::default();
        g.filter(down(pos2(100.0, 100.0)), 0.0, |_| true);
        let moved = g.filter(vec![Event::PointerMoved(pos2(130.0, 104.0))], 0.016, |_| true);
        assert!(matches!(moved.first(), Some(Event::PointerButton { pos, pressed: true, .. }) if *pos == pos2(100.0, 100.0)));
        assert!(matches!(moved.last(), Some(Event::PointerMoved(p)) if *p == pos2(130.0, 104.0)));
        assert_eq!(presses(&g.filter(up(pos2(130.0, 104.0)), 0.032, |_| true)), [false]);
        assert_eq!(g.take_scroll(), 0.0);
    }

    #[test]
    fn the_canvas_and_big_pads_get_the_finger_at_once() {
        let mut g = Gestures::default();
        assert_eq!(presses(&g.filter(down(pos2(100.0, 100.0)), 0.0, |_| false)), [true]);
        g.filter(vec![Event::PointerMoved(pos2(100.0, 160.0))], 0.016, |_| false);
        assert_eq!(g.take_scroll(), 0.0);
    }

    #[test]
    fn holding_still_hands_the_press_on() {
        let mut g = Gestures::default();
        g.filter(down(pos2(100.0, 100.0)), 0.0, |_| true);
        assert!(g.busy());
        assert_eq!(presses(&g.filter(Vec::new(), HOLD + 0.01, |_| true)), [true]);
        // Now a drag: vertical moves go to the control.
        g.filter(vec![Event::PointerMoved(pos2(100.0, 160.0))], 0.4, |_| true);
        assert_eq!(g.take_scroll(), 0.0);
    }

    #[test]
    fn a_fast_swipe_flings_on_after_the_finger_lifts() {
        let mut g = Gestures::default();
        g.filter(down(pos2(100.0, 400.0)), 0.0, |_| true);
        let mut y = 400.0;
        let mut t = 0.0;
        for _ in 0..5 {
            y -= 30.0;
            t += 0.016;
            g.filter(vec![Event::PointerMoved(pos2(100.0, y))], t, |_| true);
        }
        g.take_scroll();
        let lifted = g.filter(up(pos2(100.0, y)), t + 0.016, |_| true);
        assert!(!lifted.iter().any(|e| matches!(e, Event::PointerGone)), "hover stays while the fling runs");
        assert!(g.busy());
        let mut flung = 0.0;
        let mut gone = false;
        for i in 1..200 {
            let events = g.filter(Vec::new(), t + 0.016 * f64::from(i + 1), |_| true);
            flung += g.take_scroll();
            gone |= events.iter().any(|e| matches!(e, Event::PointerGone));
        }
        assert!(flung < -100.0, "the content keeps moving up: {flung}");
        assert!(gone && !g.busy());
    }

    #[test]
    fn small_controls_scroll_big_pads_and_strips_do_not() {
        let r = |w: f32, h: f32| egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(w, h));
        assert!(swipe_may_scroll(r(220.0, 20.0)), "a slider");
        assert!(swipe_may_scroll(r(24.0, 24.0)), "a check box");
        assert!(!swipe_may_scroll(r(300.0, 200.0)), "a colour field");
        assert!(!swipe_may_scroll(r(400.0, 4.0)), "a splitter");
        assert!(!swipe_may_scroll(r(8.0, 300.0)), "a scroll bar");
    }
}
