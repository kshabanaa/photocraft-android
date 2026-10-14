//! Calls into the Android framework through JNI: what winit 0.30 and android-activity 0.6 don't
//! expose. This is the shell's only module with `unsafe` (one block, the documented way to borrow
//! the Activity reference android-activity holds); everything else goes through jni's safe API.

use jni::objects::JObject;
use jni::refs::Global;
use jni::{JValue, jni_sig, jni_str};
use winit::platform::android::activity::AndroidApp;

/// Runs `f` on this thread's JNI environment with the app's Activity.
fn with_activity<T>(app: &AndroidApp, f: impl FnOnce(&mut jni::Env, &JObject) -> jni::errors::Result<T>) -> Result<T, String> {
    let vm = jni::JavaVM::singleton().map_err(|e| format!("no Java VM: {e}"))?;
    vm.attach_current_thread(|env| -> jni::errors::Result<T> {
        let raw = app.activity_as_ptr() as jni::sys::jobject;
        // SAFETY: android-activity documents this exact cast for `activity_as_ptr`: `raw` is a JNI
        // global reference to the Activity, valid for as long as `app` lives (it outlives this
        // call), and `as_cast_raw` only borrows it: nothing deletes the reference on drop.
        #[allow(unsafe_code)]
        let activity = unsafe { env.as_cast_raw::<Global<JObject>>(&raw)? };
        f(env, &activity)
    })
    .map_err(|e| e.to_string())
}

/// The modifier keys (Android `KeyEvent` meta state) held at the last key press and held now, as
/// `PhotoCraftActivity` records them: winit 0.30 never reports modifiers on Android.
pub fn key_meta(app: &AndroidApp) -> Result<(i32, i32), String> {
    with_activity(app, |env, activity| {
        let at_key = env.get_field(activity, jni_str!("keyMeta"), jni_sig!("I"))?.i()?;
        let held = env.get_field(activity, jni_str!("heldMeta"), jni_sig!("I"))?.i()?;
        Ok((at_key, held))
    })
    .map_err(|e| format!("key modifiers: {e}"))
}

/// The parts of the window covered by the status bar, navigation bar, display cutout and soft
/// keyboard, in pixels: left, top, right, bottom. Zero before the window is attached.
pub fn window_insets(app: &AndroidApp, sdk: u32) -> Result<[i32; 4], String> {
    with_activity(app, |env, activity| {
        let window = env.call_method(activity, jni_str!("getWindow"), jni_sig!("()Landroid/view/Window;"), &[])?.l()?;
        let decor = env.call_method(&window, jni_str!("getDecorView"), jni_sig!("()Landroid/view/View;"), &[])?.l()?;
        let insets = env.call_method(&decor, jni_str!("getRootWindowInsets"), jni_sig!("()Landroid/view/WindowInsets;"), &[])?.l()?;
        if insets.is_null() {
            return Ok([0; 4]);
        }
        if sdk >= 30 {
            // WindowInsets.Type: system bars, display cutout and IME together.
            let mut mask = 0;
            for name in [jni_str!("systemBars"), jni_str!("displayCutout"), jni_str!("ime")] {
                mask |= env.call_static_method(jni_str!("android/view/WindowInsets$Type"), name, jni_sig!("()I"), &[])?.i()?;
            }
            let r = env.call_method(&insets, jni_str!("getInsets"), jni_sig!("(I)Landroid/graphics/Insets;"), &[JValue::Int(mask)])?.l()?;
            let mut out = [0; 4];
            for (slot, name) in out.iter_mut().zip([jni_str!("left"), jni_str!("top"), jni_str!("right"), jni_str!("bottom")]) {
                *slot = env.get_field(&r, name, jni_sig!("I"))?.i()?;
            }
            Ok(out)
        } else {
            // API 29: the system window insets (bars and IME), deprecated later but complete there.
            let mut out = [0; 4];
            for (slot, name) in out.iter_mut().zip([
                jni_str!("getSystemWindowInsetLeft"),
                jni_str!("getSystemWindowInsetTop"),
                jni_str!("getSystemWindowInsetRight"),
                jni_str!("getSystemWindowInsetBottom"),
            ]) {
                *slot = env.call_method(&insets, name, jni_sig!("()I"), &[])?.i()?;
            }
            Ok(out)
        }
    })
    .map_err(|e| format!("window insets: {e}"))
}
