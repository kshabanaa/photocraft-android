# 03: Non-functional requirements

| | |
|---|---|
| **Version** | 1.0 draft, 2026-10-13 |
| **Scope** | Quality attributes, platform and policy compliance for the Android edition |
| **Verification** | Each requirement maps to tests in [07](07-test-qa-plan.md) and to traceability in [10](10-traceability-matrix.md) |

---

## 1. Compatibility (NFR-COMP)

| ID | Requirement | Priority | Verification |
|---|---|---|---|
| NFR-COMP-01 | `minSdk` 29 (Android 10), `targetSdk` 36 (Android 16), `compileSdk` 36. Raise `targetSdk` every year before Google Play's deadline (usually 31 August) | Must | Gradle config; Play Console warnings |
| NFR-COMP-02 | ABIs: `arm64-v8a` and `x86_64`. Every 64-bit `.so` is 16 KB page-aligned (ELF `LOAD` segments aligned to `0x4000`), and APKs are zip-aligned for 16 KB | Must | `llvm-readelf -l` on each `.so`; `zipalign -c -P 16 -v 4`; boot test on a 16 KB-page emulator image |
| NFR-COMP-03 | Devices: phones, tablets, foldables, Chromebooks (ARCVM) and Samsung DeX. Minimum 4 GB RAM supported (3 GB starts with reduced limits and a warning, untested). GPU: Vulkan 1.1 preferred, OpenGL ES 3.0+ fallback, CPU canvas as a last resort | Must | Device matrix ([07](07-test-qa-plan.md) §4) |
| NFR-COMP-04 | No dependency on Google Play services (GMS); the app runs on AOSP-based and de-Googled devices | Must | Run on an emulator image without GMS; dependency audit |
| NFR-COMP-05 | Large-screen rules (API 36): no orientation, aspect-ratio or resizability restrictions; correct behaviour in split screen, freeform and desktop windowing | Must | Large-screen test cases; Play pre-launch report |
| NFR-COMP-06 | Behaviour changes when targeting API 35 and 36 are handled: edge-to-edge enforcement (insets), predictive back (`KEYCODE_BACK` no longer delivered), changes to foreground-service types | Must | Targeted tests per change |

---

## 2. Performance (NFR-PERF)

### 2.1 Reference devices

- **RT (reference tablet):** a current flagship tablet, Snapdragon 8 Gen 2-class (Adreno 740), 8 GB RAM. Example:
  Galaxy Tab S9.
- **BD (baseline device):** an entry-level device, Helio G99-class (Mali-G57), 4 GB RAM. Example: Galaxy Tab A9.

The exact models are chosen in Phase 0 (Q-05).

### 2.2 Budgets

| ID | Scenario | RT target | BD target | How measured |
|---|---|---|---|---|
| NFR-PERF-01 | Cold start to interactive UI, no document | ≤ 2.5 s P50 | ≤ 5 s P50 | `adb shell am start -W` (TotalTime) plus a first-interactive-frame marker; Perfetto |
| NFR-PERF-02 | Warm resume to first frame | ≤ 300 ms | ≤ 800 ms | Perfetto |
| NFR-PERF-03 | Open a 24 MP JPEG | ≤ 1.5 s | ≤ 4 s | QA control channel timing |
| NFR-PERF-04 | Open a 50-layer 4K PSD | ≤ 4 s | ≤ 12 s, or refused cleanly if over the memory budget | Same |
| NFR-PERF-05 | Brush latency, touch or stylus to photon (30 px soft round, 12 MP document) | ≤ 50 ms P95 | ≤ 90 ms P95 | In-app instrumentation (`MotionEvent` time to present time); spot-checked with a high-speed camera |
| NFR-PERF-06 | Pan, zoom and rotate frame time (12 MP, 20 layers) | ≤ 16.7 ms P95 (60 fps) | ≤ 33 ms P95 (30 fps) | Frame-time overlay; Perfetto |
| NFR-PERF-07 | Gaussian Blur, radius 50, on a 12 MP layer | ≤ 1.5 s | ≤ 5 s | Desktop `cargo xtask perf` scenario, run on the device |
| NFR-PERF-08 | Save a 50-layer PSD to local storage | ≤ 5 s | ≤ 15 s | Control-channel timing |
| NFR-PERF-09 | Autosave flush when backgrounded (12 MP, 20 layers, incremental) | ≤ 2 s | ≤ 4 s | Timestamped log events |
| NFR-PERF-10 | Idle: no input, no jobs running | No repaint after 1 s; CPU < 1% | Same | `dumpsys gfxinfo`; `top` |
| NFR-PERF-11 | Peak memory, 24 MP with 30 layers | ≤ 2.5 GB | Within the BD budget (about 1.6 GB) or a clean refusal | `dumpsys meminfo`; Perfetto memory track |

**Notes**

- Upstream meets only 3 of its 25 desktop performance budgets today (`docs/scorecard.md`). The mobile budgets above
  are deliberately looser. Re-baseline them after the Phase 0 measurements and record the values in upstream
  `perf/budgets.toml`, under an Android section, once a device runner exists.
- Long operations already run as background jobs with progress (`jobs_ui.rs`). On Android they must also never block
  input dispatch: no ANR from native work.

---

## 3. Stability (NFR-STAB)

| ID | Requirement | Priority | Verification |
|---|---|---|---|
| NFR-STAB-01 | User-perceived crash rate < 1.0% overall (Play bad-behaviour threshold 1.09%) and < 8% on any single device model | Must | Play vitals; beta `ApplicationExitInfo` statistics |
| NFR-STAB-02 | User-perceived ANR rate < 0.3% overall (threshold 0.47%) | Must | Play vitals |
| NFR-STAB-03 | Never-crash rules apply to all Android Rust code: `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]`; panics caught around command dispatch, import and export (desktop `crash_guard` behaviour); `panic = "unwind"` | Must | Clippy in CI; `panic_hunt` test suite also run on the device |
| NFR-STAB-04 | GPU resilience: if the GPU backend crashes or hangs during start-up, the next launch falls back automatically (Vulkan, then GLES, then the CPU canvas) using the desktop crash-marker technique; 100% of matrix devices reach an interactive UI | Must | Fault injection through a QA preference; device matrix |
| NFR-STAB-05 | Memory pressure: caches shrink on `onTrimMemory`; documents larger than the budget are refused with a message; opening never ends in an out-of-memory crash | Must | `adb shell am send-trim-memory`; large-file tests |
| NFR-STAB-06 | JNI hygiene: every call checks and clears Java exceptions and turns them into `Err`; local references are bounded by frames; no global-reference leaks | Must | Bridge unit tests; StrictMode and JNI checks (`-Xcheck:jni`) in QA builds |
| NFR-STAB-07 | No main-thread disk or network access in the Kotlin layer, except during start-up, which is measured | Should | StrictMode in debug and QA builds |

---

## 4. Data integrity (NFR-DATA)

| ID | Requirement | Priority | Verification |
|---|---|---|---|
| NFR-DATA-01 | No work is lost through backgrounding plus kill, rotation, fold, multi-window resize, low-memory kill, a crash during editing, or a crash during save (the recovery entry exists) | Must | Lifecycle torture suite ([07](07-test-qa-plan.md) §3.2) |
| NFR-DATA-02 | Autosave runs every ≤ 60 s while editing (desktop preference) and immediately when the app is backgrounded | Must | Logs; recovery tests |
| NFR-DATA-03 | Encoding happens before the target file is touched, so a failed save never leaves an empty or truncated target that has no recovery entry | Must | Fault injection |
| NFR-DATA-04 | Output parity: flat formats are byte-identical and PSD/PSB/`.pcraft` renders are identical to desktop output for the same document and settings | Must | Cross-platform golden tests (Android device or emulator against Linux CI) |

---

## 5. Size (NFR-SIZE)

| ID | Requirement | Priority | Verification |
|---|---|---|---|
| NFR-SIZE-01 | Per-ABI APK download ≤ 60 MB; universal APK ≤ 120 MB; the AAB base module within Play's download limit (200 MB compressed at the time of writing; confirm) | Must | CI size report; APK Analyzer |
| NFR-SIZE-02 | Installed footprint ≤ 250 MB, excluding user data and caches | Should | Settings › Storage on a device |
| NFR-SIZE-03 | The size of each artifact is recorded per release, and growth over 5% is flagged in the release summary | Should | CI step |

Native libraries are stored uncompressed and page-aligned in the APK (modern AGP default), so the `.so` is
memory-mapped rather than extracted. That raises the APK size on disk but lowers the installed footprint.

---

## 6. Security and privacy (NFR-SEC)

| ID | Requirement | Priority | Verification |
|---|---|---|---|
| NFR-SEC-01 | The release manifest requests no dangerous permissions and no `INTERNET`. QA builds add `INTERNET` only for the loopback control server | Must | `aapt2 dump permissions` in CI fails the release on any unexpected permission |
| NFR-SEC-02 | Exported components: only the launcher activity, with its intent filters. The FileProvider is not exported (`grantUriPermissions` only). There are no exported services or receivers | Must | `aapt2 dump xmltree`; manifest lint |
| NFR-SEC-03 | Incoming files (intents, picker results, shares) are untrusted: they go through the existing hardened and fuzzed parsers with the existing size caps; MIME types and file names from other apps are not trusted (content sniffing; names sanitized before use in any path) | Must | Malformed-file corpus through intents; path-traversal name tests |
| NFR-SEC-04 | The FileProvider exposes only `cache/share/` and `cache/clipboard/`; grants are temporary and per URI | Must | Attempted access to other paths fails |
| NFR-SEC-05 | Backup rules (`dataExtractionRules`, `fullBackupContent`) exclude the recovery store, caches and logs | Must | Backup inspection |
| NFR-SEC-06 | `unsafe` code is limited to one JNI module with `SAFETY:` comments and tests; the rest of the workspace keeps `unsafe_code = "forbid"` | Must | `grep` plus a lint configuration check in CI; review |
| NFR-SEC-07 | Supply chain: Rust dependencies locked by `Cargo.lock` (CI uses `--locked`); Gradle dependency verification metadata committed; GitHub Actions pinned by commit SHA (following upstream policy on shared workflows); a pinned NDK and build-tools version | Must | CI configuration review |
| NFR-SEC-08 | Signing keys never enter the repository; secrets live in the `release` environment, restricted to the `release` branch; key backups are kept offline in two places; Play App Signing is used ([06](06-build-release-distribution.md) §4) | Must | Release checklist |
| NFR-SEC-09 | Data safety: "No data collected; no data shared". The privacy policy states the same, and the in-app About links to it | Must | Play Console review |
| NFR-SEC-10 | Release builds are not debuggable, contain no WebView, and keep R8 rules for the JNI-called Kotlin classes | Must | `aapt2 dump badging`; APK inspection |

---

## 7. Accessibility (NFR-A11Y)

| ID | Requirement | Priority | Verification |
|---|---|---|---|
| NFR-A11Y-01 | Touch density: interactive targets ≥ 48 × 48 dp | Must | Automated layout probe through the control channel (widget rectangles) |
| NFR-A11Y-02 | Text contrast ≥ 4.5:1 in the default themes (audit of `theme::Tokens`) | Should | Contrast audit |
| NFR-A11Y-03 | Respects the system font scale within clamps (FR-UI-10) | Should | Manual test |
| NFR-A11Y-04 | Full keyboard operation with a physical keyboard (focus traversal, menus, dialogs) | Should | Keyboard-only test pass |
| NFR-A11Y-05 | Screen-reader support (TalkBack) is a **known gap in v1.0**: the current dependency tree (`accesskit_winit` 0.32) contains no Android AccessKit adapter. The gap is disclosed in the store listing and README, and support is tracked as an upstream contribution to egui/AccessKit | Should (v1.x) | Roadmap item; disclosure present |

---

## 8. Localization (NFR-L10N)

| ID | Requirement | Priority | Verification |
|---|---|---|---|
| NFR-L10N-01 | All UI languages shipped on desktop are available (20 at the baseline) | Must | Screenshot pass per language |
| NFR-L10N-02 | Android-only strings use the same catalog pipeline as the rest of the UI (no separate Android `strings.xml` for in-app UI; Android resources only for the manifest label and launcher shortcuts) | Should | Catalog coverage report |
| NFR-L10N-03 | Pseudo-locale tests for text expansion: no clipped controls at +40% text length | Should | QA build with a pseudo-locale |

---

## 9. Maintainability (NFR-MAINT)

| ID | Requirement | Priority | Verification |
|---|---|---|---|
| NFR-MAINT-01 | **Never break Android**: `cargo check` for `aarch64-linux-android` (the Android app plus L0–L6) runs on every pull request and blocks merging, mirroring upstream's "Never break wasm" rule | Must | CI job |
| NFR-MAINT-02 | Layering holds: new crates are registered in `xtask/src/layers.rs`; nothing below `ui-egui` depends on Android crates; Android code sits behind `cfg(target_os = "android")`, and the app crate compiles to an empty library on desktop hosts (as `photocraft-web` does), so `cargo test --workspace` keeps working | Must | `cargo xtask layers`; workspace build on Linux, Windows and macOS |
| NFR-MAINT-03 | The Kotlin layer stays ≤ about 1,500 lines with no business logic; every Kotlin entry point is mirrored by a typed Rust wrapper | Should | Line count in CI; review |
| NFR-MAINT-04 | The version has a single source: the workspace version produces `versionName` and `versionCode` automatically | Must | Build script tests |
| NFR-MAINT-05 | Documentation: `docs/android.md` (build, run, debug, release); an Android section in `AGENTS.md` (rules and gates); a `docs/releasing.md` update | Must | Docs review at G1 |
| NFR-MAINT-06 | Reproducible environment: the NDK, SDK, build-tools, cargo-ndk and Gradle wrapper versions are pinned and documented | Must | CI configuration |

---

## 10. Observability (NFR-OBS)

| ID | Requirement | Priority | Verification |
|---|---|---|---|
| NFR-OBS-01 | Logs go to logcat (tag `PhotoCraft`) and to a rotating file log in app storage (≤ 5 MB; desktop `logging.rs` rotation rules) | Must | Log inspection |
| NFR-OBS-02 | At start-up, the reasons previous sessions ended (`ApplicationExitInfo`: crash, native crash tombstone, ANR, low memory) are logged and fed into FR-LC-05 | Must | Simulated exits |
| NFR-OBS-03 | Native debug symbols are uploaded to Play with each AAB and archived for each GitHub release, so tombstones can be symbolicated | Must | Symbols present; a test symbolication succeeds |
| NFR-OBS-04 | No remote telemetry by default. Play vitals is the only automatic aggregate (collected by the OS and Play for Play installs) | Must | Network audit: no sockets in release |

---

## 11. Energy and thermal (NFR-EN)

| ID | Requirement | Priority | Verification |
|---|---|---|---|
| NFR-EN-01 | Reactive rendering: the UI repaints only on input, animation or job progress | Must | NFR-PERF-10 |
| NFR-EN-02 | In the background, no work runs after the autosave flush except a job the user started. Long batch jobs may use a foreground service of type `mediaProcessing` (Android 15+) or `dataSync` with a visible notification | Should (no background work), Could (foreground service) | Battery historian; `dumpsys activity services` |
| NFR-EN-03 | Under thermal pressure (`PowerManager` thermal status SEVERE or worse), heavy filters reduce worker threads and the UI shows a hint | Could | Thermal emulation through `adb shell cmd thermalservice` |

---

## 12. Policy compliance checklist (NFR-COMPL)

| ID | Requirement | Source | How it is met | Checked |
|---|---|---|---|---|
| NFR-COMPL-01 | Target API level 36 for new apps and updates from 2026-08-31 | Google Play | `targetSdk = 36` (NFR-COMP-01) | Every release |
| NFR-COMPL-02 | 16 KB page-size support for apps targeting Android 15+ | Google Play | Linker flag plus checks (NFR-COMP-02) | Every release |
| NFR-COMPL-03 | 64-bit native code | Google Play | `arm64-v8a` and `x86_64` only | Every release |
| NFR-COMPL-04 | AAB for Play publishing | Google Play | `bundleRelease` in CI | Every release |
| NFR-COMPL-05 | Data safety form and privacy policy | Google Play | "No data collected"; policy hosted on getartcraft.com | Before G3, then on change |
| NFR-COMPL-06 | Store listing metadata and intellectual property: no other company's trademarks in the title or icon; no misleading claims | Google Play | Listing rules ([06](06-build-release-distribution.md) §7) | Each listing change |
| NFR-COMPL-07 | Content rating (IARC questionnaire) and target audience (not directed at children) | Google Play | Questionnaire in Play Console | Before G3 |
| NFR-COMPL-08 | Developer verification for distribution outside Play (Brazil, Indonesia, Singapore and Thailand now; global in 2027) | Android | Register the developer, package name and signing certificate ([06](06-build-release-distribution.md) §6.1) | Before the first public APK |
| NFR-COMPL-09 | Licence compliance (MIT/Apache NOTICE, ATTRIBUTION, OFL fonts, AndroidX) | Licences | Licences screen (FR-HLP-02) | Every release |
| NFR-COMPL-10 | Brand licence (ArtCraft marks in official builds only) | `docs/brand/LICENSE-brand.txt` | D-01; a build flag strips the marks for non-official builds | Every release |
