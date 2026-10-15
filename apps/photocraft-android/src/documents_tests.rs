//! Host tests for SAF documents (WP 1.5/1.6): the mapping code, and the whole app driving open,
//! save, Save As, Open Recent and recovery against a fake provider with fault injection.

use super::*;
use photocraft_engine::Session;
use photocraft_ui_egui::{jobs_ui, menus};
use serde_json::json;
use std::collections::VecDeque;
use std::io::{Read, Seek};

// ------------------------------------------------------------------ the fake provider

/// How the fake provider's next write goes.
#[derive(Clone, Debug)]
enum WriteFault {
    /// Fails before touching the document.
    Refuse(DocError),
    /// Writes the first `n` bytes, then fails (a provider or I/O error mid-write).
    Partial(usize, DocError),
    /// Writes the first `n` bytes, then the process dies: nothing after the write runs.
    Kill(usize),
}

#[derive(Clone, Debug)]
struct FakeDoc {
    name: String,
    bytes: Vec<u8>,
    writable: bool,
}

#[derive(Default)]
struct FakeState {
    docs: HashMap<String, FakeDoc>,
    shown: Vec<(u32, Picker)>,
    results: VecDeque<Picked>,
    grants: Vec<Grant>,
    released: Vec<String>,
    show_fault: Option<DocError>,
    write_fault: Option<WriteFault>,
    read_faults: HashMap<String, DocError>,
    writes: usize,
}

/// Documents in memory by URI, pickers the test answers, and faults to inject.
#[derive(Default)]
struct Fake(Mutex<FakeState>);

impl Fake {
    fn with<T>(&self, f: impl FnOnce(&mut FakeState) -> T) -> T {
        f(&mut self.0.lock().unwrap())
    }

    fn add(&self, uri: &str, name: &str, bytes: Vec<u8>, writable: bool) {
        self.with(|s| s.docs.insert(uri.into(), FakeDoc { name: name.into(), bytes, writable }));
    }

    fn bytes(&self, uri: &str) -> Vec<u8> {
        self.with(|s| s.docs.get(uri).map(|d| d.bytes.clone()).unwrap_or_default())
    }

    /// The last picker shown, and its request.
    fn last_shown(&self) -> (u32, Picker) {
        self.with(|s| s.shown.last().cloned().expect("a picker was shown"))
    }

    fn answer(&self, request: u32, outcome: Result<Vec<String>, DocError>) {
        self.with(|s| s.results.push_back(Picked { request, outcome }));
    }

    fn grant(&self, uri: &str, write: bool) {
        self.with(|s| s.grants.push(Grant { uri: uri.into(), write }));
    }
}

impl Provider for Arc<Fake> {
    fn show(&self, request: u32, picker: &Picker) -> Result<(), DocError> {
        self.with(|s| {
            if let Some(e) = s.show_fault.clone() {
                return Err(e);
            }
            s.shown.push((request, picker.clone()));
            Ok(())
        })
    }

    fn take_result(&self) -> Result<Option<Picked>, DocError> {
        Ok(self.with(|s| s.results.pop_front()))
    }

    fn describe(&self, uri: &str) -> Result<Described, DocError> {
        self.with(|s| {
            let d = s.docs.get(uri).ok_or(DocError::Missing)?;
            Ok(Described { name: d.name.clone(), mime: String::new(), size: Some(d.bytes.len() as u64), writable: d.writable })
        })
    }

    fn read(&self, uri: &str, _size_hint: u64) -> Result<Vec<u8>, DocError> {
        self.with(|s| {
            if let Some(e) = s.read_faults.get(uri) {
                return Err(e.clone());
            }
            s.docs.get(uri).map(|d| d.bytes.clone()).ok_or(DocError::Missing)
        })
    }

    fn write(&self, uri: &str, staged: &File, len: u64) -> Result<(), DocError> {
        let mut file = staged.try_clone().unwrap();
        file.rewind().unwrap();
        let mut bytes = Vec::new();
        file.take(len).read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes.len() as u64, len, "the staged copy holds the whole document before the target is touched");
        let fault = self.with(|s| {
            s.writes += 1;
            s.write_fault.take()
        });
        let put = |data: &[u8]| {
            self.with(|s| match s.docs.get_mut(uri) {
                Some(d) => d.bytes = data.to_vec(),
                None => panic!("write to an unknown document"),
            })
        };
        match fault {
            None => {
                put(&bytes);
                Ok(())
            }
            Some(WriteFault::Refuse(e)) => Err(e),
            Some(WriteFault::Partial(n, e)) => {
                put(&bytes[..n.min(bytes.len())]);
                Err(e)
            }
            Some(WriteFault::Kill(n)) => {
                put(&bytes[..n.min(bytes.len())]);
                panic!("process killed mid-write");
            }
        }
    }

    fn grants(&self) -> Result<Vec<Grant>, DocError> {
        Ok(self.with(|s| s.grants.clone()))
    }

    fn release(&self, uri: &str) -> Result<(), DocError> {
        self.with(|s| {
            s.grants.retain(|g| g.uri != uri);
            s.released.push(uri.into());
        });
        Ok(())
    }
}

// ------------------------------------------------------------------ helpers

/// A fresh staging folder for one test.
fn stage_dir(test: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("photocraft-saf-{test}-{nanos}-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Staged files left in `dir`.
fn staged_files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir).map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    names.sort();
    names
}

/// A small PSD filled with `color`.
fn psd(color: &str) -> Vec<u8> {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 8, "height": 8})).unwrap();
    s.execute("edit.fill", json!({"color": color})).unwrap();
    photocraft_io::export(&s.active().unwrap().doc, "art.psd", &photocraft_io::ExportOptions::default()).unwrap().bytes
}

/// What a document looks like, to compare documents: its composite as PNG.
fn look(doc: &Document) -> Vec<u8> {
    photocraft_io::export(doc, "look.png", &photocraft_io::ExportOptions::default()).unwrap().bytes
}

struct Rig {
    fake: Arc<Fake>,
    docs: Documents<Arc<Fake>>,
    app: PhotocraftApp,
    ctx: egui::Context,
    dir: PathBuf,
}

impl Rig {
    fn new(test: &str) -> Self {
        Self::with_encoder(test, default_encoder())
    }

    fn with_encoder(test: &str, encode: EncodeFn) -> Self {
        let fake = Arc::new(Fake::default());
        let dir = stage_dir(test);
        let docs = Documents::with_encoder(fake.clone(), dir.clone(), encode);
        let mut app = PhotocraftApp::new(Session::new(), services(&docs, None));
        app.background_jobs = true;
        Rig { fake, docs, app, ctx: egui::Context::default(), dir }
    }

    /// One frame of what the Android shell does: show and answer file dialogs, run the documents
    /// service, finish background jobs.
    fn frame(&mut self) {
        jobs_ui::tick(&mut self.app, &self.ctx);
        self.app.poll_file_dialog(&self.ctx, None);
        self.docs.frame(&mut self.app, &self.ctx);
    }

    /// Frames until `done`, or fails after a few seconds.
    fn until(&mut self, what: &str, done: impl Fn(&Self) -> bool) {
        for _ in 0..1000 {
            self.frame();
            if done(self) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("timed out waiting for {what}; status: {}", self.app.ui.status);
    }

    fn invoke(&mut self, id: &str) {
        let ctx = self.ctx.clone();
        menus::invoke(&mut self.app, &ctx, id, json!({})).unwrap();
    }

    /// File › Open of the documents at `uris`, through the picker.
    fn open(&mut self, uris: &[&str]) {
        let before = self.app.session.documents().len();
        self.invoke("file.open");
        self.frame();
        let (request, picker) = self.fake.last_shown();
        assert!(matches!(picker, Picker::Open { multiple: true, .. }), "{picker:?}");
        self.fake.answer(request, Ok(uris.iter().map(|u| u.to_string()).collect()));
        let want = before + uris.len();
        self.until("the documents to open", |r| r.app.session.documents().len() == want && r.idle());
    }

    /// No background job running.
    fn idle(&self) -> bool {
        self.app.session.jobs().is_empty()
    }

    fn active_path(&self) -> String {
        self.app.session.active().and_then(|d| d.path.clone()).unwrap_or_default()
    }

    fn dirty(&self) -> bool {
        self.app.session.active().is_some_and(|d| d.is_dirty())
    }

    /// File › Save, then frames until the save has ended.
    fn save(&mut self) {
        self.invoke("file.save");
        self.until("the save to end", |r| r.idle());
    }

    fn edit(&mut self, color: &str) {
        self.app.run("edit.fill", json!({"color": color})).unwrap();
        assert!(self.dirty());
    }
}

// ------------------------------------------------------------------ mapping

#[test]
fn locations_name_documents_by_key_and_display_name() {
    let uri = "content://com.android.externalstorage.documents/document/primary%3ADownload%2Fart.psd";
    let key = key_for(uri);
    assert_eq!(key.len(), 16);
    assert_eq!(key, key_for(uri), "stable");
    assert_ne!(key, key_for("content://other/document/1"));
    let loc = location(&key, "art.psd");
    assert_eq!(loc, format!("saf://{key}/art.psd"));
    assert_eq!(key_of(&loc), Some(key.as_str()));
    assert_eq!(key_of(&format!("saf://{key}")), Some(key.as_str()), "the folder part of a location");
    assert_eq!(display_name(&loc), "art.psd");
    assert_eq!(Path::new(&loc).extension().and_then(|e| e.to_str()), Some("psd"), "the format comes from the name");
    assert!(photocraft_ui_egui::file_open::is_location(&loc));
    for not_ours in ["/sdcard/art.psd", "saf://short/art.psd", "saf://zzzzzzzzzzzzzzzz/a", "content://x/y", ""] {
        assert_eq!(key_of(not_ours), None, "{not_ours}");
    }
}

#[test]
fn display_names_from_other_apps_are_made_safe() {
    assert_eq!(safe_name("photo.psd"), "photo.psd");
    assert_eq!(safe_name("../../etc/passwd"), ".._.._etc_passwd");
    assert_eq!(safe_name("a\\b\u{0}c\nd.png"), "a_b_c_d.png");
    assert_eq!(safe_name("  spaced.jpg  "), "spaced.jpg");
    for empty in ["", "   ", ".", "..", "..."] {
        assert_eq!(safe_name(empty), "Untitled", "{empty:?}");
    }
    let long = format!("{}.psd", "é".repeat(300));
    let safe = safe_name(&long);
    assert!(safe.len() <= 255 && safe.ends_with(".psd"), "{} bytes", safe.len());
    // The last part of a location never holds a separator, whatever the provider said.
    assert_eq!(display_name(&location("0123456789abcdef", "a/b.png")), "a_b.png");
}

#[test]
fn save_names_keep_the_chosen_format() {
    assert_eq!(save_name("saf://0123456789abcdef/art.psd"), ("art.psd".into(), "psd".into()));
    assert_eq!(save_name("Untitled-1.PNG"), ("Untitled-1.PNG".into(), "png".into()));
    assert_eq!(save_name("/data/x/notes"), ("notes.psd".into(), "psd".into()));
    assert_eq!(save_name(""), ("Untitled.psd".into(), "psd".into()));
    let key = "0123456789abcdef";
    // The provider made the name unique, or dropped the extension the user typed.
    assert_eq!(created_location(key, "art (1).psd", "art.psd", "psd"), format!("saf://{key}/art (1).psd"));
    assert_eq!(created_location(key, "art", "art.png", "png"), format!("saf://{key}/art.png"));
    assert_eq!(created_location(key, "", "art.png", "png"), format!("saf://{key}/art.png"));
}

/// Every format desktop File › Open accepts (`OPEN_EXTS` in apps/photocraft/src/services.rs).
const DESKTOP_OPEN_EXTS: &[&str] = &[
    "pcraft", "pdn", "ora", "kra", "krz", "psd", "psb", "psdt", "png", "jpg", "jpeg", "tif", "tiff", "webp", "gif", "bmp", "tga", "ico", "qoi", "exr", "hdr",
    "pbm", "pgm", "ppm", "pam", "pfm", "pcx", "dds", "xpm", "fits", "fit", "fts", "dpx", "heic", "heif", "hif", "dng", "cr2", "cr3", "nef", "nrw", "arw",
    "pef", "orf", "rw2", "raf", "abr", "grd", "svg", "svgz", "aco", "ase", "kys", "af", "afdesign", "afphoto", "afpub",
];

#[test]
fn open_pickers_offer_every_format_and_any_file() {
    for ext in DESKTOP_OPEN_EXTS {
        assert!(mime_for(ext).is_some(), "{ext} has no MIME type");
        assert!(mime_for(&ext.to_ascii_uppercase()).is_some(), "{ext} in capitals");
    }
    let all = open_mime_types(None);
    for ext in DESKTOP_OPEN_EXTS {
        assert!(all.iter().any(|m| Some(m.as_str()) == mime_for(ext)), "{ext}");
    }
    assert!(all.contains(&"*/*".to_string()), "files are identified by content, whatever the provider reports");
    assert_eq!(all.iter().collect::<HashSet<_>>().len(), all.len(), "no duplicates");
    assert_eq!(open_mime_types(Some(&[])), all, "an empty filter is no filter");
    // A command that reads images Android types reliably lists just those.
    assert_eq!(open_mime_types(Some(&["jpg".into(), "JPEG".into(), "png".into()])), ["image/jpeg", "image/png"]);
    // Photoshop brushes have no type providers report: any file.
    assert_eq!(open_mime_types(Some(&["abr".into()])), ["application/octet-stream", "*/*"]);
    assert!(open_mime_types(Some(&["psd".into()])).contains(&"*/*".to_string()), "providers often report PSD as octet-stream");
    assert!(open_mime_types(Some(&[".cube".into()])).contains(&"*/*".to_string()), "unknown extensions list any file");
}

#[test]
fn create_pickers_are_typeless_so_the_typed_name_decides_the_format() {
    // A typed create made Android's picker append `.jpg` to a name typed as `.pcraft` (Galaxy
    // S20+): every create is typeless, and the name the provider reports decides the format.
    assert_eq!(CREATE_MIME, "application/octet-stream");
    assert_eq!(created_location("k", "photo.pcraft", "photo.jpg", "jpg"), "saf://k/photo.pcraft");
    assert_eq!(created_location("k", "photo", "photo.jpg", "jpg"), "saf://k/photo.jpg", "a name typed without an extension keeps the offered format");
}

#[test]
fn recents_keep_one_entry_per_document_and_at_most_30() {
    let loc = |i: usize, name: &str| location(&key_for(&format!("content://p/{i}")), name);
    // Document 1 was renamed: its newer entry (first) stays.
    let mut recents = vec![loc(1, "new.psd"), "/sdcard/desktop-path.psd".into(), loc(2, "b.psd"), loc(1, "old.psd")];
    assert_eq!(recents_to_drop(&recents), [loc(1, "old.psd")]);
    recents = (0..35).map(|i| loc(i, "x.psd")).collect();
    recents.insert(3, "/not/a/location.png".into());
    let drop = recents_to_drop(&recents);
    assert_eq!(drop, (30..35).map(|i| loc(i, "x.psd")).collect::<Vec<_>>(), "the oldest beyond 30 go; other entries stay");
    assert!(recents_to_drop(&recents[..10]).is_empty());
}

#[test]
fn grants_go_once_their_document_is_neither_recent_nor_open() {
    let held: Vec<Grant> = (0..4).map(|i| Grant { uri: format!("content://p/{i}"), write: true }).collect();
    let keep: HashSet<String> = [key_for("content://p/0"), key_for("content://p/2")].into_iter().collect();
    assert_eq!(grants_to_release(&held, &keep), ["content://p/1", "content://p/3"]);
    assert!(grants_to_release(&[], &keep).is_empty());
}

#[test]
fn exceptions_map_to_errors_the_user_can_act_on() {
    assert_eq!(classify("java.lang.SecurityException", "Permission Denial: reading ..."), DocError::PermissionLost);
    assert_eq!(classify("java.io.FileNotFoundException", "No such document"), DocError::Missing);
    assert_eq!(classify("java.io.FileNotFoundException", "open failed: EACCES (Permission denied)"), DocError::PermissionLost);
    assert_eq!(classify("java.io.IOException", "write failed: ENOSPC (No space left on device)"), DocError::NoSpace);
    assert_eq!(classify("java.io.IOException", "open failed: EROFS (Read-only file system)"), DocError::ReadOnly);
    assert_eq!(classify("java/lang/UnsupportedOperationException", ""), DocError::ReadOnly);
    assert_eq!(classify("java.io.IOException", "Broken pipe"), DocError::Failed("Broken pipe".into()));
    assert_eq!(classify("java.io.EOFException", ""), DocError::Failed("EOFException".into()));
    for e in [DocError::PermissionLost, DocError::Missing, DocError::ReadOnly] {
        assert!(e.wants_save_as(), "{e:?}");
        assert!(save_failed("art.psd", &e, false).contains("Choose where to save it"), "{e:?}");
    }
    for e in [DocError::NoSpace, DocError::Failed("x".into()), DocError::Cancelled] {
        assert!(!e.wants_save_as(), "{e:?}");
    }
    let kept = save_failed("art.psd", &DocError::Failed("Broken pipe".into()), true);
    assert!(kept.starts_with("art.psd: Broken pipe") && kept.contains("still open") && kept.contains("complete copy"), "{kept}");
}

#[test]
fn bridge_answers_parse() {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(parse_picked(&s(&["7", "ok", "content://a", "content://b"])), Some(Picked { request: 7, outcome: Ok(s(&["content://a", "content://b"])) }));
    assert_eq!(parse_picked(&s(&["7", "cancel"])), Some(Picked { request: 7, outcome: Err(DocError::Cancelled) }));
    assert_eq!(
        parse_picked(&s(&["3", "error", "No document picker"])),
        Some(Picked { request: 3, outcome: Err(DocError::Failed("No document picker".into())) })
    );
    for bad in [s(&[]), s(&["x", "ok"]), s(&["1"]), s(&["1", "maybe"])] {
        assert_eq!(parse_picked(&bad), None, "{bad:?}");
    }
    assert_eq!(
        parse_described(&s(&["art.psd", "image/vnd.adobe.photoshop", "1234", "1"])),
        Described { name: "art.psd".into(), mime: "image/vnd.adobe.photoshop".into(), size: Some(1234), writable: true }
    );
    assert_eq!(parse_described(&s(&["", "", "", "0"])), Described::default());
    assert_eq!(parse_described(&s(&["short"])), Described { name: "short".into(), ..Described::default() });
    assert_eq!(
        parse_grants(&s(&["content://a", "rw", "content://b", "r", "content://dangling"])),
        [Grant { uri: "content://a".into(), write: true }, Grant { uri: "content://b".into(), write: false }]
    );
}

// ------------------------------------------------------------------ open

#[test]
fn file_open_reads_the_picked_documents_and_keeps_their_locations() {
    let mut r = Rig::new("open");
    r.fake.add("content://p/1", "red.psd", psd("#ff0000"), true);
    r.fake.add("content://p/2", "blue.psd", psd("#0000ff"), true);
    r.open(&["content://p/2", "content://p/1"]);
    let paths: Vec<String> = r.app.session.documents().iter().filter_map(|d| d.path.clone()).collect();
    let (red, blue) = (location(&key_for("content://p/1"), "red.psd"), location(&key_for("content://p/2"), "blue.psd"));
    assert_eq!(paths, [blue.clone(), red.clone()]);
    let names: Vec<String> = r.app.session.documents().iter().map(|d| d.doc.name.clone()).collect();
    assert_eq!(names, ["blue.psd", "red.psd"], "tabs show the provider's names");
    assert!(r.app.ui.recent_files.contains(&red) && r.app.ui.recent_files.contains(&blue), "both go to Open Recent");
    assert!(!r.dirty());
    // The picker offered every format.
    let (_, picker) = r.fake.last_shown();
    let Picker::Open { mime_types, .. } = picker else { panic!() };
    assert!(mime_types.contains(&"*/*".to_string()));
}

#[test]
fn cancel_ends_quietly_and_a_picker_that_cannot_show_says_why() {
    let mut r = Rig::new("cancel");
    r.invoke("file.open");
    r.frame();
    let (request, _) = r.fake.last_shown();
    r.fake.answer(request, Err(DocError::Cancelled));
    r.until("the answer", |r| !r.app.file_dialog_open());
    assert!(r.app.session.documents().is_empty());
    assert!(!r.app.ui.status_error, "{}", r.app.ui.status);
    // An empty answer is a cancel too.
    r.invoke("file.open");
    r.frame();
    let (request, _) = r.fake.last_shown();
    r.fake.answer(request, Ok(Vec::new()));
    r.until("the answer", |r| !r.app.file_dialog_open());
    assert!(!r.app.ui.status_error);
    r.fake.with(|s| s.show_fault = Some(DocError::Failed("No document picker on this device".into())));
    r.invoke("file.open");
    r.until("the error", |r| !r.app.file_dialog_open());
    assert!(r.app.ui.status_error && r.app.ui.status.contains("No document picker"), "{}", r.app.ui.status);
}

/// The error notices shown so far.
fn errors(app: &PhotocraftApp) -> Vec<String> {
    app.ui.notices.iter().filter(|n| n.error).map(|n| n.title.clone()).collect()
}

#[test]
fn one_unreadable_document_does_not_stop_the_others() {
    let mut r = Rig::new("partial-open");
    r.fake.add("content://p/1", "fine.psd", psd("#00ff00"), true);
    r.fake.add("content://p/2", "gone.psd", psd("#000000"), true);
    r.fake.with(|s| s.read_faults.insert("content://p/2".into(), DocError::Missing));
    r.invoke("file.open");
    r.frame();
    let (request, _) = r.fake.last_shown();
    r.fake.answer(request, Ok(vec!["content://p/1".into(), "content://p/2".into(), "content://p/unknown".into()]));
    r.until("the readable one to open", |r| r.app.session.documents().len() == 1 && r.idle() && errors(&r.app).len() == 2);
    assert_eq!(r.app.session.active().unwrap().doc.name, "fine.psd");
    let errors = errors(&r.app);
    assert!(errors.contains(&"Couldn't open gone.psd: it isn't there any more (moved or deleted, or its storage was removed).".to_string()), "{errors:?}");
    assert!(errors.iter().any(|e| e.starts_with("Couldn't open a document: ")), "{errors:?}");
}

// ------------------------------------------------------------------ save

#[test]
fn save_writes_back_through_a_staged_copy() {
    let mut r = Rig::new("save");
    r.fake.add("content://p/1", "art.psd", psd("#ff0000"), true);
    r.open(&["content://p/1"]);
    r.edit("#00ff00");
    let want = look(&r.app.session.active().unwrap().doc);
    r.save();
    assert!(!r.dirty(), "saved: {}", r.app.ui.status);
    let written = photocraft_io::import("art.psd", &r.fake.bytes("content://p/1")).unwrap().document;
    assert_eq!(look(&written), want, "the provider holds the edited document");
    assert!(staged_files(&r.dir).is_empty(), "nothing is kept after a successful save: {:?}", staged_files(&r.dir));
    assert_eq!(r.active_path(), location(&key_for("content://p/1"), "art.psd"));
}

#[test]
fn an_encode_error_leaves_the_target_untouched_and_the_document_unsaved() {
    let failing: EncodeFn = Arc::new(|_, _, _, _| Err("the encoder ran out of memory".into()));
    let mut r = Rig::with_encoder("encode-error", failing);
    let original = psd("#ff0000");
    r.fake.add("content://p/1", "art.psd", original.clone(), true);
    r.open(&["content://p/1"]);
    r.edit("#00ff00");
    r.save();
    assert!(r.dirty(), "the edits are still unsaved");
    assert!(r.app.ui.status_error && r.app.ui.status.contains("ran out of memory"), "{}", r.app.ui.status);
    assert_eq!(r.fake.bytes("content://p/1"), original, "the document's file is untouched");
    assert_eq!(r.fake.with(|s| s.writes), 0, "the provider was never asked to write");
    assert!(staged_files(&r.dir).is_empty());
}

#[test]
fn a_write_error_keeps_a_complete_copy_until_a_save_succeeds() {
    let mut r = Rig::new("write-error");
    r.fake.add("content://p/1", "art.psd", psd("#ff0000"), true);
    r.open(&["content://p/1"]);
    r.edit("#00ff00");
    let want = look(&r.app.session.active().unwrap().doc);
    let location = r.active_path();
    r.fake.with(|s| s.write_fault = Some(WriteFault::Partial(10, DocError::Failed("Broken pipe".into()))));
    r.save();
    assert!(r.dirty(), "a failed save leaves the document unsaved");
    assert!(r.app.ui.status.contains("Broken pipe") && r.app.ui.status.contains("complete copy"), "{}", r.app.ui.status);
    assert_eq!(r.fake.bytes("content://p/1").len(), 10, "the provider was left with part of the file");
    assert_eq!(staged_files(&r.dir).len(), 2, "data and record: {:?}", staged_files(&r.dir));
    // Were PhotoCraft to stop now, the next start would offer the document back, saved to the
    // same place.
    let next = Documents::new(Arc::new(Fake::default()), r.dir.clone());
    let offered = next.recoverable();
    assert_eq!(offered.len(), 1);
    let entry = offered.into_iter().next().unwrap();
    assert_eq!((entry.name.as_str(), entry.path.as_deref()), ("art.psd", Some(location.as_str())));
    assert_eq!(look(&(entry.load)().unwrap()), want, "the kept copy is the whole edited document");
    // The provider recovers; Save again succeeds and the copy goes.
    r.save();
    assert!(!r.dirty(), "{}", r.app.ui.status);
    assert!(staged_files(&r.dir).is_empty(), "{:?}", staged_files(&r.dir));
    assert_eq!(look(&photocraft_io::import("art.psd", &r.fake.bytes("content://p/1")).unwrap().document), want);
}

#[test]
fn a_kill_mid_write_never_loses_the_document() {
    let dir = stage_dir("kill");
    let fake = Arc::new(Fake::default());
    let original = psd("#ff0000");
    fake.add("content://p/1", "art.psd", original.clone(), true);
    let docs = Documents::new(fake.clone(), dir.clone());
    docs.learn("content://p/1", true);
    let loc = location(&key_for("content://p/1"), "art.psd");
    let mut session = Session::new();
    session.add_document(photocraft_io::import("art.psd", &original).unwrap().document, Some(loc.clone()));
    session.execute("edit.fill", json!({"color": "#00ff00"})).unwrap();
    let doc = session.active().unwrap().doc.clone();
    let want = look(&doc);
    fake.with(|s| s.write_fault = Some(WriteFault::Kill(64)));
    // The process stops inside the provider's write: no error path, no clean-up runs.
    let killed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        docs.save(doc.id.0, &loc, || Ok(photocraft_io::export(&doc, &loc, &photocraft_io::ExportOptions::default()).unwrap().bytes), || Ok(()))
    }));
    assert!(killed.is_err());
    drop(docs);
    assert_eq!(fake.bytes("content://p/1").len(), 64, "the document's file is cut short");
    // The next start: the recovery flow offers the whole edited document, bound to its location,
    // so File › Save finishes the job.
    let restarted = Documents::new(fake.clone(), dir.clone());
    let offered = restarted.recoverable();
    assert_eq!(offered.len(), 1, "{:?}", staged_files(&dir));
    let entry = offered.into_iter().next().unwrap();
    assert_eq!(entry.path.as_deref(), Some(loc.as_str()));
    let recovered = (entry.load)().unwrap();
    assert_eq!(look(&recovered), want);
    // Adopted by the recovered document, the copy stays until that document is saved or closed.
    restarted.adopt(41, &entry.key);
    restarted.prune(&HashMap::from([(41, true)]));
    assert_eq!(staged_files(&dir).len(), 2);
    restarted.prune(&HashMap::new());
    assert!(staged_files(&dir).is_empty(), "closed without saving: the user let it go");
}

#[test]
fn a_cancelled_save_writes_nothing_and_keeps_nothing() {
    let dir = stage_dir("cancelled");
    let fake = Arc::new(Fake::default());
    fake.add("content://p/1", "art.psd", b"original".to_vec(), true);
    let docs = Documents::new(fake.clone(), dir.clone());
    docs.learn("content://p/1", true);
    let loc = location(&key_for("content://p/1"), "art.psd");
    let r = docs.save(1, &loc, || Ok(b"new bytes".to_vec()), || Err("cancelled".into()));
    assert_eq!(r, Err("cancelled".into()));
    assert_eq!(fake.bytes("content://p/1"), b"original");
    assert!(staged_files(&dir).is_empty());
}

#[test]
fn save_goes_to_save_as_where_the_document_cannot_be_written() {
    for (case, writable, fault) in [
        ("read-only", false, None),
        ("permission-lost", true, Some(WriteFault::Refuse(DocError::PermissionLost))),
        ("deleted", true, Some(WriteFault::Refuse(DocError::Missing))),
    ] {
        let mut r = Rig::new(case);
        let original = psd("#ff0000");
        r.fake.add("content://p/1", "art.psd", original.clone(), writable);
        r.open(&["content://p/1"]);
        r.edit("#00ff00");
        r.fake.with(|s| s.write_fault = fault);
        r.invoke("file.save");
        r.until("the Save As picker", |r| matches!(r.fake.with(|s| s.shown.last().cloned()), Some((_, Picker::Create { .. }))));
        assert!(r.app.ui.status.contains("Choose where to save it"), "{case}: {}", r.app.ui.status);
        let (request, picker) = r.fake.last_shown();
        assert_eq!(
            picker,
            Picker::Create { mime_type: "application/octet-stream".into(), name: "art.psd".into(), initial: Some("content://p/1".into()) },
            "{case}"
        );
        r.fake.add("content://p/new", "art (1).psd", Vec::new(), true);
        r.fake.answer(request, Ok(vec!["content://p/new".into()]));
        r.until("the save to the new document", |r| !r.dirty() && r.idle());
        assert_eq!(r.active_path(), location(&key_for("content://p/new"), "art (1).psd"), "{case}");
        assert_eq!(r.fake.bytes("content://p/1"), original, "{case}: the original is untouched");
        assert!(photocraft_io::import("art.psd", &r.fake.bytes("content://p/new")).is_ok(), "{case}");
        assert!(staged_files(&r.dir).is_empty(), "{case}: {:?}", staged_files(&r.dir));
    }
}

#[test]
fn exports_write_a_new_document_and_keep_no_copy() {
    let dir = stage_dir("export");
    let fake = Arc::new(Fake::default());
    fake.add("content://p/out", "art.png", Vec::new(), true);
    let docs = Documents::new(fake.clone(), dir.clone());
    docs.learn("content://p/out", true);
    let loc = location(&key_for("content://p/out"), "art.png");
    assert_eq!(docs.write(&loc, b"png bytes"), Ok(()));
    assert_eq!(fake.bytes("content://p/out"), b"png bytes");
    fake.with(|s| s.write_fault = Some(WriteFault::Refuse(DocError::NoSpace)));
    let e = docs.write(&loc, b"more").unwrap_err();
    assert!(e.contains("enough space"), "{e}");
    assert!(staged_files(&dir).is_empty(), "{:?}", staged_files(&dir));
    // Only document locations can be written: there are no paths on Android.
    assert!(docs.write("/sdcard/Download/art.png", b"x").unwrap_err().contains("document picker"));
    let unknown = location(&key_for("content://p/never-picked"), "x.png");
    assert!(docs.write(&unknown, b"x").unwrap_err().contains("permission"));
}

// ------------------------------------------------------------------ Open Recent and grants

#[test]
fn open_recent_reopens_through_the_lasting_grant() {
    let mut r = Rig::new("recent");
    r.fake.add("content://p/1", "art.psd", psd("#ff0000"), true);
    r.fake.grant("content://p/1", true);
    let loc = location(&key_for("content://p/1"), "art.psd");
    r.app.push_recent(&loc);
    r.invoke("file.openRecent.0");
    r.until("the recent document", |r| r.app.session.documents().len() == 1 && r.idle());
    assert_eq!(r.active_path(), loc);
    assert!(r.fake.with(|s| s.shown.is_empty()), "no picker for a recent document");
    // It can be saved where it is: the grant says writable.
    r.edit("#0000ff");
    r.save();
    assert!(!r.dirty(), "{}", r.app.ui.status);
}

#[test]
fn a_recent_document_that_is_gone_or_no_longer_permitted_says_so() {
    let mut r = Rig::new("recent-gone");
    r.fake.grant("content://p/deleted", true);
    let deleted = location(&key_for("content://p/deleted"), "deleted.psd");
    let revoked = location(&key_for("content://p/revoked"), "revoked.psd");
    r.app.push_recent(&deleted);
    r.app.push_recent(&revoked);
    r.invoke("file.openRecent.0");
    r.until("the message", |r| !r.app.file_dialog_open() && r.app.ui.status_error);
    assert!(r.app.ui.status.contains("revoked.psd") && r.app.ui.status.contains("no longer has permission"), "{}", r.app.ui.status);
    r.app.ui.status_error = false;
    r.invoke("file.openRecent.1");
    r.until("the message", |r| !r.app.file_dialog_open() && r.app.ui.status_error && !r.docs.busy());
    assert!(r.app.ui.status.contains("deleted.psd") && r.app.ui.status.contains("isn't there any more"), "{}", r.app.ui.status);
    assert!(r.app.session.documents().is_empty());
}

#[test]
fn recents_and_grants_stay_in_step() {
    let mut r = Rig::new("grants");
    for i in 0..40 {
        r.fake.grant(&format!("content://p/{i}"), true);
    }
    // 34 documents in Open Recent (oldest first in, so 33 is the newest), one listed twice
    // under an old name, and one desktop-style path.
    r.app.session.prefs.edit(|p| p.file_handling.recent_file_count = 100);
    for i in 0..34 {
        r.app.push_recent(&location(&key_for(&format!("content://p/{i}")), &format!("doc{i}.psd")));
    }
    r.app.push_recent(&location(&key_for("content://p/33"), "renamed.psd"));
    r.app.push_recent("/storage/emulated/0/legacy.png");
    r.frame();
    let saf: Vec<&String> = r.app.ui.recent_files.iter().filter(|p| key_of(p).is_some()).collect();
    assert_eq!(saf.len(), MAX_RECENT_GRANTS, "{:?}", r.app.ui.recent_files);
    assert!(r.app.ui.recent_files.contains(&"/storage/emulated/0/legacy.png".to_string()));
    assert!(r.app.ui.recent_files.contains(&location(&key_for("content://p/33"), "renamed.psd")));
    assert!(!r.app.ui.recent_files.contains(&location(&key_for("content://p/33"), "doc33.psd")), "the old name goes");
    // Grants: the 30 recent documents keep theirs (33 down to 4); the rest are released.
    let held: HashSet<String> = r.fake.with(|s| s.grants.iter().map(|g| g.uri.clone()).collect());
    assert_eq!(held.len(), MAX_RECENT_GRANTS, "{held:?}");
    assert!(held.contains("content://p/4") && !held.contains("content://p/3") && !held.contains("content://p/39"));
    // Clearing Open Recent releases every grant no open document needs.
    r.app.clear_recent();
    r.frame();
    assert!(r.fake.with(|s| s.grants.is_empty()));
}

#[test]
fn a_document_still_opening_keeps_its_grant() {
    let mut r = Rig::new("fresh");
    r.fake.add("content://p/1", "art.psd", psd("#ff0000"), true);
    r.invoke("file.open");
    r.frame();
    let (request, _) = r.fake.last_shown();
    // As the picker closes, Java takes the lasting grant; then, before the document is read and
    // listed anywhere, something else changes Open Recent, so grants are reconciled.
    r.fake.grant("content://p/1", true);
    r.fake.answer(request, Ok(vec!["content://p/1".into()]));
    r.app.push_recent("/elsewhere/other.png");
    r.frame();
    assert!(r.fake.with(|s| s.released.is_empty()), "the new document's grant stays");
    r.until("the document to open", |r| r.app.session.documents().len() == 1 && r.idle());
    r.frame();
    assert!(r.fake.with(|s| s.grants.iter().any(|g| g.uri == "content://p/1")));
    // Once it is neither open nor recent, its grant goes.
    r.app.session.close(0);
    r.app.clear_recent();
    r.frame();
    assert_eq!(r.fake.with(|s| s.released.clone()), ["content://p/1"]);
}

// ------------------------------------------------------------------ staging housekeeping

#[test]
fn start_up_drops_what_cannot_be_recovered() {
    let dir = stage_dir("clean");
    let old = Record { location: "saf://0123456789abcdef/a.psd".into(), name: "a.psd".into(), doc: 1, session: "earlier".into(), created: now_s() };
    let write = |id: &str, data: bool, record: Option<&Record>| {
        if data {
            fs::write(dir.join(format!("{id}.data")), b"bytes").unwrap();
        }
        if let Some(r) = record {
            fs::write(dir.join(format!("{id}.json")), serde_json::to_vec(r).unwrap()).unwrap();
        }
    };
    write("complete", true, Some(&old));
    write("staging-interrupted", true, None);
    write("data-gone", false, Some(&old));
    write("expired", true, Some(&Record { created: now_s() - STAGED_MAX_AGE_S - 10, ..old.clone() }));
    fs::write(dir.join("broken.json"), b"{not json").unwrap();
    let docs = Documents::new(Arc::new(Fake::default()), dir.clone());
    assert_eq!(staged_files(&dir), ["complete.data", "complete.json"]);
    let offered = docs.recoverable();
    assert_eq!(offered.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(), ["staged:complete"]);
    // A copy that no longer decodes fails to load with a message, never a crash.
    let entry = offered.into_iter().next().unwrap();
    assert!((entry.load)().is_err());
}

#[test]
fn copies_of_documents_saved_or_closed_since_are_dropped() {
    let dir = stage_dir("prune");
    let fake = Arc::new(Fake::default());
    fake.add("content://p/1", "a.psd", Vec::new(), true);
    let docs = Documents::new(fake.clone(), dir.clone());
    docs.learn("content://p/1", true);
    let loc = location(&key_for("content://p/1"), "a.psd");
    for doc in [1, 2] {
        fake.with(|s| s.write_fault = Some(WriteFault::Refuse(DocError::Failed("Broken pipe".into()))));
        assert!(docs.save(doc, &loc, || Ok(b"bytes".to_vec()), || Ok(())).is_err());
    }
    assert_eq!(staged_files(&dir).len(), 4);
    // Document 1 is still open with its changes; document 2 was saved (no unsaved changes).
    docs.prune(&HashMap::from([(1, true), (2, false)]));
    assert_eq!(staged_files(&dir).len(), 2);
    docs.prune(&HashMap::from([(1, true)]));
    assert_eq!(staged_files(&dir).len(), 2);
    docs.prune(&HashMap::new());
    assert!(staged_files(&dir).is_empty());
}
