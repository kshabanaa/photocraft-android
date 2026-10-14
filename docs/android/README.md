# PhotoCraft for Android: Business Requirements Document (BRD) package

| | |
|---|---|
| **Product** | PhotoCraft, the open-source, pure-Rust image editor: Android edition |
| **Package version** | 1.0 (draft for review) |
| **Date** | 2026-10-13 |
| **Upstream baseline** | [storytold/photocraft](https://github.com/storytold/photocraft) `main` at `2d5a749` (workspace version 0.8.2, committed 2026-10-13) |
| **Status** | Draft. Phase 1 can start once the decisions in [§2](#2-decisions-needed-before-phase-1) are signed off |

---

## 1. Executive summary

### Goal

Ship PhotoCraft as an installable, signed Android app:

- an **APK** for direct download (GitHub Releases and getartcraft.com), and
- an **Android App Bundle (AAB)** for Google Play,

built from the same Rust workspace and released under the same version numbers as the desktop and web editions.

### Approach: a native port, not a rewrite and not a web wrapper

PhotoCraft's engine and UI already run on macOS, Windows, Linux, FreeBSD and in the browser. Android becomes a third
*shell*, alongside the desktop app (`apps/photocraft`) and the web app (`apps/photocraft-web`):

- **`apps/photocraft-android`**: a Rust `cdylib` that starts eframe through `android-activity` (GameActivity) and
  implements the UI's platform seam for Android. That seam is the `Services` struct in `crates/ui-egui/src/lib.rs`.
- **`packaging/android`**: a small Gradle project with the manifest, icons and a thin Kotlin layer. The Kotlin layer
  covers only what Android exposes solely through Java APIs: the Storage Access Framework, the share sheet, intents,
  the clipboard, stylus axes, window insets and back navigation.

### Why it is feasible

- **The dependency tree is pure Rust.** The decoders (zune-jpeg, png, tiff, exr, image-webp, heic-rs), the plug-in
  host (wasmi), SVG (resvg) and compression (flate2's Rust backend, ruzstd, blake3 `pure`) need no C or C++ libraries
  to cross-compile. Phase 0 confirms that the whole graph compiles for `aarch64-linux-android`.
- **The UI stack supports Android.** eframe 0.36.2, egui-winit 0.36.2 and winit 0.30.13 ship an Android backend
  through `android-activity`. Version 0.6.1 of that crate is already in `Cargo.lock`, together with `ndk` 0.9 and
  `jni` 0.22. wgpu 30 renders through Vulkan or OpenGL ES on Android.
- **A clean platform seam already exists.** `Services` holds about 35 optional injected functions: file dialogs,
  import and export, background save, clipboard, preferences, autosave and recovery, OS events, quit and others.
  The desktop and web shells each provide one; Android provides a third. The web shell already shows that the UI
  runs without a desktop file system (`FileDialogAnswer::Contents`, `Services::inbox`).
- **The robustness machinery is reusable.** Crash-safe GPU startup (`apps/photocraft/src/gpu_startup.rs`), crash
  recovery (`photocraft_format::RecoveryStore`) and the stylus feed (`crates/ui-egui/src/stylus.rs`) map directly
  onto Android's problems: driver crashes, process death and pen input.

### What makes it hard

- **The UI is a dense desktop UI.** It is modelled 1:1 on Photoshop: 67 tools, 628 menu items, about 30 panels,
  modifier-key workflows and hover feedback. Making it work by touch, and on phones, is the largest work item.
- **winit 0.30 has gaps on Android.** It drops stylus tilt, orientation, tool type and batched samples, and its
  soft-keyboard and IME support is weak. A small Kotlin bridge supplies these.
- **Android itself works differently.** Storage is URI-based (SAF), not path-based. The OS can kill the process at
  any time. Mobile GPUs and drivers vary widely, and memory is tighter.
- **The project's own rules constrain the design:**
  - `unsafe_code = "forbid"`: JNI and the `android_main` export need an isolated exception, following the
    `crates/tablet` precedent.
  - "Rust only": a thin Kotlin layer is unavoidable and needs maintainer approval.
  - Very high upstream velocity: more than 1,000 PRs merged in the first two weeks of the project, so a fork would
    drift within days.

### Who builds it

**AI agents, with no human engineers** (decision D-14). Claude Code sessions do the engineering, review, QA
automation and release work, in parallel waves. The owner makes decisions, holds the accounts, keys and money, and
tests on real devices: about 20–35 hours in total, mostly at the gates.
See [11-ai-delivery-plan.md](11-ai-delivery-plan.md).

### Effort and timeline

| Scope | Effort (agent-hours, upstream's calibration) | Calendar (2–3 parallel agent sessions; owner about 1 h a day) |
|---|---|---|
| Tablet-first v1.0 (tablets, Chromebooks, foldables, desktop mode; phones usable) | 660–1,350 | about 14 weeks: G3 around 2027-01-22 |
| Phone-optimized layout (v1.1) | +80–160 | +4 weeks: around 2027-02-19 |

The calendar is bound by owner availability, usage limits, device testing and Play's fixed waits, not by agent
speed. Detail: [08](08-roadmap-wbs-estimates.md) and [11](11-ai-delivery-plan.md) §8.

### Recommendation

1. **Upstream first.** Contribute Android as a supported target of `storytold/photocraft`, built and released by
   the same pipeline. Do not maintain a long-lived fork.
2. **Tablet, Chromebook and desktop mode first.** Phones are usable in v1.0 and optimized in v1.1.
3. **64-bit only** (`arm64-v8a`, `x86_64`), **minSdk 29** (Android 10), **targetSdk 36** (Android 16),
   **GameActivity**.
4. **Ship the APK on GitHub and the website, and the AAB on Google Play**, from one pipeline. F-Droid comes after
   v1.0.
5. **No `INTERNET` permission and no telemetry in release builds.** This keeps the product promise: "offline, and
   yours".
6. **AI agents build it**, under upstream's agent rules (`AGENTS.md`), with independent reviewer agents, CI gates
   and owner device checks at every gate ([11](11-ai-delivery-plan.md)).

---

## 2. Decisions needed before Phase 1

The full decision log, with the options and rationale, is in [09](09-risks-decisions-questions.md#2-decision-log).

| ID | Decision | Recommendation | Due |
|---|---|---|---|
| D-01 | Who publishes: the official upstream target, or an independent fork | **Decided 2026-10-13: the owner's independent build** (own app ID and name, no ArtCraft marks, own releases). Consequences: [11](11-ai-delivery-plan.md) §10 | Decided |
| D-15 | App name and application ID of the independent build | A distinct name, plus an ID in the owner's namespace (for example `io.github.<user>.<app>`) | Before the first public APK |
| D-02 | Form-factor scope of v1.0 | Tablets, Chromebooks and desktop mode first; phones basic in v1.0, optimized in v1.1 | G0 |
| D-03 | Kotlin or Java glue in the repository | Thin Kotlin layer in `packaging/android`, about 1,500 lines at most, with no business logic | G0 |
| D-04 | Activity type | GameActivity (better IME support; AndroidX-based) | G0 |
| D-05 | Policy for `unsafe` | One isolated JNI module (`deny` crate-wide; `allow` only there, with `SAFETY:` comments) | G0 |
| D-06 | SDK levels and ABIs | minSdk 29, targetSdk 36, `arm64-v8a` and `x86_64` only | G0 |
| D-08 | Signing strategy | One app signing key for the GitHub APK and Play (own key uploaded to Play App Signing) | G1 (before the first public APK) |
| D-10 | Network and telemetry | No `INTERNET` permission in release builds; no telemetry | G0 |
| D-14 | Delivery model | AI agents build everything; the owner decides, holds the accounts and keys, and tests on devices (**decided** 2026-10-13) | Decided |

---

## 3. Document map

| # | Document | Contents | Primary readers |
|---|---|---|---|
| 00 | [README.md](README.md) (this file) | Summary, decisions, map, glossary, evidence | Everyone |
| 01 | [01-business-requirements.md](01-business-requirements.md) | Background, objectives, KPIs, stakeholders, personas, market, options, scope, business requirements, assumptions, constraints, costs, sign-off | Sponsor, product owner, maintainers |
| 02 | [02-functional-requirements.md](02-functional-requirements.md) | What the Android app must do: lifecycle, files, features, input, UI, settings, text, clipboard, localization, help, updates, QA hooks | Engineers, UX, QA |
| 03 | [03-non-functional-requirements.md](03-non-functional-requirements.md) | Compatibility, performance budgets, stability, data integrity, size, security and privacy, accessibility, maintainability, policy compliance | Engineers, QA, release |
| 04 | [04-technical-architecture.md](04-technical-architecture.md) | Solution architecture, repository layout, toolchain, `Services` mapping, JNI bridge, storage, GPU, input, lifecycle, upstream change catalogue | Engineers, maintainers |
| 05 | [05-ux-touch-design.md](05-ux-touch-design.md) | Touch and stylus UX: layouts per window size, gestures, the Touch shortcut, text entry, panels, dialogs, file flows, wireframes | UX, engineers |
| 06 | [06-build-release-distribution.md](06-build-release-distribution.md) | Build pipeline, versioning, signing, CI, GitHub, website, Play and F-Droid distribution, store listing, rollout | Release manager, engineers |
| 07 | [07-test-qa-plan.md](07-test-qa-plan.md) | Test strategy, device matrix, test areas, gates, defect policy, tools, test data | QA, engineers |
| 08 | [08-roadmap-wbs-estimates.md](08-roadmap-wbs-estimates.md) | Phases, gates, work breakdown with hours, schedule, staffing, critical path, cost | Sponsor, product owner, leads |
| 09 | [09-risks-decisions-questions.md](09-risks-decisions-questions.md) | Risk register, decision log, open questions, assumptions to validate | Sponsor, leads |
| 10 | [10-traceability-matrix.md](10-traceability-matrix.md) | Business requirements traced to functional and non-functional requirements, verification and phases | QA, product owner |
| 11 | [11-ai-delivery-plan.md](11-ai-delivery-plan.md) | How AI agents build it: roles, owner-only tasks, machine readiness, branching, brief template, waves, quality controls, AI calendar and cost | Owner, lead agent |
| 12 | [12-phase0-spike-report.md](12-phase0-spike-report.md) | Phase 0 results: environment, compile checks, resolved [verify] items, problems and fixes, remaining spike steps | Owner, agents |

**Reading paths.** Owner: README, 11, 01, 09. Agents: 11, then 02, 03, 04, 06 for their work package. UX work:
05, 02. QA work: 07, 10, 03.

---

## 4. Conventions

- **Requirement IDs**
  - `BR-xx`: business requirement
  - `FR-<area>-xx`: functional requirement
  - `NFR-<area>-xx`: non-functional requirement
  - `UC-xx`: upstream code change
  - `D-xx`: decision
  - `R-xx`: risk
  - `Q-xx`: open question
  - `A-xx`: assumption
  - `C-xx`: constraint
  - `DEP-xx`: dependency
- **Priority (MoSCoW)**
  - **Must**: release-blocking for v1.0
  - **Should**: planned for v1.0, can slip to v1.1 with sign-off
  - **Could**: planned only if capacity allows
  - **Won't**: excluded from this release; recorded so the exclusion is visible
- **Gates**
  - **G0**: feasibility go/no-go
  - **G1**: internal alpha
  - **G2**: closed beta
  - **G3**: public v1.0
- **Paths.** File paths refer to the upstream repository at the baseline commit unless stated otherwise.
- **Hours.** Estimates are agent-hours given as ranges, the unit upstream uses for its own estimates (`ROADMAP.md`).
  This project is built by AI agents (D-14).

---

## 5. Glossary

| Term | Meaning |
|---|---|
| **AAB** | Android App Bundle: the publishing format Google Play requires. Play generates per-device APKs from it. |
| **ABI** | Application Binary Interface: the CPU architecture of native code (`arm64-v8a`, `x86_64`, …). |
| **AGP** | Android Gradle Plugin. |
| **ANR** | "Application Not Responding": the dialog shown when the main thread is blocked too long. Play counts its rate. |
| **APK** | Android Package: the installable file. Distributed directly for sideloading. |
| **ARCVM** | The virtual machine that runs Android apps on ChromeOS. |
| **DeX / desktop windowing** | Desktop-style, freely resizable windows on Samsung devices (DeX) and on large-screen Android. |
| **FileProvider** | AndroidX component that exposes selected app files to other apps through temporary `content://` grants. |
| **GameActivity** | AndroidX Activity for native apps. It improves on `NativeActivity`, notably with GameTextInput for the soft keyboard. |
| **JNI** | Java Native Interface: calls between Kotlin/Java and Rust. |
| **LMK** | Low-memory killer: Android ends background (and sometimes foreground) processes under memory pressure. |
| **MediaStore** | Android's shared media index. Apps add images to `Pictures/` through it without any permission. |
| **NDK** | Android Native Development Kit: the toolchain and sysroot for native code. |
| **Photo Picker** | The system image picker; it needs no storage permission. |
| **Play App Signing** | Google holds the app signing key; developers upload with a separate upload key. |
| **SAF** | Storage Access Framework: system pickers that return `content://` URIs with scoped read and write grants. |
| **`Services`** | PhotoCraft's platform seam: the struct of optional functions each shell injects into `PhotocraftApp`. |
| **`StylusFeed`** | PhotoCraft's pen-sample channel (`crates/ui-egui/src/stylus.rs`), filled per platform. |
| **versionCode** | The monotonically increasing integer Android uses to order app updates. |
| **Window size class** | Material/Android width buckets: Compact (< 600 dp), Medium (600–839 dp), Expanded (≥ 840 dp). |
| **16 KB page size** | Newer devices use 16 KB memory pages. Play requires 64-bit native libraries aligned for them. |

---

## 6. Evidence base

**Upstream files examined** at `2d5a749`:

- Root: `README.md`, `ROADMAP.md`, `AGENTS.md`, `Cargo.toml`, `Cargo.lock`, `.cargo/config.toml`
- Apps: `apps/photocraft/{Cargo.toml, src/main.rs, src/services.rs, src/gpu_startup.rs, src/tablet.rs}`,
  `apps/photocraft-web/{Cargo.toml, src/main.rs, src/web.rs}`
- UI (`crates/ui-egui/src/`): `lib.rs` (`Services`), `file_dialog.rs`, `stylus.rs`, `gpu_canvas.rs`,
  `workspace_ui.rs`, `titlebar.rs`
- Text: `crates/text/src/{fonts.rs, cjk.rs}`
- Other crate manifests: `crates/{gpu, tablet, heif, plugins, codecs, text, engine, io, format}/Cargo.toml`
- Docs: `docs/{releasing.md, hardware-parity.md}`, `docs/brand/LICENSE-brand.txt`,
  `assets/app-icon/LICENSE.txt`
- CI: `.github/workflows/release.yml`, which delegates to `storytold/craft-common-actions`
- Pinned shared libraries: `storytold/craft-libs` at the pinned revisions (`craft-print`, `craft-ui`)

**External references**, checked 2026-10-13; re-check each one before submission:

- Google Play target API level requirements: <https://developer.android.com/google/play/requirements/target-sdk> and
  <https://support.google.com/googleplay/android-developer/answer/11926878>. New apps and updates must target API 36
  from 2026-08-31, with an extension available to 2026-11-01.
- 16 KB page-size support: <https://developer.android.com/guide/practices/page-sizes>. Required on Play for apps
  targeting Android 15 and later since 2025-11-01.
- GameActivity text input (GameTextInput): <https://developer.android.com/games/agdk/add-support-for-text-input>
- Android developer verification for apps installed outside Google Play. Enforcement began around 2026-09-30 in
  Brazil, Indonesia, Singapore and Thailand; the global rollout is planned for 2027; adb and an "advanced flow"
  still allow unregistered installs. Secondary reports:
  - <https://www.bitdefender.com/en-au/blog/hotforsecurity/google-developer-verification-sideloaded-apps>
  - <https://www.malaysianwireless.com/2026/06/google-android-developer-verification-sept-30/>

  Confirm the details in the Android Developer Console.
- New personal Play developer accounts (created after 2023-11-13) must run a closed test with at least 12 testers for
  14 consecutive days before production access. Organization accounts are exempt. Source: Play Console Help, "App
  testing requirements for new personal developer accounts".
