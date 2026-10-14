# 07: Test and QA plan

| | |
|---|---|
| **Version** | 1.0 draft, 2026-10-13 |
| **Covers** | Every FR and NFR in [02](02-functional-requirements.md) and [03](03-non-functional-requirements.md); gates G0–G3 |
| **Principle** | Reuse upstream's test assets (1,700+ tests, the real-file corpora, `panic_hunt`, perf scenarios, control-channel automation) and add Android-specific layers on top |
| **Who tests** | AI delivery (D-14). "QA" and "engineers" below mean the QA agent and implementer agents. Physical-device checks, feel and usability judgements are the owner's (and beta testers') tasks ([11](11-ai-delivery-plan.md) §2, H-03, H-11) |

---

## 1. Objectives

1. Prove the Android build behaves like desktop: same commands and same output (BR-03, BR-04).
2. Prove the platform shell never loses work across Android lifecycle events (BR-06).
3. Prove the app starts and stays stable across GPU families, RAM tiers and form factors (BR-08).
4. Prove touch and stylus users can complete real workflows (BR-05).
5. Prove release artifacts meet store and security policy (BR-02, BR-07).

---

## 2. Test levels

| Level | What | Where it runs | Owner | Frequency |
|---|---|---|---|---|
| L1: upstream unit and integration tests | The existing workspace tests (engine, codecs, PSD, compose, …) | Linux, Windows and macOS CI (unchanged) | Engineers | Every PR |
| L2: Android pure-logic tests | Host-run tests of the Android crate's mapping code: versionCode, MIME↔extension, display-name sanitizing, tilt conversion, gesture arbitration, recents capping | Host CI | Engineers | Every PR |
| L3: cross-compile gate | `cargo xtask android check` for both ABIs | CI | Engineers | Every PR |
| L4: on-device Rust tests | Selected test binaries built with cargo-ndk, pushed and run over adb: `photocraft-gpu` (GPU vs CPU compositor oracle), `photocraft-compose`, `photocraft-codecs`, `photocraft-psd`, the engine's `panic_hunt` | Physical devices (Tier 1) and the emulator | QA and engineers | Nightly, and before each gate |
| L5: Kotlin instrumented tests | AndroidX Test / UI Automator: SAF flows against a test `DocumentsProvider`, intents, FileProvider grants, back dispatch, insets, the `TMPDIR` environment, exit-reason reading | Emulator in CI; devices before gates | Engineers | Every PR touching `packaging/android` |
| L6: end-to-end through the control channel | QA build plus `adb forward`: scripted workflows that reuse upstream's agent task scenarios (`crates/automation/tests/agent_tasks.rs`), screenshots compared with references | Emulator (CI smoke); device lab (full) | QA | CI smoke on every PR; full suite nightly and per release |
| L7: performance | NFR-PERF scenarios through the control channel, plus Perfetto traces | Reference devices RT and BD | QA | Weekly from Phase 3; every release candidate |
| L8: manual exploratory and usability | Touch and stylus feel, layouts, IME, accessibility | Device lab; usability sessions ([05](05-ux-touch-design.md) §16) | QA and UX | Each phase |
| L9: beta | Closed beta (Discord, Play closed track) | Users' devices | Community manager and QA | G2 to G3 |

---

## 3. Specific test suites

### 3.1 Parity report (FR-FEAT-01)

A generator lists every engine command with `enabled` on Android vs desktop, from the command registry. Each
difference must map to FR-FEAT-02. Runs in CI on the QA APK.

### 3.2 Lifecycle torture suite (NFR-DATA-01)

Scripted with adb and the control channel. Every case asserts that the document reopens with its last edit and that
no stroke or step is lost.

| Case | Script |
|---|---|
| Background then kill | Edit → `input keyevent KEYCODE_HOME` → `am kill <pkg>` → launch → recovery |
| Rotation storm | 20 × `settings put system user_rotation` during a live stroke and in open dialogs |
| Fold and resize | Foldable emulator postures; `wm size` changes; split screen through `am start --windowingMode` |
| Low memory | `am send-trim-memory <pkg> RUNNING_CRITICAL`, then `COMPLETE`; the background LMK simulation |
| Kill during save | Kill while the save job runs (several timings); verify the target file and the recovery entry |
| Developer option "Don't keep activities" | Navigate away and back during each major flow |
| Revoked grant | Revoke the persisted URI permission, then Save → falls back to Save As with a message |
| Native crash | QA-only command that aborts on purpose → next start shows the reason and recovery |

### 3.3 Cross-platform golden outputs (BR-04, NFR-DATA-04)

A fixed set of documents from the corpus plus synthetic generators, run with a fixed command script on Linux CI and on
the Android emulator and devices:

- Flat exports (PNG, TIFF, JPEG at fixed quality): byte-identical.
- PSD, PSB and `.pcraft` saves: identical renders through the CPU compositor oracle.

Differences are bugs, except documented floating-point tolerances in GPU paths.

### 3.4 Storage provider matrix (FR-FIO-*)

| Provider | Open | Save in place | Save As | Folder (tree) | Notes |
|---|---|---|---|---|---|
| Local storage (Downloads, Documents) | ✓ | ✓ | ✓ | ✓ | Baseline |
| SD card / USB-OTG | ✓ | ✓ | ✓ | ✓ | Removable media ejected mid-save |
| Google Drive | ✓ | ✓ | ✓ | — | Slow network, offline, large files |
| Photos / Gallery (Photo Picker, `ACTION_EDIT`) | ✓ | (EDIT result) | — | — | |
| A third-party cloud provider (e.g. OneDrive or Dropbox) | ✓ | ✓ | ✓ | — | Provider-specific `"wt"` behaviour |
| Email attachment (Gmail "Open with") | ✓ | read-only → Save As | ✓ | — | |

### 3.5 Input suite (FR-INP-*)

- **Multi-touch scripts:** two-finger pan, pinch, rotate and taps, and the arbitration timing (a second finger before
  and after 150 ms). Injected through instrumentation, plus manual runs.
- **Stylus:** pressure ramps (S Pen and USI 2.0), tilt sweeps, eraser flip, hover, button mapping, palm-rest
  rejection, a 240 Hz sample-count check.
- **Keyboards:** the desktop shortcut audit list on a Bluetooth keyboard and on a Chromebook.
- **IME matrix:**

  | Keyboard app | Languages |
  |---|---|
  | Gboard | English, Japanese, Chinese pinyin, Korean, Arabic, emoji |
  | Samsung Keyboard | English, Korean |
  | SwiftKey | English |

  Each combination is tested in type layers, numeric fields, layer rename and the command palette.

### 3.6 GPU suite (NFR-STAB-04)

- Backend-plan unit tests: marker present or absent, deny-list hits.
- Fault injection (QA preference "fail device creation on backend X") checks that each fallback step is reached.
- The L4 GPU oracle runs on Adreno, Mali, Xclipse and PowerVR devices.
- Long-session test: 30 minutes of painting and filters with no device-lost errors and no memory growth.

### 3.7 Security and policy suite (NFR-SEC-*, NFR-COMPL-*)

- **Manifest audit:** permissions, exported components, `debuggable`, `allowBackup` rules.
- **Intent fuzzing:** malformed and oversized files through `VIEW` and `SEND`; hostile display names (path
  separators, `..`, very long strings, control characters).
- **FileProvider:** access outside `share/` and `clipboard/` is refused.
- **No network in release:** the release APK has no `INTERNET` permission, and socket creation fails (checked in a
  test build of the release configuration).
- **Policy:** `targetSdk` 36, 16 KB alignment, 64-bit only, AAB built, symbols present.

### 3.8 Localization and accessibility

- Screenshot pass in every shipped language (control channel), plus a pseudo-locale with +40% text length: no clipped
  controls.
- Touch-target probe: widget rectangles from the control channel measured against 48 dp in Touch density.
- Contrast audit of the theme tokens; font scale 0.85–1.3 layout check; keyboard-only navigation pass.

---

## 4. Device matrix

The models are examples. Choose by availability, but keep the **GPU family**, **RAM tier** and **form factor**
coverage.

| Tier | Device class (example) | SoC / GPU family | RAM | Form factor | Why |
|---|---|---|---|---|---|
| 1 | Flagship tablet with a stylus (Galaxy Tab S9-class) | Snapdragon / **Adreno 7xx** | 8–12 GB | Tablet, S Pen, DeX | Reference tablet (RT); persona P1 |
| 1 | Flagship tablet, MediaTek (Galaxy Tab S10-class) | Dimensity / **Mali (Immortalis)** | 12 GB | Tablet, S Pen | Second GPU family |
| 1 | Pixel phone (Pixel 8 or 9) | Tensor / **Mali** | 8–12 GB | Phone; 16 KB-page developer option | Google reference phone; compact layout |
| 1 | Intel Chromebook | x86_64 / Intel | 8 GB | Laptop or convertible, keyboard, touchpad | `x86_64` ABI; persona P2 |
| 1 | Android emulator, API 29 and API 36 (x86_64), plus a 16 KB-page system image | SwiftShader / host GPU | 4–8 GB | Configurable | CI; policy checks |
| 2 | Entry tablet (Galaxy Tab A9-class) | Helio G99 / **Mali-G57** | **4 GB** | Tablet | Baseline device (BD); memory limits |
| 2 | Mid-range Samsung phone (A55-class) | Exynos / **Xclipse (RDNA)** | 8 GB | Phone | Xclipse drivers |
| 2 | Low-end phone with **PowerVR** (Helio G36/G37-class) | PowerVR GE8320 | 3–4 GB | Phone | GLES fallback, CPU canvas path |
| 2 | Foldable (Galaxy Z Fold- or Pixel Fold-class) | Adreno or Mali | 12 GB | Foldable | Posture and size-class changes |
| 2 | ARM Chromebook with a USI stylus | MediaTek / Mali | 4–8 GB | Detachable | USI pen; ARM ChromeOS |

Optional burst testing on a device farm covers further Adreno and Mali generations before G3.

---

## 5. Test areas checklist (per release candidate)

1. **Install and update:** fresh install; update over the previous release (data kept); downgrade blocked; switching
   between Play and GitHub (with the shared certificate, D-08); uninstall and reinstall behaviour.
2. **Launch:** splash; first-run cards; start screen; Safe GPU mode shortcut.
3. **Files:** the provider matrix (§3.4) for PSD, `.pcraft`, JPEG, HEIC and DNG; Recents; Share; Export to Gallery.
4. **Core workflows by touch and stylus** (tablet): retouch, composite, cut-out, design, paint, PSD exchange.
5. **Phone tasks** ([05](05-ux-touch-design.md) §12.3).
6. **Lifecycle torture** (§3.2).
7. **Performance** (NFR-PERF on RT and BD).
8. **Localization and accessibility** (§3.8).
9. **Smoke checklist (10 minutes, every release):**
   - Install.
   - Open a PSD from Downloads.
   - Paint 3 strokes with the stylus, pinch and rotate.
   - Add an adjustment layer.
   - Save in place.
   - Export a JPEG to the gallery.
   - Share.
   - Background, kill and relaunch: recovery works or there is nothing to recover.
   - About shows the right version.

---

## 6. Gate entry and exit criteria

| Gate | Entry | Exit |
|---|---|---|
| **G0: feasibility** | Spike done ([04](04-technical-architecture.md) §18) | Workspace compiles for both ABIs; PhotoCraft renders and opens a file on the emulator and at least one physical device (the owner's); a second GPU family planned before G2 (second device, beta tester or device farm); sizes, start-up and memory measured; blockers listed; D-01 to D-06 and D-10 decided |
| **G1: internal alpha** | Phase 1 complete | FR-LC-01 to 07, FR-FIO-01, 05, 06, 07, 11 and 14 pass on Tier 1 devices; the CI gate, QA APK and emulator smoke are green; no open S1; lifecycle torture suite ≥ 90% passing |
| **G2: closed beta** | Phase 2 complete for tablets | All Must FR-INP and FR-UI items for tablets pass; usability R2 passes ([05](05-ux-touch-design.md) §16); performance within 1.5× of the targets on RT; no open S1; ≤ 5 open S2; beta APK signed with the production key |
| **G3: public v1.0** | Phases 3 and 4 complete | Every **Must** requirement passes; the NFR-PERF targets are met on RT and BD (or waived with sign-off); beta crash rate < 1%, ANR < 0.3%; zero open data-loss defects; store listing approved; policy suite green; legal sign-off |

---

## 7. Defect management

| Severity | Definition | Release rule |
|---|---|---|
| **S1: critical** | Data loss or corruption; crash on launch; crash in a core workflow without a workaround; security issue | Blocks every gate; halts a rollout |
| **S2: major** | Crash or failure in a non-core feature, or with a workaround; severe performance regression | Each gate allows ≤ 5 open S2, each with a workaround documented |
| **S3: minor** | Functional issue with an easy workaround; layout problems | Tracked; fixed by priority |
| **S4: cosmetic** | Visual polish | Backlog |

- **Labels:** `platform:android` plus an area (`android:storage`, `android:input`, `android:gpu`, `android:ux`,
  `android:release`). Use the upstream issue tracker, so the community can see and help.
- **Triage:** twice a week during the beta, daily during staged rollout.
- **Every crash fix** comes with a synthetic regression test that failed before the fix (upstream never-crash rule).

---

## 8. Tools

| Purpose | Tool |
|---|---|
| Build and inspect | cargo-ndk, Gradle, APK Analyzer, `aapt2`, `apksigner`, `zipalign`, `bundletool`, `llvm-readelf` |
| Automation | adb; the upstream control protocol (QA build); AndroidX Test / UI Automator; an emulator runner in GitHub Actions |
| Performance | Perfetto, `simpleperf`, `dumpsys gfxinfo` and `meminfo`, `am start -W`, upstream `cargo xtask perf` scenarios |
| Stability | `ApplicationExitInfo` logs, tombstones plus native symbols, Play vitals, StrictMode and `-Xcheck:jni` (QA) |
| Devices | The device lab (§4); an optional device farm |

---

## 9. Test data

- **Images:** public-domain only (upstream rule). The upstream real-file corpora (photocraft-corpus, psd-tools,
  ag-psd, PngSuite) are fetched by `cargo xtask corpus --all` and pushed to devices for L4 and L6 runs.
- **Synthetic large files:** generated by the existing test generators (PSB > 1 GB, 100+ layers, 16 and 32-bit).
- **Phone-native formats:** HEIC, Ultra HDR JPEG and DNG captured on the team's test devices of non-personal scenes,
  stored with the test assets under a permissive licence.
- **Hostile files:** the upstream fuzz corpora and malformed samples, plus hostile names for the intent tests.
