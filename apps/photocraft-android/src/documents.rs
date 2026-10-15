//! Documents through Android's Storage Access Framework (work packages 1.5 and 1.6; FR-FIO-05 to
//! FR-FIO-08 and FR-FIO-11): File › Open, Open Recent, Save, Save As and Export, behind the
//! `photocraft_ui_egui::Services` seam the desktop and web shells implement.
//!
//! - **Locations.** Where the desktop has a path, a document here has a *location*,
//!   `saf://<key>/<display name>` (UC-03, [`photocraft_ui_egui::file_open::is_location`]). The UI
//!   takes the format from its extension and the title from its last part, and keeps it for
//!   File › Save and Open Recent; the key stands for the document's `content://` URI
//!   ([`key_for`]). URIs never leave this module and the bridge.
//! - **Pickers.** The system pickers answer on Android's main thread; [`Documents::frame`]
//!   collects their answers every frame, and worker threads describe and read the chosen
//!   documents, so a slow provider (a cloud drive) never stalls the window. Answers go back
//!   through the [`FileDialogReply`], which wakes the app.
//! - **Safe write** (FR-FIO-07, NFR-DATA-03). A save encodes the whole document first; an encoder
//!   error leaves the target untouched. The bytes are then *staged*: written to a file in app
//!   storage and synced, with a record of what they are for. Only then is the target replaced,
//!   from the staged file, by the bridge. A failed write keeps the staged copy (and the document
//!   stays open and unsaved); a process killed mid-write leaves it behind too, and the next start
//!   offers it through the recovery flow (`Services::recover`), bound to the same location so
//!   File › Save finishes the job. Staged copies go once a save of that document succeeds, or
//!   when it is closed without saving.
//! - **Recents** (FR-FIO-11). Chosen documents keep a lasting grant where the provider offers one,
//!   so Open Recent works after a restart. Grants are released once a document has left both the
//!   recent list and the open tabs, and the list keeps at most [`MAX_RECENT_GRANTS`] of them.
//!
//! Everything here is plain Rust over the [`Provider`] trait, so the host tests drive it with a
//! fake provider and fault injection; `bridge::Jni` is the Android implementation.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use photocraft_doc::Document;
use photocraft_ui_egui::file_open::display_name;
use photocraft_ui_egui::jobs_ui::SaveCtl;
use photocraft_ui_egui::{ExportSettings, FileDialogAnswer, FileDialogReply, FileDialogRequest, PhotocraftApp, Recoverable, Services};
use serde::{Deserialize, Serialize};

/// The scheme of document locations.
pub const SCHEME: &str = "saf://";
/// How many documents Open Recent keeps (each holds a lasting grant; FR-FIO-11).
pub const MAX_RECENT_GRANTS: usize = 30;
/// Staged copies older than this are dropped at start-up: they were offered for recovery on
/// every launch since.
const STAGED_MAX_AGE_S: u64 = 30 * 24 * 3600;
/// The share of a save's progress bar that encoding fills (as on the desktop).
const ENCODE_SHARE: f32 = 0.85;
/// The recovery key prefix of staged copies (see [`Documents::adopt`]).
const STAGED_KEY: &str = "staged:";
/// Folder pickers (FR-FIO-10, work package 1.7) aren't built yet.
const NO_FOLDERS: &str = "Choosing a folder isn't available on Android yet.";

// ------------------------------------------------------------------ locations

/// The location of the document with `key`, called `name`.
pub fn location(key: &str, name: &str) -> String {
    format!("{SCHEME}{key}/{}", safe_name(name))
}

/// The key of a location (or of its folder part, `saf://<key>`), when it is one of ours.
pub fn key_of(location: &str) -> Option<&str> {
    let key = location.strip_prefix(SCHEME)?.split('/').next()?;
    (key.len() == 16 && key.bytes().all(|b| b.is_ascii_hexdigit())).then_some(key)
}

/// A stable key for a document URI (64-bit FNV-1a, in hex): the same document always gets the
/// same location, so Open Recent lists it once.
pub fn key_for(uri: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in uri.bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// A display name from a provider, made safe as the last part of a location: no separators or
/// control characters, not empty or a dot name, and at most 255 bytes (the extension kept).
/// Names come from other apps and are treated as hostile (NFR-SEC-03).
pub fn safe_name(name: &str) -> String {
    let cleaned: String = name.chars().map(|c| if c == '/' || c == '\\' || c.is_control() { '_' } else { c }).collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() || cleaned.chars().all(|c| c == '.') {
        return "Untitled".into();
    }
    const MAX: usize = 255;
    if cleaned.len() <= MAX {
        return cleaned.to_string();
    }
    let ext = extension(cleaned).filter(|e| e.len() < 16).map(|e| format!(".{e}")).unwrap_or_default();
    let mut stem = String::new();
    for c in cleaned.chars() {
        if stem.len() + c.len_utf8() + ext.len() > MAX {
            break;
        }
        stem.push(c);
    }
    stem + &ext
}

/// The lower-case extension of a name, without the dot.
fn extension(name: &str) -> Option<String> {
    Path::new(name).extension().and_then(|e| e.to_str()).filter(|e| !e.is_empty()).map(str::to_ascii_lowercase)
}

/// The name and extension a Save dialog suggests for `suggested` (a location, a path or a bare
/// name): its last part, with `.psd` added when it has no extension, as the desktop's type list does.
pub fn save_name(suggested: &str) -> (String, String) {
    let name = safe_name(&display_name(suggested));
    match extension(&name) {
        Some(ext) => (name, ext),
        None => (format!("{name}.psd"), "psd".into()),
    }
}

/// The location of a document just created by the Save picker: the provider's name for it (it may
/// have made the name unique, "photo (1).psd"), with the chosen format's extension added when the
/// name has none, so the format stays the one the user chose.
pub fn created_location(key: &str, provider_name: &str, requested: &str, ext: &str) -> String {
    let name = if provider_name.trim().is_empty() { requested } else { provider_name };
    let name = safe_name(name);
    if extension(&name).is_some() { location(key, &name) } else { location(key, &format!("{name}.{ext}")) }
}

// ------------------------------------------------------------------ MIME types

/// Every extension the desktop's File › Open accepts (`OPEN_EXTS` in apps/photocraft/src/
/// services.rs), with the MIME type document providers report for it. Formats Android has no type
/// for get the type other systems use; providers often report them as `application/octet-stream`.
const TYPES: &[(&str, &str)] = &[
    ("pcraft", "application/octet-stream"),
    ("pdn", "application/octet-stream"),
    ("ora", "image/openraster"),
    ("kra", "application/x-krita"),
    ("krz", "application/x-krita"),
    ("psd", "image/vnd.adobe.photoshop"),
    ("psb", "image/vnd.adobe.photoshop"),
    ("psdt", "image/vnd.adobe.photoshop"),
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("tif", "image/tiff"),
    ("tiff", "image/tiff"),
    ("webp", "image/webp"),
    ("gif", "image/gif"),
    ("bmp", "image/bmp"),
    ("tga", "image/x-tga"),
    ("ico", "image/x-icon"),
    ("qoi", "image/qoi"),
    ("exr", "image/x-exr"),
    ("hdr", "image/vnd.radiance"),
    ("pbm", "image/x-portable-bitmap"),
    ("pgm", "image/x-portable-graymap"),
    ("ppm", "image/x-portable-pixmap"),
    ("pam", "image/x-portable-arbitrarymap"),
    ("pfm", "image/x-portable-floatmap"),
    ("pcx", "image/x-pcx"),
    ("dds", "image/vnd-ms.dds"),
    ("xpm", "image/x-xpixmap"),
    ("fits", "image/fits"),
    ("fit", "image/fits"),
    ("fts", "image/fits"),
    ("dpx", "image/x-dpx"),
    ("heic", "image/heic"),
    ("heif", "image/heif"),
    ("hif", "image/heif"),
    ("dng", "image/x-adobe-dng"),
    ("cr2", "image/x-canon-cr2"),
    ("cr3", "image/x-canon-cr3"),
    ("nef", "image/x-nikon-nef"),
    ("nrw", "image/x-nikon-nrw"),
    ("arw", "image/x-sony-arw"),
    ("pef", "image/x-pentax-pef"),
    ("orf", "image/x-olympus-orf"),
    ("rw2", "image/x-panasonic-rw2"),
    ("raf", "image/x-fuji-raf"),
    ("abr", "application/octet-stream"),
    ("grd", "application/octet-stream"),
    ("svg", "image/svg+xml"),
    ("svgz", "image/svg+xml"),
    ("aco", "application/octet-stream"),
    ("ase", "application/octet-stream"),
    ("kys", "application/octet-stream"),
    ("af", "application/octet-stream"),
    ("afdesign", "application/octet-stream"),
    ("afphoto", "application/octet-stream"),
    ("afpub", "application/octet-stream"),
];

/// Types Android itself knows, which providers report reliably, and for which the Save picker
/// keeps the extension that matches them.
const COMMON: &[&str] = &["image/png", "image/jpeg", "image/gif", "image/bmp", "image/webp", "image/tiff", "image/heic", "image/heif"];

const ANY: &str = "*/*";
const OCTET: &str = "application/octet-stream";

/// The MIME type for an extension (any case), when PhotoCraft knows one.
pub fn mime_for(ext: &str) -> Option<&'static str> {
    TYPES.iter().find(|(e, _)| e.eq_ignore_ascii_case(ext)).map(|(_, m)| *m)
}

/// The MIME types an Open picker offers (FR-FIO-01). File › Open (`None`): every format's type,
/// plus every file, since providers report professional formats inconsistently and files are
/// identified by their contents anyway. A command that reads certain extensions: their types,
/// plus every file when one of them has no type providers report reliably.
pub fn open_mime_types(extensions: Option<&[String]>) -> Vec<String> {
    let mut types: Vec<String> = Vec::new();
    let mut push = |m: &str| {
        if !types.iter().any(|t| t == m) {
            types.push(m.to_string());
        }
    };
    match extensions.filter(|e| !e.is_empty()) {
        None => {
            for (_, m) in TYPES {
                push(m);
            }
            push(OCTET);
            push("application/zip");
            push(ANY);
        }
        Some(exts) => {
            let mut any = false;
            for ext in exts {
                match mime_for(ext.trim_start_matches('.')) {
                    Some(m) => {
                        push(m);
                        any |= !COMMON.contains(&m);
                    }
                    None => any = true,
                }
            }
            if any {
                push(OCTET);
                push(ANY);
            }
        }
    }
    types
}

/// The MIME type a Save picker creates every document with: `application/octet-stream`, whatever
/// the format (FR-FIO-06). Providers append the extension of a typed create to the name the user
/// typed when it doesn't match: on a Galaxy S20+ (Android 13), a JPEG's Save As typed as
/// `name.pcraft` was created as `name.pcraft.jpg` and saved as a JPEG. A typeless create keeps the
/// name as typed, so typing another extension switches the format. Exports still show in the
/// gallery: MediaStore types files on device storage by their extension (FR-FIO-08).
pub const CREATE_MIME: &str = OCTET;

// ------------------------------------------------------------------ errors

/// What went wrong with a document, in terms the user can act on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocError {
    /// The user backed out of a picker.
    Cancelled,
    /// PhotoCraft's permission for the document is gone (revoked, or a grant that didn't last).
    PermissionLost,
    /// The document is gone: deleted, moved, or its storage removed.
    Missing,
    /// The provider only allows reading it.
    ReadOnly,
    /// The storage holding it is full.
    NoSpace,
    /// Anything else, in the platform's words.
    Failed(String),
}

impl DocError {
    /// Why, as the end of "Couldn't open <name>: …".
    pub fn reason(&self) -> String {
        match self {
            DocError::Cancelled => "cancelled".into(),
            DocError::PermissionLost => "PhotoCraft no longer has permission to use it. Open it again with File › Open.".into(),
            DocError::Missing => "it isn't there any more (moved or deleted, or its storage was removed).".into(),
            DocError::ReadOnly => "the app that stores it only allows reading it.".into(),
            DocError::NoSpace => "there isn't enough space.".into(),
            DocError::Failed(why) => why.clone(),
        }
    }

    /// Whether File › Save should go on to Save As (FR-FIO-05): the document can't be written where it is.
    pub fn wants_save_as(&self) -> bool {
        matches!(self, DocError::PermissionLost | DocError::Missing | DocError::ReadOnly)
    }
}

/// The [`DocError`] for an exception the bridge caught: its class and message. Errno names in the
/// message decide first, since providers wrap them in whatever exception they like.
pub fn classify(class: &str, message: &str) -> DocError {
    let has = |s: &str| message.contains(s);
    let class = class.rsplit(['.', '/', '$']).next().unwrap_or(class);
    if has("ENOSPC") || has("No space left") || has("EDQUOT") {
        DocError::NoSpace
    } else if has("EROFS") || has("Read-only file system") {
        DocError::ReadOnly
    } else if class == "SecurityException" || has("EACCES") || has("EPERM") || has("Permission denied") {
        DocError::PermissionLost
    } else if class == "FileNotFoundException" || has("ENOENT") || has("No such file") || has("Missing file") {
        DocError::Missing
    } else if class == "UnsupportedOperationException" {
        DocError::ReadOnly
    } else if message.is_empty() {
        DocError::Failed(class.to_string())
    } else {
        DocError::Failed(message.to_string())
    }
}

/// The message of a failed save of `name` (shown as "Save failed: …"). `kept`: a complete copy
/// was staged and stays until a save succeeds.
fn save_failed(name: &str, e: &DocError, kept: bool) -> String {
    let mut msg = match e {
        DocError::PermissionLost => format!("PhotoCraft no longer has permission to change {name}."),
        DocError::Missing => format!("{name} isn't there any more (moved or deleted, or its storage was removed)."),
        DocError::ReadOnly => format!("{name} can't be changed where it is: the app that stores it only allows reading it."),
        DocError::NoSpace => format!("There isn't enough space to save {name}."),
        DocError::Cancelled => "cancelled".into(),
        DocError::Failed(why) => format!("{name}: {why}"),
    };
    if kept {
        msg.push_str(" The document is still open, and a complete copy of it is kept in PhotoCraft until a save succeeds.");
    }
    if e.wants_save_as() {
        msg.push_str(" Choose where to save it instead.");
    }
    msg
}

// ------------------------------------------------------------------ the platform

/// A system picker to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Picker {
    /// ACTION_OPEN_DOCUMENT: documents to read.
    Open { mime_types: Vec<String>, multiple: bool, initial: Option<String> },
    /// ACTION_CREATE_DOCUMENT: where to write a new document called `name`.
    Create { mime_type: String, name: String, initial: Option<String> },
}

/// A picker's answer: the chosen documents' URIs, or why there are none
/// ([`DocError::Cancelled`] when the user backed out).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picked {
    pub request: u32,
    pub outcome: Result<Vec<String>, DocError>,
}

/// What a provider says about a document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Described {
    pub name: String,
    pub mime: String,
    pub size: Option<u64>,
    /// PhotoCraft holds write permission and the provider supports writing.
    pub writable: bool,
}

/// A lasting permission PhotoCraft holds for a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grant {
    pub uri: String,
    pub write: bool,
}

/// A picker answer as `PhotoCraftActivity.takePickerResult` hands it over: `{request, "ok", uri…}`,
/// `{request, "cancel"}` or `{request, "error", message}`. `None` for anything else.
pub fn parse_picked(fields: &[String]) -> Option<Picked> {
    let (request, rest) = fields.split_first()?;
    let request = request.parse().ok()?;
    let (status, rest) = rest.split_first()?;
    let outcome = match status.as_str() {
        "ok" => Ok(rest.to_vec()),
        "cancel" => Err(DocError::Cancelled),
        "error" => Err(DocError::Failed(rest.first().cloned().unwrap_or_default())),
        _ => return None,
    };
    Some(Picked { request, outcome })
}

/// `describeDocument`'s answer: `{display name, MIME type, size, "1" when writable}`. Missing or
/// unreadable fields read as unknown.
pub fn parse_described(fields: &[String]) -> Described {
    let field = |i: usize| fields.get(i).cloned().unwrap_or_default();
    Described { name: field(0), mime: field(1), size: fields.get(2).and_then(|s| s.parse().ok()), writable: fields.get(3).is_some_and(|w| w == "1") }
}

/// `persistedGrants`' answer: `{uri, mode, uri, mode, …}`, modes "r", "w" or "rw".
pub fn parse_grants(flat: &[String]) -> Vec<Grant> {
    flat.as_chunks::<2>().0.iter().map(|[uri, mode]| Grant { uri: uri.clone(), write: mode.contains('w') }).collect()
}

/// The Android side of documents: pickers, provider queries, reads, writes and grants
/// (`bridge::Jni`). Every call may come from any thread.
pub trait Provider: Send + Sync + 'static {
    /// Show `picker` for request `request`; the answer comes from [`Self::take_result`].
    fn show(&self, request: u32, picker: &Picker) -> Result<(), DocError>;
    /// The oldest picker answer not yet taken.
    fn take_result(&self) -> Result<Option<Picked>, DocError>;
    fn describe(&self, uri: &str) -> Result<Described, DocError>;
    /// The document's whole contents, read in bounded chunks (`photocraft_format::read::read_all`).
    fn read(&self, uri: &str, size_hint: u64) -> Result<Vec<u8>, DocError>;
    /// Replace the document's contents with the first `len` bytes of `staged`.
    fn write(&self, uri: &str, staged: &File, len: u64) -> Result<(), DocError>;
    fn grants(&self) -> Result<Vec<Grant>, DocError>;
    fn release(&self, uri: &str) -> Result<(), DocError>;
}

// ------------------------------------------------------------------ staging (safe write)

/// What a staged copy is for: written next to its data as `<id>.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// The location the save was writing to.
    pub location: String,
    /// The document's name.
    pub name: String,
    /// The document's id in the session that staged it.
    pub doc: u64,
    /// That session ([`Staging::session`]).
    pub session: String,
    /// When it was staged (seconds since the Unix epoch).
    pub created: u64,
}

/// Staged copies in app storage (`noBackupFilesDir/saving`): `<id>.data` and, for saves,
/// `<id>.json`. Never backed up, never cleared by the system.
struct Staging {
    dir: PathBuf,
    /// This process's id for its staged copies: copies from other sessions are recoverable.
    session: String,
    next: AtomicU64,
}

/// A staged copy: its data file stays open for the provider to copy from.
struct Staged {
    id: String,
    file: File,
    len: u64,
}

fn now_s() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Staging {
    fn new(dir: PathBuf) -> Self {
        let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        let staging = Staging { dir, session: format!("{millis:x}{:x}", std::process::id()), next: AtomicU64::new(1) };
        staging.clean();
        staging
    }

    fn data(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.data"))
    }

    fn record(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    /// Writes `bytes` and syncs them, then (for a save) the record: only a copy with its record
    /// is complete. A failure leaves nothing behind.
    fn stage(&self, bytes: &[u8], record: Option<&Record>) -> std::io::Result<Staged> {
        fs::create_dir_all(&self.dir)?;
        let id = format!("{}-{}", self.session, self.next.fetch_add(1, Ordering::Relaxed));
        let data = self.data(&id);
        let mut file = OpenOptions::new().read(true).write(true).create_new(true).open(&data)?;
        let written = file.write_all(bytes).and_then(|()| file.sync_all()).and_then(|()| match record {
            Some(r) => {
                let text = serde_json::to_vec(r).map_err(std::io::Error::other)?;
                photocraft_format::atomic_write(&self.record(&id), &text)
            }
            None => Ok(()),
        });
        if let Err(e) = written {
            self.remove(&id);
            return Err(e);
        }
        Ok(Staged { id, file, len: bytes.len() as u64 })
    }

    /// Deletes a staged copy (record first, so a half-deleted copy is never offered).
    fn remove(&self, id: &str) {
        for path in [self.record(id), self.data(id)] {
            if let Err(e) = fs::remove_file(&path)
                && e.kind() != std::io::ErrorKind::NotFound
            {
                log::warn!("couldn't delete a staged save {}: {e}", path.display());
            }
        }
    }

    /// Complete copies from earlier sessions: (id, record), oldest first.
    fn pending(&self) -> Vec<(String, Record)> {
        let mut found: Vec<(String, Record)> = self
            .entries()
            .into_iter()
            .filter_map(|(id, record)| Some((id, record?)))
            .filter(|(id, r)| r.session != self.session && self.data(id).is_file())
            .collect();
        found.sort_by_key(|(_, r)| r.created);
        found
    }

    /// Every staged id in the folder, with its record when it has a readable one.
    fn entries(&self) -> Vec<(String, Option<Record>)> {
        let Ok(dir) = fs::read_dir(&self.dir) else { return Vec::new() };
        let mut ids: HashSet<String> = HashSet::new();
        for entry in dir.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(id) = name.strip_suffix(".data").or_else(|| name.strip_suffix(".json")) {
                ids.insert(id.to_string());
            }
        }
        ids.into_iter()
            .map(|id| {
                let record = fs::read(self.record(&id)).ok().and_then(|t| serde_json::from_slice::<Record>(&t).ok());
                (id, record)
            })
            .collect()
    }

    /// At start-up: drops what can't be recovered (data without a record: the process stopped
    /// while staging, before the target was touched; a record without data) and copies older
    /// than [`STAGED_MAX_AGE_S`].
    fn clean(&self) {
        let now = now_s();
        for (id, record) in self.entries() {
            let complete = record.as_ref().is_some_and(|r| r.session != self.session) && self.data(&id).is_file();
            let expired = record.as_ref().is_some_and(|r| now.saturating_sub(r.created) > STAGED_MAX_AGE_S);
            if !complete || expired {
                self.remove(&id);
            }
        }
    }
}

// ------------------------------------------------------------------ documents

/// What this session knows about a document: its URI, and whether File › Save may write it.
#[derive(Clone, Debug)]
struct Known {
    uri: String,
    writable: bool,
}

/// A picker request waiting for its answer.
enum Waiting {
    Open(FileDialogReply),
    Save { reply: FileDialogReply, name: String, ext: String },
}

/// A staged copy this session looks after: deleted once its document is saved or closed.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Kept {
    id: String,
    doc: u64,
    location: String,
}

#[derive(Default)]
struct State {
    next_request: u32,
    waiting: HashMap<u32, Waiting>,
    /// Workers still describing or reading documents.
    workers: usize,
    known: HashMap<String, Known>,
    /// Documents chosen or reopened that haven't reached the recent list or the tabs yet (still
    /// decoding, or saving in the background): their grants stay.
    fresh: HashSet<String>,
    /// Lasting grants were read into `known`.
    seeded: bool,
    /// Open failures for the UI to report: (document name, why).
    failures: Vec<(String, String)>,
    /// File › Save targets that can't be written: the UI goes on to Save As.
    save_as: Vec<String>,
    kept: Vec<Kept>,
    /// The recent list and the open documents' locations when grants were last reconciled.
    reconciled: Option<(Vec<String>, Vec<String>)>,
}

/// Encodes a document for a location (the format from its extension): bytes and warnings.
pub type EncodeFn =
    Arc<dyn Fn(&Document, &str, &photocraft_io::ExportOptions, &photocraft_raster::Interrupt) -> Result<(Vec<u8>, Vec<String>), String> + Send + Sync>;

struct Inner<P> {
    provider: P,
    staging: Staging,
    encode: EncodeFn,
    state: Mutex<State>,
}

/// The documents service: cheap to clone, shared by the service closures, the UI thread and
/// worker threads.
pub struct Documents<P> {
    inner: Arc<Inner<P>>,
}

impl<P> Clone for Documents<P> {
    fn clone(&self) -> Self {
        Documents { inner: Arc::clone(&self.inner) }
    }
}

/// Decrements the worker count when a worker ends, however it ends.
struct WorkerDone<P: Provider>(Documents<P>);

impl<P: Provider> Drop for WorkerDone<P> {
    fn drop(&mut self) {
        let mut state = self.0.state();
        state.workers = state.workers.saturating_sub(1);
    }
}

/// The real encoder: `photocraft_io`, behind a panic guard (the shell's last-resort guard,
/// AGENTS.md › Never crash).
pub fn default_encoder() -> EncodeFn {
    Arc::new(|doc, location, opts, interrupt| {
        guard("Save", || photocraft_io::export_with(doc, location, opts, interrupt).map(|r| (r.bytes, r.warnings)).map_err(|e| e.to_string()))
    })
}

/// Runs `f`, turning a panic into an error that keeps the document.
pub fn guard<T>(what: &str, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|_| {
        log::error!("{what} panicked");
        Err(format!("{what} failed unexpectedly; the document is unchanged"))
    })
}

impl<P: Provider> Documents<P> {
    /// Documents staged in `stage_dir` (cleaned of what can't be recovered).
    pub fn new(provider: P, stage_dir: PathBuf) -> Self {
        Self::with_encoder(provider, stage_dir, default_encoder())
    }

    /// [`Self::new`] with another encoder (tests inject failures).
    pub fn with_encoder(provider: P, stage_dir: PathBuf, encode: EncodeFn) -> Self {
        let inner = Inner { provider, staging: Staging::new(stage_dir), encode, state: Mutex::new(State::default()) };
        Documents { inner: Arc::new(inner) }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.inner.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Requests waiting for a picker, or workers still running: keep frames coming.
    pub fn busy(&self) -> bool {
        let state = self.state();
        !state.waiting.is_empty() || state.workers > 0
    }

    /// Reads the lasting grants into what this session knows, once.
    fn seed(&self) {
        if self.state().seeded {
            return;
        }
        match self.inner.provider.grants() {
            Ok(held) => {
                let mut state = self.state();
                state.seeded = true;
                for g in held {
                    state.known.entry(key_for(&g.uri)).or_insert(Known { uri: g.uri, writable: g.write });
                }
            }
            Err(e) => log::warn!("couldn't list document grants: {}", e.reason()),
        }
    }

    /// The URI behind a location or its folder part.
    fn uri_of(&self, location: &str) -> Option<String> {
        let key = key_of(location)?;
        self.seed();
        self.state().known.get(key).map(|k| k.uri.clone())
    }

    /// Records what a provider said about the document at `uri`, just chosen or reopened; returns
    /// its key.
    fn learn(&self, uri: &str, writable: bool) -> String {
        let key = key_for(uri);
        let mut state = self.state();
        state.known.insert(key.clone(), Known { uri: uri.to_string(), writable });
        state.fresh.insert(key.clone());
        key
    }

    // -------------------------------------------------------------- the file dialog service

    /// `Services::file_dialog`: shows a picker, or reads a recent document, and answers through
    /// `reply` once done (a later frame).
    pub fn show(&self, request: FileDialogRequest, reply: FileDialogReply) {
        match request {
            FileDialogRequest::Open { multiple, initial_dir, extensions } => {
                let initial = initial_dir.as_deref().and_then(|d| self.uri_of(d));
                let picker = Picker::Open { mime_types: open_mime_types(extensions.as_deref()), multiple, initial };
                self.start(picker, Waiting::Open(reply));
            }
            FileDialogRequest::Save { suggested } => {
                let (name, ext) = save_name(&suggested);
                let initial = self.uri_of(&suggested);
                let picker = Picker::Create { mime_type: CREATE_MIME.into(), name: name.clone(), initial };
                self.start(picker, Waiting::Save { reply, name, ext });
            }
            FileDialogRequest::Folder { .. } => reply.send(Some(FileDialogAnswer::Unavailable(NO_FOLDERS.into()))),
            FileDialogRequest::Reopen { location } => self.reopen(location, reply),
        }
    }

    fn start(&self, picker: Picker, waiting: Waiting) {
        let request = {
            let mut state = self.state();
            state.next_request = state.next_request.wrapping_add(1).max(1);
            let request = state.next_request;
            state.waiting.insert(request, waiting);
            request
        };
        if let Err(e) = self.inner.provider.show(request, &picker) {
            let waiting = self.state().waiting.remove(&request);
            if let Some(w) = waiting {
                reply_of(w).send(Some(FileDialogAnswer::Unavailable(format!("The document picker couldn't open: {}", e.reason()))));
            }
        }
    }

    /// File › Open Recent: reads the document at `location` without a picker.
    fn reopen(&self, location: String, reply: FileDialogReply) {
        let name = display_name(&location);
        let Some(uri) = self.uri_of(&location) else {
            self.state().failures.push((name, DocError::PermissionLost.reason()));
            reply.send(None);
            return;
        };
        self.spawn(move |docs| match docs.read_document(&uri) {
            Ok(file) => reply.send(Some(FileDialogAnswer::Located(vec![file]))),
            Err((_, e)) => {
                docs.state().failures.push((name, e.reason()));
                reply.send(None);
            }
        });
    }

    /// Describes and reads one document: its location and bytes, or its name (when the provider
    /// told it) and why it can't be read.
    fn read_document(&self, uri: &str) -> Result<(String, Vec<u8>), (String, DocError)> {
        let described = self.inner.provider.describe(uri).map_err(|e| ("a document".to_string(), e))?;
        let key = self.learn(uri, described.writable);
        let location = location(&key, &described.name);
        let bytes = self.inner.provider.read(uri, described.size.unwrap_or(0)).map_err(|e| (display_name(&location), e))?;
        Ok((location, bytes))
    }

    /// Runs `work` on a worker thread. If none can be started, `work` (and the reply it holds,
    /// which the app then takes as Cancel) is dropped, and the failure is reported.
    fn spawn(&self, work: impl FnOnce(&Self) + Send + 'static) {
        self.state().workers += 1;
        let docs = self.clone();
        let started = std::thread::Builder::new().name("documents".into()).spawn(move || {
            let _done = WorkerDone(docs.clone());
            work(&docs);
        });
        if let Err(e) = started {
            // The closure was dropped with the failed spawn (and its reply with it, which the
            // app takes as Cancel): say why.
            log::error!("couldn't start a documents worker: {e}");
            let mut state = self.state();
            state.workers = state.workers.saturating_sub(1);
            state.failures.push(("document".into(), format!("PhotoCraft couldn't start reading it ({e})")));
        }
    }

    /// Takes the pickers' answers and hands each to a worker. Asks the platform only while a
    /// picker is open, so idle frames make no JNI call.
    fn collect_answers(&self) {
        while !self.state().waiting.is_empty() {
            let picked = match self.inner.provider.take_result() {
                Ok(Some(p)) => p,
                Ok(None) => return,
                Err(e) => {
                    log::warn!("couldn't read a picker answer: {}", e.reason());
                    return;
                }
            };
            let waiting = {
                let mut state = self.state();
                // Java took lasting grants for these as the picker closed: keep them while the
                // documents are read and opened (see `reconcile`).
                if let Ok(uris) = &picked.outcome {
                    state.fresh.extend(uris.iter().map(|u| key_for(u)));
                }
                state.waiting.remove(&picked.request)
            };
            let Some(waiting) = waiting else {
                log::warn!("an answer for picker request {} that nothing waits for", picked.request);
                continue;
            };
            match (waiting, picked.outcome) {
                (w, Err(DocError::Cancelled)) => reply_of(w).send(None),
                (w, Err(e)) => reply_of(w).send(Some(FileDialogAnswer::Unavailable(e.reason()))),
                (w, Ok(uris)) if uris.is_empty() => reply_of(w).send(None),
                (Waiting::Open(reply), Ok(uris)) => self.spawn(move |docs| {
                    let mut files = Vec::new();
                    for uri in uris {
                        match docs.read_document(&uri) {
                            Ok(file) => files.push(file),
                            Err((name, e)) => docs.state().failures.push((name, e.reason())),
                        }
                    }
                    reply.send((!files.is_empty()).then_some(FileDialogAnswer::Located(files)));
                }),
                (Waiting::Save { reply, name, ext }, Ok(uris)) => self.spawn(move |docs| {
                    let Some(uri) = uris.into_iter().next() else { return reply.send(None) };
                    // A document the picker just created can be written; its name may differ
                    // from the one offered. A provider that can't describe it still gets the save.
                    let described = docs.inner.provider.describe(&uri).unwrap_or_default();
                    let key = docs.learn(&uri, true);
                    reply.send(Some(FileDialogAnswer::SaveTo(created_location(&key, &described.name, &name, &ext))));
                }),
            }
        }
    }

    // -------------------------------------------------------------- writing

    /// The URI to write a location to, or why it can't be.
    fn target(&self, location: &str) -> Result<String, DocError> {
        let key = key_of(location).ok_or_else(|| DocError::Failed("PhotoCraft on Android saves through the document picker only".into()))?;
        self.seed();
        let known = self.state().known.get(key).cloned().ok_or(DocError::PermissionLost)?;
        if !known.writable {
            return Err(DocError::ReadOnly);
        }
        Ok(known.uri)
    }

    /// Save (FR-FIO-05, FR-FIO-07): encode with `encode` (the target is untouched if it fails),
    /// stage the bytes, `commit` (the last point to cancel), then replace the target from the
    /// staged copy. A failed write keeps the copy for document `doc` and reports it.
    pub fn save(
        &self,
        doc: u64,
        location: &str,
        encode: impl FnOnce() -> Result<Vec<u8>, String>,
        commit: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        let name = display_name(location);
        let uri = match self.target(location) {
            Ok(uri) => uri,
            Err(e) => {
                if e.wants_save_as() {
                    self.state().save_as.push(location.to_string());
                }
                return Err(save_failed(&name, &e, false));
            }
        };
        let bytes = encode()?;
        let record = Record { location: location.to_string(), name: name.clone(), doc, session: self.inner.staging.session.clone(), created: now_s() };
        let staged = self
            .inner
            .staging
            .stage(&bytes, Some(&record))
            .map_err(|e| format!("{name} was not changed: PhotoCraft couldn't prepare the save in its own storage ({e})"))?;
        drop(bytes);
        if let Err(e) = commit() {
            self.inner.staging.remove(&staged.id);
            return Err(e);
        }
        match self.inner.provider.write(&uri, &staged.file, staged.len) {
            Ok(()) => {
                self.inner.staging.remove(&staged.id);
                // Copies kept from earlier failed saves of it there are no longer needed.
                let done: Vec<Kept> = {
                    let mut state = self.state();
                    let (done, kept) = std::mem::take(&mut state.kept).into_iter().partition(|k| k.doc == doc && k.location == location);
                    state.kept = kept;
                    done
                };
                for k in done {
                    self.inner.staging.remove(&k.id);
                }
                Ok(())
            }
            Err(e) => {
                let mut state = self.state();
                state.kept.push(Kept { id: staged.id.clone(), doc, location: location.to_string() });
                if e.wants_save_as() {
                    state.save_as.push(location.to_string());
                }
                Err(save_failed(&name, &e, true))
            }
        }
    }

    /// `Services::save_file`: [`Self::save`] with the real encoder, reporting progress to the job.
    pub fn save_document(&self, doc: &Document, location: &str, settings: &ExportSettings, ctl: &SaveCtl) -> Result<Vec<String>, String> {
        ctl.progress(0.0, "Encoding");
        let mut warnings = Vec::new();
        self.save(
            doc.id.0,
            location,
            || {
                let opts = export_options(settings);
                let cancelled = || ctl.check().is_err();
                let progress = |f: f32| ctl.progress(f * ENCODE_SHARE, "");
                let interrupt = photocraft_raster::Interrupt::new(&cancelled, &progress);
                let (bytes, w) = (self.inner.encode)(doc, location, &opts, &interrupt)?;
                warnings = w;
                Ok(bytes)
            },
            || {
                ctl.commit()?;
                ctl.progress(ENCODE_SHARE, "Writing");
                Ok(())
            },
        )?;
        ctl.progress(1.0, "");
        Ok(warnings)
    }

    /// `Services::write` (Export As, Quick Export, presets, swatches): the same staged write,
    /// without keeping the copy (the document itself is still open).
    pub fn write(&self, location: &str, bytes: &[u8]) -> Result<(), String> {
        let name = display_name(location);
        let uri = self.target(location).map_err(|e| save_failed(&name, &e, false))?;
        let staged =
            self.inner.staging.stage(bytes, None).map_err(|e| format!("{name} was not written: PhotoCraft couldn't prepare it in its own storage ({e})"))?;
        let written = self.inner.provider.write(&uri, &staged.file, staged.len);
        self.inner.staging.remove(&staged.id);
        written.map_err(|e| save_failed(&name, &e, false))?;
        // An export is done with its target: unless it is open or recent, its grant can go.
        if let Some(key) = key_of(location) {
            self.state().fresh.remove(key);
        }
        Ok(())
    }

    // -------------------------------------------------------------- recovery

    /// `Services::recover`: complete copies left by saves that failed or were cut off in earlier
    /// sessions. Each recovers as an unsaved document bound to the location it was saving to.
    pub fn recoverable(&self) -> Vec<Recoverable> {
        let staging = &self.inner.staging;
        staging
            .pending()
            .into_iter()
            .map(|(id, record)| {
                let data = staging.data(&id);
                let name = record.name.clone();
                Recoverable {
                    key: format!("{STAGED_KEY}{id}"),
                    name: record.name,
                    path: Some(record.location),
                    load: Box::new(move || {
                        let bytes = photocraft_format::read_file(&data).map_err(|e| format!("{name}: {e}"))?;
                        guard("Recovery", || photocraft_io::import(&name, &bytes).map(|r| r.document).map_err(|e| e.to_string()))
                    }),
                }
            })
            .collect()
    }

    /// `Services::adopt_autosave`: the recovered document `doc` now owns staged copy `key`.
    pub fn adopt(&self, doc: u64, key: &str) {
        let Some(id) = key.strip_prefix(STAGED_KEY) else { return };
        let location = self.inner.staging.pending().into_iter().find(|(i, _)| i == id).map(|(_, r)| r.location).unwrap_or_default();
        self.state().kept.push(Kept { id: id.to_string(), doc, location });
    }

    /// Drops the staged copies of documents that were saved or closed (`live`: open documents
    /// and whether they have unsaved changes).
    fn prune(&self, live: &HashMap<u64, bool>) {
        let gone: Vec<Kept> = {
            let mut state = self.state();
            if state.kept.is_empty() {
                return;
            }
            let (gone, kept) = std::mem::take(&mut state.kept).into_iter().partition(|k| live.get(&k.doc) != Some(&true));
            state.kept = kept;
            gone
        };
        for k in gone {
            self.inner.staging.remove(&k.id);
        }
    }

    // -------------------------------------------------------------- every frame

    /// Called every frame on the UI thread: delivers picker answers, reports failures, goes on
    /// to Save As where Save can't write, and keeps recents and grants in step.
    pub fn frame(&self, app: &mut PhotocraftApp, ctx: &egui::Context) {
        self.collect_answers();
        let (failures, save_as) = {
            let mut state = self.state();
            (std::mem::take(&mut state.failures), std::mem::take(&mut state.save_as))
        };
        for (name, why) in failures {
            app.open_failed(&name, &why);
        }
        for location in save_as {
            save_as_instead(app, &location);
        }
        self.reconcile(app);
        if self.busy() {
            // Answers arrive without an input event to wake the loop.
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    /// Open Recent keeps one entry per document and at most [`MAX_RECENT_GRANTS`] documents;
    /// grants go once their document is neither recent nor open.
    fn reconcile(&self, app: &mut PhotocraftApp) {
        let live: HashMap<u64, bool> = app.session.documents().iter().map(|d| (d.doc.id.0, d.is_dirty())).collect();
        self.prune(&live);
        let open: Vec<String> = app.session.documents().iter().filter_map(|d| d.path.clone()).filter(|p| key_of(p).is_some()).collect();
        let snapshot = (app.ui.recent_files.clone(), open);
        if self.state().reconciled.as_ref() == Some(&snapshot) {
            return;
        }
        for path in recents_to_drop(&app.ui.recent_files) {
            app.remove_recent(&path);
        }
        let (_, open) = snapshot;
        let mut keep: HashSet<String> = app.ui.recent_files.iter().chain(&open).filter_map(|l| key_of(l)).map(str::to_string).collect();
        self.seed();
        {
            let mut state = self.state();
            state.fresh.retain(|k| !keep.contains(k));
            keep.extend(state.fresh.iter().cloned());
        }
        match self.inner.provider.grants() {
            Ok(held) => {
                for uri in grants_to_release(&held, &keep) {
                    if let Err(e) = self.inner.provider.release(&uri) {
                        log::warn!("couldn't release a document grant: {}", e.reason());
                    }
                }
            }
            Err(e) => log::warn!("couldn't list document grants: {}", e.reason()),
        }
        self.state().reconciled = Some((app.ui.recent_files.clone(), open));
    }
}

fn reply_of(w: Waiting) -> FileDialogReply {
    match w {
        Waiting::Open(reply) | Waiting::Save { reply, .. } => reply,
    }
}

/// File › Save couldn't write the document at `location` where it is: ask where to save it
/// (FR-FIO-05). The save's own error has said why.
fn save_as_instead(app: &mut PhotocraftApp, location: &str) {
    let Some(index) = app.session.documents().iter().position(|d| d.path.as_deref() == Some(location)) else { return };
    app.session.set_active(index);
    if let Err(e) = app.save_as(None)
        && e != "cancelled"
    {
        app.ui.status = e;
        app.ui.status_error = true;
    }
}

/// The recent entries to take off the list: older entries for a document listed again under
/// another name (renamed since), and documents beyond the [`MAX_RECENT_GRANTS`] newest. Other
/// entries (not locations) are left alone.
pub fn recents_to_drop(recents: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut kept = 0;
    let mut drop = Vec::new();
    for r in recents {
        let Some(key) = key_of(r) else { continue };
        if !seen.insert(key) || kept >= MAX_RECENT_GRANTS {
            drop.push(r.clone());
        } else {
            kept += 1;
        }
    }
    drop
}

/// The held grants to give up: those for documents whose key is not in `keep`.
pub fn grants_to_release(held: &[Grant], keep: &HashSet<String>) -> Vec<String> {
    held.iter().filter(|g| !keep.contains(&key_for(&g.uri))).map(|g| g.uri.clone()).collect()
}

/// Export settings as encoder options (as the desktop and web shells map them).
pub fn export_options(settings: &ExportSettings) -> photocraft_io::ExportOptions {
    let mut opts = photocraft_io::ExportOptions::default();
    if let Some(q) = settings.jpeg_quality {
        opts.encode.jpeg_quality = q;
    }
    opts.encode.webp_lossless = settings.webp_lossless;
    if let Some(q) = settings.webp_quality {
        opts.encode.webp_quality = q;
    }
    opts.tiff_layers = settings.tiff_layers;
    opts.xmp = if settings.xmp_all { photocraft_io::XmpEmbed::All } else { photocraft_io::XmpEmbed::None };
    opts.tga_bits = settings.tga_bits;
    opts.image_previews = settings.image_previews;
    opts.psd_maximize_compatibility = settings.psd_maximize_compatibility;
    opts
}

/// The platform services for Android: codecs, documents through `docs`, staged-save recovery,
/// and preferences in `prefs` (`filesDir/preferences.json`, written atomically).
pub fn services<P: Provider>(docs: &Documents<P>, prefs: Option<PathBuf>) -> Services {
    let (dialog, writer, saver, recovery, adopter) = (docs.clone(), docs.clone(), docs.clone(), docs.clone(), docs.clone());
    let load = prefs.clone();
    Services {
        import: Some(Box::new(|name: &str, bytes: &[u8], max_svg_group_depth: usize| {
            guard("Open", || {
                photocraft_io::import_with_svg_group_depth(name, bytes, max_svg_group_depth).map(|r| (r.document, r.warnings)).map_err(|e| e.to_string())
            })
        })),
        export: Some(Box::new(|doc: &Document, path: &str, settings: &ExportSettings| {
            let opts = export_options(settings);
            guard("Export", || photocraft_io::export(doc, path, &opts).map(|r| (r.bytes, r.warnings)).map_err(|e| e.to_string()))
        })),
        encode_png: Some(Box::new(|w, h, rgba| {
            let img = photocraft_codecs::Image::from_u8(w, h, photocraft_codecs::ChannelLayout::Rgba, rgba.to_vec()).map_err(|e| e.to_string())?;
            photocraft_codecs::encode(&img, photocraft_codecs::Format::Png, &photocraft_codecs::EncodeOptions::default()).map_err(|e| e.to_string())
        })),
        file_dialog: Some(Box::new(move |request, _parent, reply| dialog.show(request, reply))),
        write: Some(Box::new(move |path: &str, bytes: &[u8]| writer.write(path, bytes))),
        save_file: Some(Arc::new(move |doc: &Document, path: &str, settings: &ExportSettings, ctl: &SaveCtl| saver.save_document(doc, path, settings, ctl))),
        recover: Some(Box::new(move || recovery.recoverable())),
        adopt_autosave: Some(Box::new(move |doc: u64, key: &str| adopter.adopt(doc, key))),
        load_prefs: Some(Box::new(move || load.as_ref().and_then(|p| fs::read_to_string(p).ok()))),
        save_prefs: Some(Box::new(move |text: &str| {
            let path = prefs.as_ref().ok_or("no app storage for preferences")?;
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            photocraft_format::atomic_write(path, text.as_bytes()).map_err(|e| e.to_string())
        })),
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "documents_tests.rs"]
mod tests;
