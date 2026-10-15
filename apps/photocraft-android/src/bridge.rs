//! Calls into the Android framework through JNI: what winit 0.30 and android-activity 0.6 don't
//! expose. This is the shell's only module with `unsafe` (one block, the documented way to borrow
//! the Activity reference android-activity holds); everything else goes through jni's safe API.

use std::os::fd::AsRawFd;
use std::path::PathBuf;

use jni::objects::{JByteArray, JObject, JObjectArray, JString};
use jni::refs::Global;
use jni::{JValue, jni_sig, jni_str};
use winit::platform::android::activity::AndroidApp;

use crate::documents::{self, Described, DocError, Grant, Picked, Picker, Provider};

/// Runs `f` on this thread's JNI environment with the app's Activity. A Java exception `f` lets
/// through comes back as [`jni::errors::Error::CaughtJavaException`], with its class and message.
fn on_activity<T>(app: &AndroidApp, f: impl FnOnce(&mut jni::Env, &JObject) -> jni::errors::Result<T>) -> jni::errors::Result<T> {
    let vm = jni::JavaVM::singleton()?;
    vm.attach_current_thread(|env| -> jni::errors::Result<T> {
        let raw = app.activity_as_ptr() as jni::sys::jobject;
        // SAFETY: android-activity documents this exact cast for `activity_as_ptr`: `raw` is a JNI
        // global reference to the Activity, valid for as long as `app` lives (it outlives this
        // call), and `as_cast_raw` only borrows it: nothing deletes the reference on drop.
        #[allow(unsafe_code)]
        let activity = unsafe { env.as_cast_raw::<Global<JObject>>(&raw)? };
        f(env, &activity)
    })
}

/// [`on_activity`] with errors as text.
fn with_activity<T>(app: &AndroidApp, f: impl FnOnce(&mut jni::Env, &JObject) -> jni::errors::Result<T>) -> Result<T, String> {
    on_activity(app, f).map_err(|e| e.to_string())
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

/// Where PhotoCraft keeps files that must survive a crash and never go to a backup
/// (`Context.getNoBackupFilesDir`): staged saves live there.
pub fn no_backup_dir(app: &AndroidApp) -> Result<PathBuf, String> {
    with_activity(app, |env, activity| {
        let dir = env.call_method(activity, jni_str!("noBackupDir"), jni_sig!("()Ljava/lang/String;"), &[])?.l()?;
        Ok(PathBuf::from(env.cast_local::<JString>(dir)?.try_to_string(env)?))
    })
    .map_err(|e| format!("app storage: {e}"))
}

// ------------------------------------------------------------------ documents (SAF)

/// The size of each chunk a document is read in (one Java array, reused).
const READ_CHUNK: usize = 1 << 20;

/// The documents provider: `PhotoCraftActivity`'s Storage Access Framework methods. Document
/// contents cross as bytes (chunked reads, a staged file Java copies from), never as paths.
pub struct Jni {
    app: AndroidApp,
}

impl Jni {
    pub fn new(app: AndroidApp) -> Self {
        Jni { app }
    }

    /// [`on_activity`] for document calls: a Java exception becomes the [`DocError`] it means.
    fn call<T>(&self, f: impl FnOnce(&mut jni::Env, &JObject) -> jni::errors::Result<T>) -> Result<T, DocError> {
        on_activity(&self.app, f).map_err(|e| doc_error(&e))
    }
}

/// The [`DocError`] for a JNI failure.
fn doc_error(e: &jni::errors::Error) -> DocError {
    match e {
        jni::errors::Error::CaughtJavaException { name, msg, .. } => documents::classify(name, msg),
        other => DocError::Failed(other.to_string()),
    }
}

/// The exception pending after a failed call, caught (so JNI can be used again) and returned in
/// place of the plain error.
fn caught(env: &jni::Env, e: jni::errors::Error) -> jni::errors::Error {
    match env.exception_catch() {
        Err(caught) => caught,
        Ok(()) => e,
    }
}

/// A Java `String[]` (null: `None`) as Rust strings; null elements read as "".
fn strings(env: &mut jni::Env, obj: JObject) -> jni::errors::Result<Option<Vec<String>>> {
    if obj.is_null() {
        return Ok(None);
    }
    let array = env.cast_local::<JObjectArray<JString>>(obj)?;
    let len = array.len(env)?;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let s = array.get_element(env, i)?;
        out.push(if s.is_null() { String::new() } else { s.try_to_string(env)? });
    }
    Ok(Some(out))
}

/// A Java string, or null for `None`.
fn string_or_null<'local>(env: &mut jni::Env<'local>, s: Option<&str>) -> jni::errors::Result<JObject<'local>> {
    Ok(match s {
        Some(s) => JString::new(env, s)?.into(),
        None => JObject::null(),
    })
}

/// Reads a document opened with `openRead`, a chunk at a time, for `photocraft_format::read::read_all`
/// (bounded reads, a clear error for documents larger than memory).
struct JavaReader<'e, 'l, 'a> {
    env: &'e mut jni::Env<'l>,
    activity: &'a JObject<'a>,
    handle: i64,
    buffer: &'a JByteArray<'a>,
    chunk: Vec<i8>,
    /// Bytes read from Java and not yet handed on.
    pending: Vec<u8>,
    taken: usize,
    /// What went wrong on the Java side, when it did.
    error: Option<DocError>,
}

impl std::io::Read for JavaReader<'_, '_, '_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.taken >= self.pending.len() {
            let n = self
                .env
                .call_method(self.activity, jni_str!("readChunk"), jni_sig!("(J[B)I"), &[JValue::Long(self.handle), JValue::from(self.buffer)])
                .and_then(|v| v.i());
            let n = match n {
                Ok(n) => n,
                Err(e) => {
                    let e = doc_error(&caught(self.env, e));
                    let message = e.reason();
                    self.error = Some(e);
                    return Err(std::io::Error::other(message));
                }
            };
            let Ok(n) = usize::try_from(n) else { return Ok(0) };
            let n = n.min(self.chunk.len());
            let Some(chunk) = self.chunk.get_mut(..n) else { return Ok(0) };
            self.buffer.get_region(self.env, 0, chunk).map_err(std::io::Error::other)?;
            self.pending.clear();
            self.pending.extend(chunk.iter().map(|&b| b as u8));
            self.taken = 0;
        }
        let rest = self.pending.get(self.taken..).unwrap_or_default();
        let n = rest.len().min(out.len());
        if let (Some(dst), Some(src)) = (out.get_mut(..n), rest.get(..n)) {
            dst.copy_from_slice(src);
        }
        self.taken += n;
        Ok(n)
    }
}

impl Provider for Jni {
    fn show(&self, request: u32, picker: &Picker) -> Result<(), DocError> {
        let request = i32::try_from(request).map_err(|_| DocError::Failed("too many picker requests".into()))?;
        self.call(|env, activity| {
            match picker {
                Picker::Open { mime_types, multiple, initial } => {
                    let types = JString::new(env, mime_types.join("\n"))?;
                    let initial = string_or_null(env, initial.as_deref())?;
                    env.call_method(
                        activity,
                        jni_str!("openDocuments"),
                        jni_sig!("(ILjava/lang/String;ZLjava/lang/String;)V"),
                        &[JValue::Int(request), JValue::from(&types), JValue::Bool(*multiple), JValue::from(&initial)],
                    )?;
                }
                Picker::Create { mime_type, name, initial } => {
                    let mime = JString::new(env, mime_type)?;
                    let name = JString::new(env, name)?;
                    let initial = string_or_null(env, initial.as_deref())?;
                    env.call_method(
                        activity,
                        jni_str!("createDocument"),
                        jni_sig!("(ILjava/lang/String;Ljava/lang/String;Ljava/lang/String;)V"),
                        &[JValue::Int(request), JValue::from(&mime), JValue::from(&name), JValue::from(&initial)],
                    )?;
                }
            }
            Ok(())
        })
    }

    fn take_result(&self) -> Result<Option<Picked>, DocError> {
        let fields = self.call(|env, activity| {
            let answer = env.call_method(activity, jni_str!("takePickerResult"), jni_sig!("()[Ljava/lang/String;"), &[])?.l()?;
            strings(env, answer)
        })?;
        Ok(fields.and_then(|f| {
            let picked = documents::parse_picked(&f);
            if picked.is_none() {
                log::warn!("a picker answer PhotoCraft can't read: {f:?}");
            }
            picked
        }))
    }

    fn describe(&self, uri: &str) -> Result<Described, DocError> {
        let fields = self.call(|env, activity| {
            let uri = JString::new(env, uri)?;
            let described =
                env.call_method(activity, jni_str!("describeDocument"), jni_sig!("(Ljava/lang/String;)[Ljava/lang/String;"), &[JValue::from(&uri)])?.l()?;
            strings(env, described)
        })?;
        Ok(documents::parse_described(&fields.unwrap_or_default()))
    }

    fn read(&self, uri: &str, size_hint: u64) -> Result<Vec<u8>, DocError> {
        self.call(|env, activity| {
            let juri = JString::new(env, uri)?;
            let handle = env.call_method(activity, jni_str!("openRead"), jni_sig!("(Ljava/lang/String;)J"), &[JValue::from(&juri)])?.j()?;
            let buffer = match JByteArray::new(env, READ_CHUNK) {
                Ok(b) => b,
                Err(e) => {
                    let e = caught(env, e);
                    env.call_method(activity, jni_str!("closeRead"), jni_sig!("(J)V"), &[JValue::Long(handle)])?;
                    return Err(e);
                }
            };
            let (bytes, error) = {
                let mut reader = JavaReader { env, activity, handle, buffer: &buffer, chunk: vec![0; READ_CHUNK], pending: Vec::new(), taken: 0, error: None };
                let bytes = photocraft_format::read::read_all(&mut reader, size_hint);
                (bytes, reader.error.take())
            };
            // Any exception was caught by the reader, so the stream can be closed.
            env.call_method(activity, jni_str!("closeRead"), jni_sig!("(J)V"), &[JValue::Long(handle)])?;
            Ok(bytes.map_err(|e| error.unwrap_or_else(|| DocError::Failed(e.to_string()))))
        })?
    }

    fn write(&self, uri: &str, staged: &std::fs::File, len: u64) -> Result<(), DocError> {
        let len = i64::try_from(len).map_err(|_| DocError::Failed("the document is too large to write".into()))?;
        // Java duplicates the descriptor (ParcelFileDescriptor.fromFd) and closes only its copy;
        // `staged` stays open, and owned here, for the whole call.
        let fd = staged.as_raw_fd();
        self.call(|env, activity| {
            let uri = JString::new(env, uri)?;
            env.call_method(
                activity,
                jni_str!("writeDocument"),
                jni_sig!("(Ljava/lang/String;IJ)V"),
                &[JValue::from(&uri), JValue::Int(fd), JValue::Long(len)],
            )?;
            Ok(())
        })
    }

    fn grants(&self) -> Result<Vec<Grant>, DocError> {
        let flat = self.call(|env, activity| {
            let held = env.call_method(activity, jni_str!("persistedGrants"), jni_sig!("()[Ljava/lang/String;"), &[])?.l()?;
            strings(env, held)
        })?;
        Ok(documents::parse_grants(&flat.unwrap_or_default()))
    }

    fn release(&self, uri: &str) -> Result<(), DocError> {
        self.call(|env, activity| {
            let uri = JString::new(env, uri)?;
            env.call_method(activity, jni_str!("releaseGrant"), jni_sig!("(Ljava/lang/String;)V"), &[JValue::from(&uri)])?;
            Ok(())
        })
    }
}
