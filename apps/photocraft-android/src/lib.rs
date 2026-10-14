//! PhotoCraft on Android (Phase 0 spike): starts the shared `PhotocraftApp` through eframe's
//! GameActivity backend with default (empty) platform services. Off Android this crate is empty,
//! so workspace builds and tests on desktop hosts are unaffected.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(target_os = "android")]
mod bridge;
pub mod gesture;

#[cfg(target_os = "android")]
mod android {
    use crate::bridge;
    use eframe::egui;
    use winit::platform::android::activity::input::{ImeOptions, InputType, TextInputAction, TextInputState, TextSpan};
    use photocraft_ui_egui::PhotocraftApp;
    use photocraft_ui_egui::theme::ThemeKind;
    use winit::platform::android::activity::AndroidApp;

    /// Called once by android-activity on its native thread after `GameActivity.onCreate`.
    // The export is the only reason for `unsafe` here: android-activity looks the symbol up by
    // name. The body is safe code.
    #[allow(unsafe_code)]
    #[unsafe(no_mangle)]
    fn android_main(app: AndroidApp) {
        android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("PhotoCraft"));
        std::panic::set_hook(Box::new(|info| log::error!("panic: {info}")));
        let shell_app = app.clone();
        let mut options = eframe::NativeOptions { android_app: Some(app), renderer: eframe::Renderer::Wgpu, ..Default::default() };
        if let eframe::egui_wgpu::WgpuSetup::CreateNew(create) = &mut options.wgpu_options.wgpu_setup {
            // One backend per launch, chosen before the instance exists: a window bound to a Vulkan
            // surface cannot also get an EGL surface (BadAlloc). Vulkan by default; OpenGL ES on the
            // deny-list. The emulator's gfxstream Vulkan driver (ro.hardware.vulkan=ranchu) crashes in
            // vkQueueSubmit on egui's first frame (Phase 0 spike, Android 16 emulator).
            let vulkan_denied = android_system_properties::AndroidSystemProperties::new().get("ro.hardware.vulkan").as_deref() == Some("ranchu");
            create.instance_descriptor.backends = if vulkan_denied { eframe::wgpu::Backends::GL } else { eframe::wgpu::Backends::VULKAN };
            log::info!("gpu plan: {}", if vulkan_denied { "OpenGL ES (Vulkan driver on the deny-list)" } else { "Vulkan" });
            create.native_adapter_selector = Some(std::sync::Arc::new(|adapters, surface| {
                use eframe::wgpu::Backend;
                let usable = |a: &&eframe::wgpu::Adapter| surface.is_none_or(|s| a.is_surface_supported(s));
                let denied = |a: &eframe::wgpu::Adapter| a.get_info().driver == "gfxstream";
                adapters
                    .iter()
                    .filter(usable)
                    .find(|a| a.get_info().backend == Backend::Vulkan && !denied(a))
                    .or_else(|| adapters.iter().filter(usable).find(|a| a.get_info().backend == Backend::Gl))
                    .cloned()
                    .ok_or_else(|| "no usable GPU adapter (Vulkan or OpenGL ES)".to_owned())
            }));
            create.device_descriptor = std::sync::Arc::new(|adapter| eframe::wgpu::DeviceDescriptor {
                label: Some("photocraft wgpu device"),
                // Never ask for more than the adapter offers: mobile and emulator Vulkan drivers allow less
                // than wgpu's defaults (SwiftShader: 16 KiB uniform buffers, not 64 KiB).
                required_limits: photocraft_ui_egui::gpu_canvas::device_limits(adapter).or_worse_values_from(&adapter.limits()),
                ..Default::default()
            });
        }
        let started = eframe::run_native(
            "PhotoCraft",
            options,
            Box::new(move |cc| {
                PhotocraftApp::setup_context(&cc.egui_ctx, ThemeKind::Pro);
                let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), photocraft_ui_egui::Services::default());
                app.set_theme(&cc.egui_ctx, ThemeKind::Pro);
                if let Some(rs) = cc.wgpu_render_state.clone() {
                    let info = rs.adapter.get_info();
                    log::info!("wgpu backend {:?}, adapter {:?}", info.backend, info.name);
                    if info.backend == eframe::wgpu::Backend::Gl {
                        // The GPU canvas's shaders don't compile to OpenGL ES yet (naga's GLSL for
                        // the canvas vertex shader is rejected: Phase 0, SwiftShader GLES 3.0), and
                        // the canvas then shows nothing. The CPU canvas (as the web build's `?cpu`)
                        // draws through egui's own textures.
                        log::info!("canvas: CPU (OpenGL ES)");
                    } else {
                        app.set_wgpu(rs);
                    }
                }
                let sdk = android_system_properties::AndroidSystemProperties::new()
                    .get("ro.build.version.sdk")
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(29);
                Ok(Box::new(AndroidShell { app, android: shell_app, sdk, logged: None, scale_logged: None, deferred: Vec::new(), ime: Ime::default(), gestures: Default::default(), held: egui::Modifiers::NONE }))
            }),
        );
        if let Err(e) = started {
            log::error!("PhotoCraft could not start: {e}");
        }
    }

    /// `PhotocraftApp` plus what winit 0.30 doesn't give egui on Android:
    /// - the parts of the window the status bar, navigation bar, display cutout and soft keyboard
    ///   cover, read through JNI. egui keeps panels and windows inside `Context::content_rect`,
    ///   which is the window minus these insets;
    /// - soft-keyboard text. GameActivity delivers it as GameTextInput state, which winit drops,
    ///   so the shell reads that state every frame and replays the changes as egui events.
    struct AndroidShell {
        app: PhotocraftApp,
        android: AndroidApp,
        /// The device's API level (`ro.build.version.sdk`).
        sdk: u32,
        /// The last insets logged (left, top, right, bottom, window width, height), in pixels.
        logged: Option<[i32; 6]>,
        /// The last scale facts logged (debug).
        scale_logged: Option<(f32, f32, f32, Option<egui::Vec2>)>,
        /// Input held over from the last frame: the release of a tap (see `raw_input_hook`).
        deferred: Vec<egui::Event>,
        ime: Ime,
        /// A finger's swipe told from its drag (see `gesture`).
        gestures: crate::gesture::Gestures,
        /// The modifier keys held, as last told to egui (see `key_modifiers`).
        held: egui::Modifiers,
    }

    /// The soft keyboard's text buffer as last seen, while a text field has focus, and whether
    /// the keyboard may show for that field.
    #[derive(Default)]
    struct Ime {
        active: bool,
        shadow: String,
        /// The widget that had focus at the end of the last frame.
        focus: Option<egui::Id>,
        /// Whether the soft keyboard may show for `focus`: only after a tap on the field.
        keyboard: bool,
        /// Where this frame's input pressed, if it did (points).
        press: Option<egui::Pos2>,
        /// The last press: where (points) and when (`InputState::time`, seconds).
        last_press: Option<(egui::Pos2, f64)>,
        /// The focused field's last IME output (see `gate_keyboard`).
        output: Option<egui::output::IMEOutput>,
    }

    impl AndroidShell {
        fn safe_area(&mut self, ctx: &egui::Context) -> Option<egui::SafeAreaInsets> {
            let window = self.android.native_window()?;
            let (w, h) = (window.width(), window.height());
            if w <= 0 || h <= 0 {
                return None;
            }
            let px = match bridge::window_insets(&self.android, self.sdk) {
                Ok(px) => px.map(|v| v.max(0)),
                Err(e) => {
                    log::warn!("{e}");
                    return None;
                }
            };
            let key = [px[0], px[1], px[2], px[3], w, h];
            if self.logged != Some(key) {
                log::info!("safe area: insets {:?} (left, top, right, bottom px) of a {w} x {h} window", &px);
                self.logged = Some(key);
                ctx.request_repaint();
            }
            let ppp = ctx.pixels_per_point();
            let points = |v: i32| v as f32 / ppp;
            Some(egui::SafeAreaInsets(egui::epaint::MarginF32 { left: points(px[0]), right: points(px[2]), top: points(px[1]), bottom: points(px[3]) }))
        }

        /// Shows the soft keyboard only for a text field the user tapped. Dialogs focus their first
        /// field when they open (New Document's name), and in landscape the keyboard then covered
        /// most of the dialog before the user asked to type. A field that took focus on its own
        /// keeps it (hardware keys still type there); a tap on it brings the keyboard up.
        fn gate_keyboard(&mut self, ctx: &egui::Context) {
            let now = ctx.input(|i| i.time);
            let press = self.ime.press.take();
            if let Some(pos) = press {
                self.ime.last_press = Some((pos, now));
            }
            let focused = ctx.memory(|m| m.focused());
            let editing = ctx.text_edit_focused();
            let rect = focused.and_then(|id| ctx.read_response(id)).map(|r| r.rect);
            let tapped = |pos: egui::Pos2| rect.is_some_and(|r| r.expand(4.0).contains(pos));
            if focused != self.ime.focus {
                // Focus moved: to a field the user just tapped, or by itself.
                self.ime.focus = focused;
                self.ime.output = None;
                self.ime.keyboard = editing && self.ime.last_press.is_some_and(|(pos, t)| now - t < 1.0 && tapped(pos));
                if editing {
                    self.start_editor(ctx);
                }
            } else if editing && press.is_some_and(tapped) && !self.ime.keyboard {
                // A tap on the field that already has focus (or whose keyboard was dismissed).
                self.ime.keyboard = true;
            }
            if editing && press.is_some_and(tapped) {
                // egui only asks winit for the keyboard when IME turns on, so a keyboard the user
                // dismissed with focus kept would not come back.
                self.android.show_soft_input(true);
            }
            if editing && !self.ime.keyboard {
                ctx.output_mut(|o| o.ime = None);
            } else if editing {
                // egui reports IME output only for a field it painted. When the keyboard shrinks
                // the screen and scrolls the field out of sight, that would hide the keyboard, the
                // field would come back, and the keyboard with it, over and over. The field still
                // has focus, so the keyboard stays.
                ctx.output_mut(|o| match &o.ime {
                    Some(ime) => self.ime.output = Some(ime.clone()),
                    None => o.ime.clone_from(&self.ime.output),
                });
            }
        }

        /// A text field took focus (this frame, before egui asks for the keyboard). Start the
        /// keyboard from an empty buffer, with no full-screen editor in landscape: a number pad for
        /// number fields (egui reports them as `DragValue`s), else text, multi-line so Enter
        /// arrives as "\n" (winit drops GameTextInput's editor actions).
        fn start_editor(&mut self, ctx: &egui::Context) {
            // The tap that focused a number field reports it as a click (or focus-gained) on a DragValue.
            let number = ctx.output(|o| o.events.iter().any(|e| e.widget_info().typ == egui::WidgetType::DragValue));
            let input = if number {
                InputType::TYPE_CLASS_NUMBER | InputType::TYPE_NUMBER_FLAG_DECIMAL | InputType::TYPE_NUMBER_FLAG_SIGNED
            } else {
                InputType::TYPE_CLASS_TEXT | InputType::TYPE_TEXT_FLAG_MULTI_LINE
            };
            self.android.set_ime_editor_info(input, TextInputAction::None, ImeOptions::IME_FLAG_NO_FULLSCREEN);
            self.android.set_text_input_state(TextInputState { text: String::new(), selection: TextSpan { start: 0, end: 0 }, compose_region: None });
            self.ime.shadow.clear();
        }

        /// Gives this frame's keys and clicks the modifiers held with them (winit 0.30 never
        /// reports them on Android): a key press the modifiers held when it went down, a click or
        /// scroll the ones held now.
        fn key_modifiers(&mut self, raw_input: &mut egui::RawInput) {
            use egui::Event;
            let keyed = raw_input.events.iter().any(|e| matches!(e, Event::Key { .. } | Event::PointerButton { .. } | Event::MouseWheel { .. }));
            if !keyed && self.held.is_none() {
                return;
            }
            let (at_key, held) = match bridge::key_meta(&self.android) {
                Ok((at_key, held)) => (crate::modifiers_from_meta(at_key), crate::modifiers_from_meta(held)),
                Err(e) => {
                    log::warn!("{e}");
                    return;
                }
            };
            if held != self.held {
                self.held = held;
                raw_input.events.insert(0, Event::ModifiersChanged(held));
            }
            for event in &mut raw_input.events {
                match event {
                    Event::Key { modifiers, .. } => *modifiers = at_key,
                    Event::PointerButton { modifiers, .. } | Event::MouseWheel { modifiers, .. } => *modifiers = held,
                    _ => {}
                }
            }
        }

        /// What the soft keyboard typed since the last frame, as egui events.
        fn ime_events(&mut self, ctx: &egui::Context) -> Vec<egui::Event> {
            let focused = ctx.text_edit_focused();
            self.ime.active = focused;
            if !focused {
                return Vec::new();
            }
            let text = self.android.text_input_state().text;
            if text == self.ime.shadow {
                return Vec::new();
            }
            let events = crate::keyboard_diff(&self.ime.shadow, &text);
            self.ime.shadow = text;
            events
        }
    }

    impl eframe::App for AndroidShell {
        fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
            self.app.logic(ctx, frame);
        }

        fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
            // egui hands the app a root Ui over the whole viewport; PhotoCraft's panels lay out in
            // the part the system bars, cutout and keyboard leave free. The strips behind the bars
            // take the window colour.
            let content = ui.ctx().content_rect();
            ui.painter().rect_filled(ui.ctx().viewport_rect(), 0.0, ui.visuals().window_fill);
            // What a finger swiped this frame scrolls the scroll area under it, as a wheel would,
            // but without wheel events: dropdowns and number fields step on those.
            let scroll = self.gestures.take_scroll();
            if scroll != 0.0 {
                ui.ctx().input_mut(|i| i.smooth_scroll_delta.y += scroll);
            }
            ui.scope_builder(egui::UiBuilder::new().max_rect(content), |ui| self.app.ui(ui, frame));
            self.gate_keyboard(ui.ctx());
        }

        fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
            let scale = (ctx.pixels_per_point(), ctx.zoom_factor(), ctx.native_pixels_per_point().unwrap_or(0.0), raw_input.screen_rect.map(|r| r.size()));
            if self.scale_logged != Some(scale) {
                log::info!("scale: ppp {:.3}, zoom {:.3}, native {:.3}, screen_rect {:?}", scale.0, scale.1, scale.2, scale.3);
                self.scale_logged = Some(scale);
            }
            self.app.raw_input_hook(ctx, raw_input);
            self.key_modifiers(raw_input);
            if let Some(insets) = self.safe_area(ctx) {
                raw_input.safe_area_insets = Some(insets);
            }
            let now = raw_input.time.unwrap_or(0.0);
            raw_input.events = self.gestures.filter(std::mem::take(&mut raw_input.events), now, |pos| {
                // The topmost interactive widget under the finger, from last frame's hit test.
                ctx.interactive_rects_last_pass().into_iter().rev().find(|r| r.contains(pos)).is_some_and(crate::gesture::swipe_may_scroll)
            });
            if self.gestures.busy() {
                ctx.request_repaint();
            }
            // A quick tap arrives as press and release in the same frame, and widgets that act on
            // the release of a press they saw (menu items) never fired; the last moves of a drag
            // that arrive with its release are lost the same way (a slider stopped short of the
            // finger). Hold the release (and what follows it) over to the next frame, so a tap
            // behaves like a mouse click and a drag ends where the finger did.
            let mut events = std::mem::take(&mut self.deferred);
            events.append(&mut raw_input.events);
            let pointer = |e: &egui::Event| matches!(e, egui::Event::PointerButton { pressed: true, .. } | egui::Event::PointerMoved(_));
            let release = |e: &egui::Event| matches!(e, egui::Event::PointerButton { pressed: false, .. });
            if let Some(first) = events.iter().position(pointer)
                && let Some(offset) = events.iter().skip(first + 1).position(release)
            {
                self.deferred = events.split_off(first + 1 + offset);
                ctx.request_repaint();
            }
            raw_input.events = events;
            self.ime.press = raw_input.events.iter().rev().find_map(|e| match e {
                egui::Event::PointerButton { pos, pressed: true, .. } => Some(*pos),
                _ => None,
            });
            // Android reports a pressure for every touch, and for a finger it is the contact area
            // (often 0.1-0.3), so strokes came out hairline-thin. winit 0.30 doesn't say which
            // tool touched, so fingers paint at full pressure, like a mouse, until the stylus
            // bridge (FR-INP-06) feeds real pen samples.
            for event in &mut raw_input.events {
                if let egui::Event::Touch { force, .. } = event {
                    *force = None;
                }
            }
            raw_input.events.extend(self.ime_events(ctx));
            if self.ime.active {
                // Keyboard text arrives without a winit event to wake the loop.
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
        }
    }
}

/// egui's modifiers for an Android `KeyEvent` meta state (`getMetaState()`).
pub fn modifiers_from_meta(meta: i32) -> egui::Modifiers {
    const SHIFT: i32 = 0x1;
    const ALT: i32 = 0x2;
    const CTRL: i32 = 0x1000;
    const META: i32 = 0x1_0000;
    let ctrl = meta & CTRL != 0;
    egui::Modifiers { alt: meta & ALT != 0, ctrl, shift: meta & SHIFT != 0, mac_cmd: false, command: ctrl || meta & META != 0 }
}

/// The egui events that turn the keyboard buffer `old` into `new` at the caret: a Backspace for
/// each character removed after their common prefix, then the inserted text, with each "\n" as
/// an Enter press (an autocorrected "teh" -> "the" is two Backspaces and "he").
pub fn keyboard_diff(old: &str, new: &str) -> Vec<egui::Event> {
    let key = |key, pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::NONE };
    let common = old.chars().zip(new.chars()).take_while(|(a, b)| a == b).count();
    let mut events = Vec::new();
    for _ in common..old.chars().count() {
        events.push(key(egui::Key::Backspace, true));
        events.push(key(egui::Key::Backspace, false));
    }
    let inserted: String = new.chars().skip(common).collect();
    for (i, part) in inserted.split('\n').enumerate() {
        if i > 0 {
            events.push(key(egui::Key::Enter, true));
            events.push(key(egui::Key::Enter, false));
        }
        if !part.is_empty() {
            events.push(egui::Event::Text(part.to_owned()));
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use super::keyboard_diff;
    use egui::Event;

    fn describe(events: &[Event]) -> Vec<String> {
        events
            .iter()
            .filter_map(|e| match e {
                Event::Text(t) => Some(format!("text:{t}")),
                Event::Key { key, pressed: true, .. } => Some(format!("{key:?}")),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn android_meta_states_map_to_egui_modifiers() {
        use super::modifiers_from_meta;
        assert!(modifiers_from_meta(0).is_none());
        // Ctrl+Shift with the left keys (META_CTRL_ON | META_CTRL_LEFT_ON | META_SHIFT_ON | META_SHIFT_LEFT_ON).
        let m = modifiers_from_meta(0x1000 | 0x2000 | 0x1 | 0x40);
        assert!(m.ctrl && m.command && m.shift && !m.alt);
        assert!(modifiers_from_meta(0x2 | 0x10).alt);
    }

    #[test]
    fn typing_deleting_autocorrect_and_enter() {
        assert_eq!(describe(&keyboard_diff("", "Spike")), ["text:Spike"]);
        assert_eq!(describe(&keyboard_diff("Spike", "Spik")), ["Backspace"]);
        assert_eq!(describe(&keyboard_diff("teh", "the")), ["Backspace", "Backspace", "text:he"]);
        assert_eq!(describe(&keyboard_diff("ok", "ok\n")), ["Enter"]);
        assert_eq!(describe(&keyboard_diff("", "a\nb")), ["text:a", "Enter", "text:b"]);
        assert!(keyboard_diff("same", "same").is_empty());
        // Multi-byte text compares by character, not byte.
        assert_eq!(describe(&keyboard_diff("مرح", "مرحبا")), ["text:با"]);
        assert_eq!(describe(&keyboard_diff("日本", "日")), ["Backspace"]);
    }
}
