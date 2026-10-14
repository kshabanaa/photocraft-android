# 01: Business requirements

| | |
|---|---|
| **Document** | PhotoCraft for Android: business requirements |
| **Version** | 1.0 draft, 2026-10-13 |
| **Owner** | The project owner; AI agents deliver (D-14, see [§6](#6-stakeholders-and-raci)) |
| **Related** | [02 functional](02-functional-requirements.md), [03 non-functional](03-non-functional-requirements.md), [08 roadmap](08-roadmap-wbs-estimates.md), [09 risks and decisions](09-risks-decisions-questions.md) |

---

## 1. Background

### 1.1 The product today

PhotoCraft is an open-source (MIT OR Apache-2.0), clean-room image editor comparable to Photoshop, written entirely
in Rust by the ArtCraft team and community. Upstream facts at the baseline commit `2d5a749` (2026-10-13):

| Attribute | Value |
|---|---|
| Version and stage | 0.8.2, **alpha** (README; `ROADMAP.md`: "ready for real work" about 45%, feature breadth about 76%) |
| Age and velocity | First commit 2026-09-30; more than 1,000 merged PRs; releases roughly every one or two days (0.2.0 on 2026-10-05, 0.6.0 on 2026-10-10) |
| Architecture | 24 crates in strict layers (L0 foundation through L6 UI), enforced by `cargo xtask layers`. Pure-data document model, a command engine with 500+ commands, and a thin egui UI on top. CPU compositor as the reference oracle; wgpu GPU compositor for the canvas |
| UI stack | eframe/egui 0.36.2, winit 0.30.13, wgpu 30 |
| Platforms shipped | macOS (universal), Windows (x64, x86, arm64), Linux (x86_64, aarch64, riscv64), FreeBSD 14, Web (WebAssembly) |
| Formats | PSD/PSB with high-fidelity round trips, layered TIFF, `.pcraft`, OpenRaster, Krita and Paint.NET (read), Affinity (read), SVG, the common flat formats, OpenEXR, HDR, FITS, camera raw (DNG, CR2, NEF, ARW, RAF, …) and HEIC (read) |
| Product promises | "Open source, offline, and yours"; no account; never crash, never lose work (`AGENTS.md`, "Never crash") |
| Release pipeline | A push to the `release` branch runs the shared workflow `storytold/craft-common-actions`, which builds signed artifacts into a draft GitHub Release (`docs/releasing.md`) |

### 1.2 Android today

There is **no Android build**. Android users can only open the web build in a browser (Chrome on Android supports
WebGPU, with a WebGL2 fallback). That route has hard limits:

- The WebAssembly build is single-threaded and capped at 24 MiB. The Japanese dictionary and complex-script line
  breaking are left out to meet the cap.
- There is no file system. Saving means downloading a file, and there is no "Save" back to the original.
- A 32-bit WebAssembly address space caps how large a document can be.
- Users get no app icon, intents ("Open with"), share-sheet integration or stylus tilt.

### 1.3 Why now

- **Desktop-class form factors are growing on Android.** Large tablets with styluses (Galaxy Tab S, Pixel Tablet),
  foldables, Samsung DeX, desktop windowing on large screens and Chromebooks are the hardware where a layered PSD
  editor makes sense. PhotoCraft's desktop-grade UI fits them unusually well.
- **The codebase is portable now and will get harder to port later.** Every week the upstream project adds code that
  assumes desktop operating systems. An early "never break Android" CI gate costs far less than a late port.
- **Android is open to direct APK distribution, but the rules are tightening.** Developer verification for
  sideloaded apps started around 2026-09-30 in four countries and goes global in 2027. Setting up signing,
  verification and Play presence early avoids friction later.

---

## 2. Problem and opportunity statement

> Android tablet, Chromebook and phone users who work with layered images, PSD files in particular, have no free,
> offline, open-source editor that opens and saves real PSDs with layers, masks, adjustment layers, type and layer
> styles. The available apps are subscription-based, ad-supported, phone-oriented simplified editors, or browser
> tools without file-system integration. PhotoCraft already has the engine and the desktop-class UI. What it lacks is
> an Android shell, touch and stylus input adaptations, and a distribution channel.

---

## 3. Business objectives

| ID | Objective | Measured by (see §5) |
|---|---|---|
| BO-1 | Extend PhotoCraft's reach to Android tablets, Chromebooks, foldables and phones with a native, offline, signed app | Installs, active devices, availability on Play and GitHub |
| BO-2 | Keep a **single codebase**: Android is a build target of the same workspace, released with the same version as desktop and web | Android artifacts in every release; no fork |
| BO-3 | Preserve the product promises on Android: offline, no account, open source, real PSD files, never lose work | Zero-permission manifest; zero data-loss defects; crash and ANR rates |
| BO-4 | Deliver a touch and stylus experience good enough for real work on tablets, and a usable one on phones | Workflow completion tests; beta satisfaction; store rating |
| BO-5 | Build the Android platform layer so the other Crafting Apps (VectorCraft, FilmCraft, LightCraft, PdfCraft, EffectCraft, DesignCraft) can reuse it | Reusable crate and Gradle template in `craft-libs` (Should) |

---

## 4. Target users and personas

| Persona | Device and context | Needs | Pain today | Priority |
|---|---|---|---|---|
| **P1: Tablet creator** (illustrator or photo retoucher) | Galaxy Tab S or Pixel Tablet with a stylus; sometimes a keyboard cover or DeX | Layers, brushes with pressure and tilt, masks, adjustment layers, exchanging PSDs with a desktop | Android painting apps lack Photoshop-grade PSD fidelity; Photoshop-class tools are subscription and desktop-only | Primary |
| **P2: Chromebook learner** (student or teacher) | School Chromebook, x86_64 or ARM, keyboard and touchpad | A free, offline editor whose skills transfer to Photoshop; no account; works without network | Web tools need connectivity; installed options are limited | Primary |
| **P3: Professional on the go** (designer or marketer) | Phone or foldable | Open a client's PSD from Drive or email, check layers, change a text layer, export JPEG or PNG, share | Viewing a PSD on a phone usually means flattening it or uploading it to a cloud service | Secondary (phone layout in v1.1) |
| **P4: Open-source enthusiast** | Any device; often without Google Play services | Open source, no tracking, installed from GitHub or F-Droid, verifiable signatures | Most editors include ads or analytics | Secondary |
| **P5: Desktop PhotoCraft user** | Desktop plus a tablet | The same app, files and shortcuts on both; continuity through `.pcraft` and PSD | None, until Android exists | Secondary |

---

## 5. Success metrics (KPIs)

Targets marked "proposed" are business choices for the sponsor to confirm (Q-08).

| KPI | Target | Source | When |
|---|---|---|---|
| Release parity | Android artifacts (APKs and AAB) produced by every PhotoCraft release from v1.0, with the same version | GitHub Releases, Play Console | From G3 |
| User-perceived crash rate | < 1.0% of daily active users (Play's bad-behaviour threshold is 1.09%); stretch < 0.5% | Play vitals; `ApplicationExitInfo` in beta builds | 30 days after G3 |
| User-perceived ANR rate | < 0.3% (Play threshold 0.47%) | Play vitals | 30 days after G3 |
| Data integrity | 0 confirmed data-loss defects open for more than 7 days | GitHub issues labelled `platform:android` and `data-loss` | Continuous |
| Start-up time | Cold start to interactive ≤ 2.5 s P50 on the reference tablet; ≤ 5 s on the baseline device | Perfetto or `am start -W` | G2 and G3 |
| Compatibility | The app reaches an interactive UI on 100% of the Tier 1 and Tier 2 device matrix, through the GPU fallback where needed | Test lab ([07](07-test-qa-plan.md)) | G3 |
| Touch workflow completion | All six upstream "alpha gate" workflows (retouch, composite, cut-out, design, paint, PSD exchange) completed by touch and stylus on a tablet by 5 of 5 test users | Moderated usability tests | G2 and G3 |
| Engine parity | ≥ 95% of desktop engine commands enabled on Android, with every exception documented | Generated parity report | G3 |
| Download size | ≤ 60 MB per ABI | APK Analyzer; CI size report | Every release |
| Rating (proposed) | ≥ 4.2 stars on Play after 90 days (with at least 50 ratings) | Play Console | G3 + 90 days |
| Adoption (proposed) | 10,000 installs across channels within 90 days of G3 | Play Console plus GitHub download counts | G3 + 90 days |

---

## 6. Stakeholders and RACI

### 6.1 Stakeholders

Delivery model D-14: AI agents build everything; there are no human engineers. Roles are detailed in
[11](11-ai-delivery-plan.md) §1.

| Role | Who | Interest |
|---|---|---|
| **Owner** (sponsor and product owner) | The person who commissioned this BRD | Scope, decisions, accounts, money, acceptance, device testing |
| Lead agent | The main Claude Code session | Plan, briefs, integration, reporting |
| Implementer agents | Parallel Claude Code sessions, one per work package | Code, tests, docs |
| Reviewer agents | Separate sessions (optionally another AI tool) | Independent review against requirements and upstream rules |
| QA agent | Claude Code session with emulator and adb | Automated suites; defects |
| Release agent | Claude Code session | Packaging, CI, store and policy drafts |
| Upstream maintainers | `storytold/photocraft` maintainers (if D-01 = official) | Code quality, project rules, review load |
| ArtCraft team | Brand owner | Trademarks; official listing (if D-01 = official) |
| Beta testers and usability participants | Discord community, friends | Real-device feedback; the 12-tester closed test |
| End users | Personas P1–P5 | Usability, reliability, privacy |

### 6.2 RACI (R = responsible, A = accountable, C = consulted, I = informed)

| Deliverable | Owner | Lead agent | Implementer agents | Reviewer agents | QA agent | Release agent | Upstream maintainers |
|---|---|---|---|---|---|---|---|
| BRD approval and decisions (D-xx) | A | R | I | I | I | I | C |
| Architecture and upstream changes (UC list) | A | R | R | C | I | I | C |
| Kotlin layer and `unsafe` exception | A | R | R | R (two reviews) | I | I | C |
| Touch UX design | A | R | R | C | C | I | I |
| Test automation and device runs | A (device checks) | C | C | I | R | I | I |
| Signing keys and store accounts | A/R (keys, accounts, payments) | C | I | I | I | C (drafts, scripts) | I |
| Store listing and privacy policy | A (submits) | C | I | C | I | R (drafts) | I |
| Upstream PRs and messages | A (sends) | R (drafts) | R | C | I | I | C |
| Go/no-go at each gate | A | R (gate report) | I | C | C | C | I |

---

## 7. Market and competitive context

The landscape as of 2026, summarized; verify the current status of each product before using this in marketing.

| Product | Model | Strengths | Gap PhotoCraft fills |
|---|---|---|---|
| Adobe Photoshop on Android (beta 2025), Photoshop Express, Lightroom | Account; freemium or subscription | Brand, AI features, cloud | Offline, no account, open source, full desktop-class PSD editing on tablets and Chromebooks |
| Krita (Android and ChromeOS builds) | Free, open source | Painting, open source | Photoshop-style workflow and fidelity of PSD adjustment layers and layer styles |
| Clip Studio Paint, Infinite Painter, ibis Paint X | Subscription or ads | Illustration | PSD round trips with adjustment layers and type; no ads or subscription |
| Snapseed, PicsArt, Pixlr | Free with ads, or freemium | Quick phone edits | Layered PSD documents; 16 and 32-bit; CMYK and Lab |
| Photopea (browser) | Free with ads | PSD in a browser | Native performance, offline use, file integration, stylus tilt, no ads |

**Differentiators to protect in the Android edition:** real PSD/PSB round trips; 8, 16 and 32-bit and CMYK/Lab
documents; adjustment layers and layer styles; offline use with no account and no ads; open source; the same engine
and files as desktop; tablet stylus support; Photoshop-familiar menus and shortcuts on Chromebooks and in desktop mode.

---

## 8. Options analysis

| # | Option | Pros | Cons | Verdict |
|---|---|---|---|---|
| A | **Native APK/AAB from the same Rust workspace** (eframe on Android, a Kotlin bridge, `packaging/android`) | One codebase; full performance (multi-threaded, no WebAssembly limits); real file integration; stylus; consistent with "native egui/eframe on wgpu" | Needs the Android shell, touch UX work and device testing; a thin Kotlin layer and a JNI `unsafe` exception | **Recommended** |
| B | Wrap the web build in a WebView or Trusted Web Activity APK | Fast to package | Violates the upstream rule "No Tauri, Electron or webview shells … never add … any webview … as a packaging target" (`AGENTS.md`); keeps every WebAssembly limit (single thread, 24 MiB cap, no SAF) | **Rejected** |
| C | Promote the hosted web build as an installable PWA | No engineering; available now | Not an APK; same WebAssembly limits; no "Save" in place; no intents | **Interim only** (can be documented now) |
| D | Rewrite the UI in Kotlin/Compose over the Rust engine (UniFFI) | Most "native" Android look | A second UI to maintain; loses 1:1 command and UI parity; conflicts with the "Rust only" rule; many times the effort | **Rejected for v1** |
| E | Independent long-lived fork with Android support | No dependence on upstream review | Upstream merges dozens of PRs a day, so the fork drifts within days; brand rules require removing ArtCraft marks; duplicated releases | **Not recommended** (fallback if upstream declines; see R-01) |

---

## 9. Scope

### 9.1 In scope (v1.0)

- An Android app built from the PhotoCraft workspace, with the same version and the same engine and command
  registry as desktop.
- ABIs `arm64-v8a` (devices) and `x86_64` (Chromebooks, emulators). Android 10 (API 29) and later.
- Form factors:
  - **Full support:** tablets, Chromebooks, unfolded foldables, desktop mode (DeX and desktop windowing).
  - **Basic support:** phones (usable; optimized in v1.1 per D-02).
- All editing features of the engine, except the documented platform exceptions (see FR-FEAT-02).
- File handling:
  - Opening and saving through the Storage Access Framework, including in-place "Save".
  - Photo Picker import; "Open with" and share-target intents; sharing results; export to the gallery.
  - Recent documents.
- Touch gestures, an on-screen modifier control, stylus pressure, tilt and eraser end, palm rejection, physical
  keyboard, mouse and trackpad.
- Autosave and recovery after background kills and crashes; a crash-safe GPU start-up with automatic fallback.
- System fonts, user-imported fonts and CJK fallback; all UI languages shipped on desktop; system light and dark
  themes.
- Distribution: signed APKs on GitHub and the website; AAB on Google Play.

### 9.2 Out of scope for v1.0 (Won't)

| Item | Reason | Revisit |
|---|---|---|
| iOS/iPadOS edition | Separate platform and BRD | Later |
| TCP control server, MCP server and CLI in release builds | Needs `INTERNET`; desktop and automation use case. Available in **QA builds** for automated testing (FR-QA-01) | — |
| Printing | Low mobile value; needs PrintManager integration | v1.1 (FR-FEAT-08) |
| Colour sampling outside the app window; display ICC profiles; HDR output | Needs MediaProjection; colour work on Android needs a separate design | Later |
| Cloud sync, accounts, AI and generative features | Follows the desktop roadmap and product decisions | Desktop roadmap |
| In-app purchases and ads | Product is free and open source (A-05) | — |
| 32-bit `armeabi-v7a` and `x86` | Memory-bound workloads; 64-bit is the Play baseline; small and shrinking device share | If demand appears |
| Wear OS, Android TV, Automotive, XR | Not relevant to the product | — |
| Desktop-only integrations: Photoshop shortcut import from an installed Photoshop, Touch Bar, scanners (WIA), caption buttons, native menu bar, separate OS windows per document | Do not exist on Android | — |

### 9.3 Future scope (candidates after v1.0)

- Phone-optimized "Compact" layout, if deferred by D-02.
- PrintManager printing.
- Camera capture into a new document.
- Platform decoders for formats the Rust codecs cannot read (AVIF).
- Wide-gamut (Display P3) canvas.
- Accessibility with screen readers (TalkBack).
- Galaxy Store listing.
- Reusing the Android shell for the other Crafting Apps (BO-5).

---

## 10. Business requirements

| ID | Requirement | Rationale | Priority | Acceptance |
|---|---|---|---|---|
| BR-01 | Provide a signed Android APK for `arm64-v8a` and `x86_64`, plus a universal APK, with every PhotoCraft release from v1.0, named and checksummed like the other artifacts | Direct availability without a store; matches the existing release practice | Must | The release draft contains `photocraft-<v>-android-*.apk` files and their `SHA256SUMS.txt` lines; `apksigner verify` passes; the APK installs on a clean device |
| BR-02 | Publish PhotoCraft on Google Play as an AAB that complies with current Play policy (target API, 16 KB page alignment, 64-bit, Data safety, privacy policy) | Reach and automatic updates | Must | Production track live; no policy warnings in Play Console; pre-launch report free of crashes |
| BR-03 | Android uses the same engine and command registry as desktop; feature differences are only the documented platform exceptions | BO-2; avoids a "lite" fork | Must | Parity report: at least 95% of commands enabled; every exception listed in FR-FEAT-02 |
| BR-04 | Documents opened and saved on Android are compatible with desktop: identical bytes for flat formats and identical renders for PSD/PSB and `.pcraft` with the same settings | Exchange between desktop and tablet (P1, P5) | Must | Cross-platform golden tests pass ([07](07-test-qa-plan.md) §3.3) |
| BR-05 | The app is usable for real work by touch and stylus on tablets, and usable on phones | BO-4 | Must (tablet), Should (phone layout) | Workflow completion KPI met; usability tests pass |
| BR-06 | Never lose user work: autosave and recovery across backgrounding, process death, crashes and failed saves | Product promise | Must | Lifecycle torture tests pass; zero open data-loss defects at G3 |
| BR-07 | Offline and private: the release build has no network permission, no telemetry and no data collection, and the Data safety form says "No data collected" | Product promise; trust; F-Droid eligibility | Must | Manifest audit in CI; Data safety and privacy policy published |
| BR-08 | Meet stability and performance targets on the reference device matrix | Play visibility depends on vitals; user trust | Must | NFR-STAB and NFR-PERF targets met at G3 |
| BR-09 | All UI languages that desktop ships are available on Android, following the system language, with per-app language selection | Global audience (20 languages shipped) | Must (system language), Should (per-app) | Language switch tests pass; Android-only strings translated |
| BR-10 | Comply with licence and brand rules: ship NOTICE, ATTRIBUTION and font licences in the app; use ArtCraft marks only in official builds; describe Adobe trademarks only nominatively in listings | Legal | Must | Legal review sign-off; licences screen present |
| BR-11 | Keep upstream healthy: an Android CI gate on pull requests, layering rules intact, no regressions on desktop or web, and isolated, reviewed `unsafe` code | BO-2; maintainers' rules | Must | CI green on all targets; `cargo xtask layers` and `cargo xtask wasm` pass |
| BR-12 | Automate Android release builds (APKs, AAB, checksums, native symbols) in the release pipeline, with secrets in the `release` environment | Repeatable releases | Must | A push to `release` produces the Android artifacts unattended |
| BR-13 | Provide support channels and user-initiated diagnostics export (logs, exit reasons) without automatic data transmission | Supportability without telemetry | Should | The "Export Diagnostics" flow works; issue template updated |
| BR-14 | Meet an accessibility baseline (touch targets, contrast, font scale, keyboard operation) and publish a roadmap for screen-reader support | Inclusion; education market (P2) | Should | NFR-A11Y checks pass; known gaps disclosed |
| BR-15 | Structure the Android shell so other Crafting Apps can reuse it | BO-5 | Could | A reusable crate and template exist in `craft-libs`, or a documented extraction plan |

---

## 11. Assumptions

| ID | Assumption | Validation |
|---|---|---|
| A-01 | Upstream maintainers will accept Android as a supported target and review the required changes | RFC issue and Discord discussion in Phase 0 (Q-02) |
| A-02 | The eframe 0.36 / winit 0.30 Android backend, together with a Kotlin bridge, meets PhotoCraft's needs | Phase 0 spike on devices |
| A-03 | Every Rust dependency compiles for `aarch64-linux-android` and `x86_64-linux-android` with only the NDK | Phase 0: `cargo check` and `cargo ndk build` for the whole graph |
| A-04 | A thin Kotlin layer is acceptable despite the "Rust only" rule | Decision D-03 |
| A-05 | The app stays free: no ads, no in-app purchases, no telemetry | Sponsor confirmation (Q-11) |
| A-06 | The application ID is `ai.storyteller.photocraft` (the existing app ID) for the official build; forks use their own | D-01 |
| A-07 | Android releases follow the desktop release cadence through the same pipeline | D-11 |
| A-08 | Staffing: **AI agents only** (D-14), with 2–3 concurrent Claude Code sessions; the owner is available about 1 hour a day plus gate sessions ([11](11-ai-delivery-plan.md) §2) | Owner confirmation; usage limits measured in Phase 0 |
| A-09 | Budget is available for 6–8 test devices or a device farm | Q-05 |
| A-10 | An organization Play developer account is available (exempt from the 12-tester, 14-day rule), or the plan accepts that test | Q-07 |
| A-11 | Kick-off on 2026-10-19 (the schedule in [08](08-roadmap-wbs-estimates.md) uses this date) | Sponsor |
| A-12 | Upstream performance work on the desktop budgets (`perf/budgets.toml`) also benefits Android | Track in Phase 3 |

---

## 12. Constraints

| ID | Constraint | Source | Effect on the solution |
|---|---|---|---|
| C-01 | Rust-only product code; **no Tauri, Electron or webview shells**; `unsafe_code = "forbid"` except in one isolated, tested helper crate; non-test code never panics; layering enforced; never break WebAssembly; clean-room; never commit font files; the UI stays thin and data-driven | Upstream `AGENTS.md` | Native shell; Kotlin limited to platform glue; isolated JNI `unsafe`; Android code behind `cfg(target_os = "android")`; fonts loaded from the system or user, or from craft-fonts at build time |
| C-02 | Google Play requirements: target API 36 for new apps and updates from 2026-08-31; 16 KB-aligned 64-bit native libraries; 64-bit support; AAB format; Data safety form; privacy policy; metadata and intellectual-property policies | Google Play policy | targetSdk 36; linker flag and alignment checks; AAB in CI; store compliance tasks |
| C-03 | Android platform rules: scoped storage (no raw paths to user files); background limits; process death at any time; edge-to-edge (API 35); predictive back, with `KEYCODE_BACK` no longer delivered when targeting API 36; orientation and resizability restrictions ignored on large screens (API 36) | Android 10–16 behaviour changes | SAF-based I/O; autosave on pause; inset handling; back callback in Kotlin; responsive layouts |
| C-04 | Developer verification for apps installed outside Play: enforcement around 2026-09-30 in Brazil, Indonesia, Singapore and Thailand; global in 2027 | Google | Register the developer and the package/signing key before public APK distribution |
| C-05 | ArtCraft brand licence: ArtCraft marks may appear only in official PhotoCraft builds; forks must remove them. The app icon (the kitsune) is MIT OR Apache-2.0 | `docs/brand/LICENSE-brand.txt`, `assets/app-icon/LICENSE.txt` | An official build may show the marks; a fork must strip them (the brand mark PNG is embedded by `crates/ui-egui/src/brand.rs`) |
| C-06 | Licences: MIT OR Apache-2.0; NOTICE and ATTRIBUTION must ship; font licences (OFL) must ship with bundled fonts; AndroidX libraries are Apache-2.0 | Licences | In-app licences screen; packaging copies the licence files |
| C-07 | Upstream versions are pinned exactly (eframe, egui and wgpu), and upgrades are upstream decisions | `Cargo.toml` | Android must work with the versions upstream ships and must not force upgrades |
| C-08 | The release workflow is shared across the Crafting Apps (`storytold/craft-common-actions`, tracked at `@main`) | `.github/workflows/release.yml` | Android release jobs go into the shared workflow, or into a separate, coordinated workflow (D-11) |

---

## 13. Dependencies

| ID | Dependency | Type | Risk link |
|---|---|---|---|
| DEP-01 | eframe, egui-winit and winit Android backend (0.36.2 / 0.30.13) via `android-activity` 0.6.1 | Technical | R-02, R-16 |
| DEP-02 | wgpu 30 Vulkan and GLES backends on Android drivers | Technical | R-03 |
| DEP-03 | AndroidX GameActivity, pinned to the exact version `android-activity` 0.6.1 was built against (a mismatch crashes at start-up) | Technical | R-02 |
| DEP-04 | Android SDK platform 36, NDK r28 or later, AGP, Gradle and JDK 17 | Tooling | — |
| DEP-05 | `cargo-ndk` to build per-ABI `.so` files into `jniLibs` | Tooling | — |
| DEP-06 | `storytold/craft-libs` (`craft-ui`, `craft-print`) and `storytold/craft-fonts` | Upstream | — |
| DEP-07 | Google Play Console account; Android Developer Console (verification) | External | R-18, R-19 |
| DEP-08 | Upstream maintainers' review capacity | Organizational | R-01 |
| DEP-09 | Physical test devices or a device farm | Resources | R-17 |
| DEP-10 | AI agent capacity: plan limits for concurrent Claude Code sessions | Resources | R-25 |
| DEP-11 | Owner availability for decisions, device checks and account tasks | Organizational | R-23 |

---

## 14. Cost estimate

AI agents build the app (D-14), so there are no engineering salaries. Effort comes from the WBS in
[08](08-roadmap-wbs-estimates.md#3-work-breakdown-structure-and-estimates), counted in agent-hours. The AI cost model
is in [11](11-ai-delivery-plan.md) §8.

| Item | Tablet-first v1.0 | With phone layout | Notes |
|---|---|---|---|
| Engineering | 660–1,350 agent-hours | 740–1,510 agent-hours | No salaries. Cost is AI usage: heavy multi-session use needs a high-limit plan or API budget; usage is reported at every gate |
| Owner time | About 20–35 h | About 25–40 h | Decisions, accounts, device checks, submissions ([11](11-ai-delivery-plan.md) §2) |
| Test devices | The owner's own Android device, plus one stylus tablet recommended (about US$300–900) | same | A broader matrix comes from beta testers and an optional device farm |
| Device farm (optional) | US$0–1,500 | same | Burst testing on rare GPUs; check current pricing |
| Google Play developer registration | US$25 | same | One-time |
| Android developer verification (non-Play distribution) | Per Google's current terms | same | Early access reportedly offered a US$25 path; confirm |
| CI | US$0 on standard GitHub-hosted runners for a public fork | same | Larger runners optional |
| Ongoing maintenance | Agent sessions for upstream sync, yearly targetSdk bumps, device issues | same | The owner approves releases |

---

## 15. Milestones (summary)

| Gate | Target date (kick-off 2026-10-19) | Exit criteria (detail in [07](07-test-qa-plan.md) §6) |
|---|---|---|
| G0: feasibility go/no-go | 2026-10-23 | The workspace compiles for Android; the PhotoCraft UI renders on the emulator and on at least one physical device (the owner's); size, start-up and memory measured; D-01 to D-06 and D-10 decided |
| G1: internal alpha | 2026-11-13 | Open, edit, save and recover a PSD through SAF on the emulator and the owner's device; QA APK built in CI; no S1 defects |
| G2: closed beta | 2026-12-11 | Touch and stylus UX complete for tablets; beta APK on GitHub (pre-release) and the Play closed track |
| G3: public v1.0 | 2027-01-22 | All Must requirements met; vitals targets met in the beta; store listing approved |
| v1.1 | 2027-02-19 | Phone "Compact" layout (if deferred); printing; first fixes from user feedback |

Dates follow the AI-delivery calendar ([11](11-ai-delivery-plan.md) §8). They slip with owner availability and
device-only defects.

---

## 16. Approval

| Role | Name | Decision | Date | Signature |
|---|---|---|---|---|
| Owner | | Approve / Reject / Revise | | |
| Upstream maintainer (if D-01 = official) | | | | |
| ArtCraft brand owner (if D-01 = official) | | | | |
