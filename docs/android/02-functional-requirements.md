# 02: Functional requirements

| | |
|---|---|
| **Version** | 1.0 draft, 2026-10-13 |
| **Scope** | What the Android edition must do. How it does it is in [04](04-technical-architecture.md); touch design detail is in [05](05-ux-touch-design.md) |
| **Priorities** | Must, Should, Could or Won't for v1.0 (see [README](README.md#4-conventions)) |

Each requirement has an ID, a priority, and acceptance criteria (AC) that [07](07-test-qa-plan.md) turns into test
cases. "Desktop behaviour" means the behaviour of the upstream desktop app at the baseline commit.

---

## 1. App lifecycle and platform integration (FR-LC)

| ID | Requirement | Priority | Acceptance criteria |
|---|---|---|---|
| FR-LC-01 | Launching from the launcher shows the system splash screen (Android 12 SplashScreen API, with the PhotoCraft icon) until the first frame renders, then the PhotoCraft UI | Must | No white flash in light or dark mode; the splash ends at the first rendered frame; cold start meets NFR-PERF-01 |
| FR-LC-02 | The app is a single activity (a GameActivity subclass) that handles configuration changes in-process: rotation, resize, fold and unfold, multi-window, keyboard attach, density, `uiMode` and locale. None of them restarts the native app or loses state | Must | Rotate, fold and resize 20 times during an open document and an in-progress dialog: no restart (logs show a single `android_main`), document and UI state intact |
| FR-LC-03 | The GPU surface is released when the window is hidden and recreated when it returns; the app never draws to a destroyed surface | Must | 50 home/return cycles and 20 lock/unlock cycles with no crash, black screen or leaked GPU memory (GPU memory stable within ±5%) |
| FR-LC-04 | When the activity goes to the background, the app immediately flushes autosave for every modified document into the recovery store, beyond the periodic autosave | Must | Edit, press Home, `adb shell am kill <pkg>`, relaunch: recovery offers the document with the last edit; flush time meets NFR-PERF-09 |
| FR-LC-05 | After process death or a crash, the next launch lists recoverable documents through the existing recovery flow and says why the previous session ended (crash, ANR or low memory, from `ApplicationExitInfo`) | Must | Simulated native crash, ANR and low-memory kill each produce the matching message and a working recovery |
| FR-LC-06 | System back (gesture or button) works through the predictive-back callback (apps targeting API 36 no longer receive `KEYCODE_BACK`). In order, it closes the topmost popup, menu or dialog; then cancels an in-progress modal tool state (transform, crop, text edit) the same way Escape does; then moves the task to the background. It never discards unsaved work | Must | Scripted back presses in 10 UI states behave as specified; back on the bare canvas leaves the app running with documents intact |
| FR-LC-07 | File › Exit runs the unsaved-changes prompt (desktop behaviour), then finishes and removes the task. Removing the task from Recents is treated like going to the background (autosave flush) | Must | Exit with unsaved changes shows the prompt; swiping from Recents, then relaunching, offers recovery |
| FR-LC-08 | Multi-window: split screen, freeform and desktop windowing, Samsung DeX and resizable ChromeOS windows work at any size from 320 × 480 dp up; the layout adapts to the window size class ([05](05-ux-touch-design.md) §2) | Must | Resizing across the Compact, Medium and Expanded thresholds switches layout without losing state; no clipped controls |
| FR-LC-09 | Single instance (`launchMode="singleTask"`): an "Open with" or share intent that arrives while the app runs opens the file as a new document tab in the running instance | Must | Opening three files from a file manager while the app runs gives three tabs and no second instance |
| FR-LC-10 | Separate OS windows per document ("New Window", `extra_windows`) | Won't | The command is hidden or disabled on Android with an explanation |
| FR-LC-11 | Launcher shortcut "Start in Safe GPU mode" (the equivalent of desktop `--safe-gpu`): starts with the CPU canvas for one launch | Should | Long-pressing the launcher icon shows the shortcut; the app starts with the CPU renderer, and the status UI shows it |

---

## 2. Documents and file I/O (FR-FIO)

| ID | Requirement | Priority | Acceptance criteria |
|---|---|---|---|
| FR-FIO-01 | File › Open uses the system document picker (SAF `ACTION_OPEN_DOCUMENT`) with single and multiple selection. Its filter covers every format the desktop open dialog accepts (the `OPEN_EXTS` list in `apps/photocraft/src/services.rs`) through MIME types, plus an "all files" fallback that identifies files by content | Must | Open PSD, PSB, `.pcraft` (zip form), ORA, KRA, PDN, Affinity, SVG, JPEG, PNG, TIFF, WebP, HEIC, DNG and EXR from local storage, SD card and Downloads; multi-select opens N tabs |
| FR-FIO-02 | File › Import from Photos uses the system Photo Picker (no storage permission) | Should | Picks one or several photos into new documents on Android 10–16 |
| FR-FIO-03 | "Open with" and share target: `ACTION_VIEW` for `image/*` and the PSD, PSB, OpenRaster, Krita, SVG and `.pcraft` MIME types; `ACTION_SEND` and `ACTION_SEND_MULTIPLE` for `image/*`; `ACTION_EDIT` for `image/*` (Gallery and Photos "Edit with") | Must (VIEW, SEND), Should (EDIT) | PhotoCraft appears in "Open with" for a PSD in Files and Google Drive, and in the share sheet of Photos; each path opens the file |
| FR-FIO-04 | Large files are read from file descriptors rather than copied through JNI byte arrays. The existing bounded-read limits apply, and a file larger than the memory budget fails with an actionable message, never a crash | Must | A 1.5 GB PSB opens on an 8 GB device or is refused with "needs about N GB, this device allows M GB"; no out-of-memory crash |
| FR-FIO-05 | File › Save (Ctrl+S) writes back to the document's original location when the app holds a persisted write grant for it; otherwise it falls back to Save As | Must | Open a PSD from Downloads, edit, save: the original file updates (verified by reopening); a read-only provider falls back to Save As |
| FR-FIO-06 | Save As and Save a Copy use `ACTION_CREATE_DOCUMENT` with the suggested name and the MIME type of the chosen format, for every format in desktop Save As (`SAVE_FILTERS`) | Must | Each format saves through the picker and reopens with the expected fidelity |
| FR-FIO-07 | Safe write: the document is encoded completely to a temporary file in app storage first; only then is the target truncated and written. If encoding fails, the target is untouched; if writing fails, the recovery entry remains and the user is told | Must | Fault injection (encoder error, provider write error, process kill mid-write) never leaves an empty or partial target without the recovery entry |
| FR-FIO-08 | Export As and Quick Export go to a location the user chooses (SAF), or straight to the shared gallery (MediaStore `Pictures/PhotoCraft`) when "Save to Gallery" is chosen | Must (SAF), Should (MediaStore) | Exported JPEG and PNG files appear in Photos and Gallery apps with the correct name and date |
| FR-FIO-09 | File › Share… exports with the current Export As settings to a temporary file and opens the Android share sheet through a FileProvider URI with a temporary read grant | Must | Share to Gmail, Drive and a messaging app delivers a valid image; the grant expires afterwards |
| FR-FIO-10 | Folder operations (Export Layers to Files, Batch, Image Processor, Load Files into Stack, any script that takes a folder) use `ACTION_OPEN_DOCUMENT_TREE` with persisted tree grants | Should | Export Layers to Files writes N files into a chosen folder; Batch reads from one tree and writes to another |
| FR-FIO-11 | File › Open Recent lists documents by display name, with a thumbnail where available. Entries are stored as URIs with persisted grants; entries whose grant was revoked or whose file is gone are marked and removable; the list holds 30 entries and grants are released when entries drop out | Must | Recents survive app restart and reboot; a deleted file shows "Not available"; the persisted grant count stays ≤ 30 |
| FR-FIO-12 | Preset and resource files import and export through SAF: brushes (`.abr`), gradients (`.grd`), swatches (`.aco`, `.ase`), patterns (`.pat`), LUTs (`.cube`, `.3dl`, `.look`), plug-ins (`.wasm`), actions (`.atn`), shortcut sets (`.kys`) | Must (abr, grd, aco, ase, pat, LUTs), Should (others) | Each type round-trips (import, use, export) |
| FR-FIO-13 | The directory form of `.pcraft` bundles cannot be opened through SAF; the zip form is fully supported. Choosing a directory bundle explains the limitation and how to convert it on desktop | Must | The message appears; the zip form opens and saves |
| FR-FIO-14 | Tab titles, Save As suggestions and Recents use the provider's display name (`OpenableColumns.DISPLAY_NAME`), never a file-descriptor path or an opaque `content://` string | Must | Files from Drive, Downloads and Photos all show human names |
| FR-FIO-15 | Remote providers (Google Drive, OneDrive, Dropbox through SAF) work for open and save. Slow transfers show progress and can be cancelled; offline errors are actionable | Should | Opening a 100 MB PSD from Drive shows progress; airplane mode gives a clear message and no hang or ANR |

---

## 3. Editing features and platform exceptions (FR-FEAT)

| ID | Requirement | Priority | Acceptance criteria |
|---|---|---|---|
| FR-FEAT-01 | The Android build uses the same engine and command registry as desktop; every command is available unless FR-FEAT-02 lists it | Must | A generated parity report (Android enabled commands vs desktop) shows ≥ 95% enabled; every difference maps to FR-FEAT-02 |
| FR-FEAT-02 | Platform exceptions for v1.0. Each is disabled with an explanatory tooltip or message (not an error), through the existing enable rules: (a) File › Print (until FR-FEAT-08); (b) colour sampling outside the app window; (c) display profile detection (canvas assumes sRGB); (d) importing shortcuts from an installed Photoshop; (e) commands that launch external processes; (f) separate OS windows; (g) desktop window chrome (caption buttons, native menu bar); (h) Touch Bar and scanner (WIA) items | Must | Each item shows its explanation; no command returns a raw OS error |
| FR-FEAT-03 | HEIC/HEIF open is enabled (feature `heif`, as in official desktop builds), and camera raw opens phone DNG files through Camera Raw | Must | HEIC from Samsung and Pixel cameras and DNG from Pixel "RAW" mode open correctly |
| FR-FEAT-04 | For formats the Rust codecs cannot read (AVIF decode today), a flat import through Android's platform decoder (`ImageDecoder`) is offered | Could | An AVIF from a phone gallery opens as a flat 8-bit or 10-bit document with a "decoded by Android" notice |
| FR-FEAT-05 | WebAssembly plug-ins (sandboxed by wasmi) can be installed from SAF into app storage and run as on desktop | Should | The `examples/plugins/invert-rs` plug-in installs and runs |
| FR-FEAT-06 | Actions recording and playback, and Batch with SAF folders, work | Should | A recorded action replays on 10 files through Batch |
| FR-FEAT-07 | Capture from the camera into a new document (`ACTION_IMAGE_CAPTURE` to a FileProvider URI) | Could | The photo opens as a new document at full resolution |
| FR-FEAT-08 | Printing through Android PrintManager, using the PDF that `craft-print` already produces | Could (v1.1) | Prints to "Save as PDF" and to a networked printer |
| FR-FEAT-09 | Wide-gamut (Display P3) and HDR canvas output | Won't | — |

---

## 4. Input (FR-INP)

| ID | Requirement | Priority | Acceptance criteria |
|---|---|---|---|
| FR-INP-01 | One finger drives the active tool exactly like the primary mouse button (press, drag, release) | Must | Every tool in the toolbox works with one finger; strokes match mouse strokes in the automated comparison |
| FR-INP-02 | Navigation gestures: two-finger drag pans; pinch zooms around the centroid; two-finger rotation rotates the view (Rotate View, snapping near 0°, 90°, 180° and 270°; on or off by preference); kinetic panning continues a fling. Gesture and tool are arbitrated: a second finger landing within 150 ms of the first cancels the first finger's stroke without committing it; later than that, the stroke continues and the second finger is ignored | Must | Scripted multi-touch tests cover each case; no stray dabs at the start of a pinch |
| FR-INP-03 | Tap gestures: two-finger tap undoes; three-finger tap redoes; two-finger double tap fits the image on screen. Each can be switched off in Preferences | Must (undo, redo), Should (fit) | Gestures trigger the commands; switched off, they do nothing |
| FR-INP-04 | A long press (about 500 ms, with a small movement tolerance) acts as a secondary click: context menus on the canvas, layers and panels; on a toolbar tool group it opens the flyout of hidden tools | Must | Every context menu reachable by right-click on desktop is reachable by long press |
| FR-INP-05 | **Touch shortcut**: an on-screen modifier control that gives Shift, Alt/Option, Ctrl/Cmd and Space (temporary Hand) as momentary (hold) and latched (tap) states, with a visible state indicator and a position the user can drag | Must | Shift-constrained lines, Alt-click clone source, Ctrl-click layer-mask selection and Space-pan all work by touch alone |
| FR-INP-06 | Stylus: pressure; tilt X and Y; barrel rotation where the hardware reports it; eraser end switches to the Eraser and back (desktop behaviour); hovering shows the brush outline; side buttons can be mapped (default: hold = Alt for eyedropper and clone source; second button = right-click) | Must (pressure, eraser end), Should (tilt, hover, buttons) | Measured on an S Pen and a USI 2.0 pen: the pressure curve matches the Pressure Curve preference; tilt drives Brush Pose; hover preview appears |
| FR-INP-07 | Palm rejection: while a stylus is in range or drawing, finger contacts on the canvas are ignored for tool input; navigation gestures follow the "Finger actions while using a stylus" preference; events cancelled by the system (`FLAG_CANCELED`, `ACTION_CANCEL`) never commit strokes | Must | Resting the palm while drawing leaves no stray marks in 20 strokes |
| FR-INP-08 | Strokes use every reported motion sample, including the historical (batched) samples that 120–240 Hz digitizers deliver between frames | Must | A fast circular stroke at 240 Hz input has no visible polygonal segments; the sample count matches the `MotionEvent` count |
| FR-INP-09 | Physical keyboards: desktop shortcuts with Ctrl as the command key; Escape, Enter, Delete, Tab and the arrow keys; holding modifiers works with touch or stylus (Shift plus a stylus line, for example); the Search/Launcher key on Chromebooks is not intercepted | Must | The shortcut audit suite (desktop list) passes with a Bluetooth keyboard and on a Chromebook |
| FR-INP-10 | Mouse and trackpad: right-click context menus; scroll-wheel zoom and pan per the desktop preferences; hover feedback; trackpad pinch | Must | Same results as Windows desktop in the mouse test suite |
| FR-INP-11 | Text entry: focusing any text field (Type tool on canvas, numeric fields, layer names, search, command palette) shows the soft keyboard; IME composition works for Chinese, Japanese and Korean; closing the keyboard neither commits nor cancels unexpectedly. Where on-canvas IME is unreliable, a native text-entry sheet edits the text and returns it to the field or type layer | Must | Typing English, Japanese (Gboard), Chinese pinyin, Korean and emoji into a type layer and into numeric fields works on 3 keyboard apps (Gboard, Samsung Keyboard, SwiftKey) |
| FR-INP-12 | Haptic feedback for discrete events (gesture undo, snapping, modifier latch) | Could | Haptics follow the system haptic setting |

---

## 5. User interface adaptation (FR-UI)

| ID | Requirement | Priority | Acceptance criteria |
|---|---|---|---|
| FR-UI-01 | The UI scale follows Android display density. New preference Interface › UI Density: **Compact** (desktop metrics) or **Touch** (interactive targets ≥ 48 × 48 dp). Default: Touch on touch-only devices, Compact when a mouse or trackpad is present (automatic, with user override) | Must | Every control in the top bar, toolbox, options bar and Layers panel measures ≥ 48 dp in Touch density |
| FR-UI-02 | Safe areas: interactive controls never sit under the status bar, navigation bar, display cutout or soft keyboard; the canvas may extend edge to edge | Must | Verified with gesture and 3-button navigation, cutouts, and the keyboard open |
| FR-UI-03 | Workspaces: new built-in **Touch** (tablet) and **Compact** (phone) presets alongside Essentials, Photography, Painting, Pixel Art, Graphic and Web, and Motion (`crates/ui-egui/src/workspace_ui.rs`), chosen automatically by window size class with a user override | Must (Touch), Should (Compact) | The workspace switches as the window crosses size-class thresholds unless the user pinned one |
| FR-UI-04 | Phone layout (window narrower than 600 dp): canvas first; a top bar with document title, undo, redo and Menu (the full menu tree as a drill-down list); tools in a drawer; Layers, Properties, Adjustments and Color as bottom sheets; a condensed options bar ([05](05-ux-touch-design.md) §3.3) | Should in v1.0, Must in v1.1 (per D-02) | Persona P3 tasks ([05](05-ux-touch-design.md) §12) complete on a 6.1" phone |
| FR-UI-05 | Nothing depends on hover alone: every tooltip, rollover highlight or hover preview is also reachable by long press or a visible affordance | Must | A hover audit lists each hover-dependent feature and its touch alternative; all implemented |
| FR-UI-06 | Dialogs fit the window: content scrolls; OK and Cancel stay visible; dialogs open full-screen in the Compact size class | Must | All dialogs (as on the desktop dialog inventory) pass at 360 × 640 dp and 600 × 960 dp |
| FR-UI-07 | The theme follows the system's light or dark mode by default; Pro, Studio and Classic stay selectable | Must | Switching the system theme updates the app live |
| FR-UI-08 | A start screen when no document is open: New (presets, including phone and tablet screen sizes), Open (SAF), Photos (Photo Picker), Recents with thumbnails, Help | Should | First launch shows the start screen; each entry works |
| FR-UI-09 | Landscape is the primary orientation (owner decision, 2026-10-14): phones and tablets run in landscape (`sensorLandscape`, either way up); portrait stays a stretch goal for foldables and large tablets | Must | Every layout and dialog renders and fits in landscape on phone and tablet; the app turns with the device between the two landscape directions without restarting |
| FR-UI-10 | The system font scale and bold-text settings affect UI text within clamps that keep layouts intact (0.85–1.3× of the base) | Should | At font scale 1.3 nothing overlaps; at 2.0 text clamps to 1.3× |

---

## 6. Settings, presets and persistence (FR-SET)

| ID | Requirement | Priority | Acceptance criteria |
|---|---|---|---|
| FR-SET-01 | Preferences persist in app-private storage, using the same JSON schema as desktop; Preferences can be exported and imported through SAF | Must | A preferences file exported on Android imports on desktop and the other way round |
| FR-SET-02 | Panel layout and workspace state persist (eframe persistence pointed at app storage) | Must | A rearranged panel survives a restart |
| FR-SET-03 | Brush, gradient, pattern and swatch libraries persist (the engine preset store in app storage) | Must | Imported brushes remain after a restart |
| FR-SET-04 | Settings can reset preferences and clear caches | Should | Reset restores the defaults; clearing caches frees the reported space |
| FR-SET-05 | Android Auto Backup and device-to-device transfer include preferences and presets and exclude the recovery store, caches and logs | Should | A `bmgr` backup and restore test restores preferences; the recovery files are not in the backup |

---

## 7. Fonts and text (FR-TXT)

| ID | Requirement | Priority | Acceptance criteria |
|---|---|---|---|
| FR-TXT-01 | System fonts are enumerated through Android's `SystemFonts` API (API 29+) and registered with the type engine. CJK fallback uses the system's Noto CJK fonts (the equivalent of `crates/text/src/cjk.rs` on other platforms) | Must | The font menu lists the system fonts; Japanese, Chinese and Korean text renders without tofu |
| FR-TXT-02 | Users can import TTF, OTF and TTC fonts through SAF into app storage; imported fonts appear in the font menus and can be removed | Should | An imported font renders in a type layer and survives a restart |
| FR-TXT-03 | The bundled craft-fonts (Japanese) ship as decided in D-09 | Should | Per D-09 |
| FR-TXT-04 | Opening a PSD reports missing-font substitutions as on desktop | Must | A PSD with a missing font shows the substitution warning |

---

## 8. Clipboard (FR-CLP)

| ID | Requirement | Priority | Acceptance criteria |
|---|---|---|---|
| FR-CLP-01 | Copy and paste of layers and pixels inside the app always works | Must | Same as desktop |
| FR-CLP-02 | Copying an image places a PNG on the system clipboard (through a FileProvider URI), and pasting accepts images copied in other apps; text fields use the system clipboard for text | Should | Copy from PhotoCraft and paste into Google Docs; copy an image in Chrome and paste into PhotoCraft |

---

## 9. Localization (FR-L10N)

| ID | Requirement | Priority | Acceptance criteria |
|---|---|---|---|
| FR-L10N-01 | The UI language follows the system locale; every catalog shipped on desktop is available | Must | Switching the system language to each shipped language changes the UI |
| FR-L10N-02 | Per-app language selection in Android Settings (Android 13+, declared through `localeConfig`), mirrored by the in-app language preference | Should | Selecting a language in Android Settings › Apps › PhotoCraft › Language applies it |
| FR-L10N-03 | Android-only strings (start screen, pickers, share, recovery, Touch shortcut, onboarding) are translated into every shipped language | Should | `cargo xtask i18n-coverage` (or an Android equivalent) reports 100% for the new keys |
| FR-L10N-04 | Right-to-left layout for Arabic, at the level of the desktop's RTL shell | Could | Matches desktop status |

---

## 10. Help, about and diagnostics (FR-HLP)

| ID | Requirement | Priority | Acceptance criteria |
|---|---|---|---|
| FR-HLP-01 | About shows the version, commit and build date (`crates/engine/src/build_info.rs`) and the distribution channel (GitHub, Play or F-Droid) | Must | Matches the release metadata |
| FR-HLP-02 | A Licenses screen shows NOTICE, ATTRIBUTION, the OFL licence of each bundled font, and the AndroidX notices | Must | The legal reviewer confirms completeness |
| FR-HLP-03 | Help links open in the default browser through an intent; the app embeds no web view | Must | Every link works; no WebView class in the APK |
| FR-HLP-04 | Help › Export Diagnostics creates a zip with the app's logs, recent exit reasons (`ApplicationExitInfo`) and device and GPU information, and offers it through the share sheet; nothing is sent automatically | Should | The zip contains the listed items and no document content |
| FR-HLP-05 | Help › Report a Bug opens the GitHub issue form (or Discord) with device information pre-filled for the user to review | Could | The link opens with the fields filled |

---

## 11. Updates (FR-UPD)

| ID | Requirement | Priority | Acceptance criteria |
|---|---|---|---|
| FR-UPD-01 | Google Play updates the Play edition; no in-app update prompt (that would add a Play services dependency) | Must (Play updates), Won't (in-app prompt) | — |
| FR-UPD-02 | GitHub and website APKs update in place over earlier versions (same signing certificate; increasing versionCode, [06](06-build-release-distribution.md) §3). There is no in-app update checker (no network permission); the release notes recommend checking the releases page or using a third-party updater | Must | Installing v1.0.1 over v1.0.0 keeps data; the certificate fingerprint matches the published one |

---

## 12. QA and automation hooks (FR-QA)

| ID | Requirement | Priority | Acceptance criteria |
|---|---|---|---|
| FR-QA-01 | A **QA build type** includes the authenticated loopback control server (the existing protocol, `docs/control-protocol.md`) for automated UI tests and store screenshots through `adb forward`. It is never present in release builds | Must | CI asserts that the release manifest has no `INTERNET` permission and that the release `.so` lacks the control-server feature; the QA build answers `ui.screenshot` |
| FR-QA-02 | Debug overlays show the GPU backend and adapter, frame time and memory budget (existing status UI) | Should | Visible in QA builds and through a hidden preference in release |
