# 11: AI delivery plan (no human engineers)

| | |
|---|---|
| **Version** | 1.0, 2026-10-13 |
| **Decision** | D-14: **AI agents build everything.** Claude Code sessions do the engineering, review, QA automation, release engineering and documentation. The owner (the person who commissioned this BRD) makes decisions, holds accounts, keys and money, and tests on real devices |
| **Supersedes** | Staffing assumptions A-08 ([01](01-business-requirements.md)) and the staffing scenarios in [08](08-roadmap-wbs-estimates.md) §4 |

Upstream PhotoCraft is itself built this way. Its `ROADMAP.md` calibrates all estimates in "Opus 5.5 agent
wall-clock hours" from 1,004 merged PRs, and its `AGENTS.md` is written for coding agents (parallel-agent rules, a
dev log, mandatory gates). The Android port follows the same working method.

---

## 1. Roles

| Role | Who | Responsibilities |
|---|---|---|
| **Owner** | The human | Decisions (D-xx), accounts (GitHub, Google Play, Android Developer Console), payments, signing-key custody, physical-device testing, store submission clicks, contact with upstream maintainers, go/no-go at each gate |
| **Lead agent (orchestrator)** | The main Claude Code session | Plans the waves, writes the work-package briefs, starts implementer sessions, integrates their branches, keeps `brd/` and the log current, reports to the owner, and escalates decisions |
| **Implementer agents** | Parallel Claude Code sessions, one per work package, each in its own git worktree with its own `CARGO_TARGET_DIR` | Code, tests and docs for one WBS item; follow upstream `AGENTS.md` (tests, clippy, `xtask layers`, `xtask wasm`, no panics, dev log) |
| **Reviewer agent** | A separate session or subagent, never the implementer of the same change; another AI tool can act as a second reviewer for high-risk changes | Reviews diffs against the requirement IDs and the upstream rules; runs `/code-review`; blocks merges with findings |
| **QA agent** | A Claude Code session with emulator and adb access | Runs the emulator and device suites ([07](07-test-qa-plan.md)) through adb and the QA control channel; captures screenshots and logs; files defects |
| **Release agent** | A Claude Code session | Packaging scripts, CI workflows, versionCode, checksums; drafts store listing text, privacy policy and release notes for the owner to submit |

Rules for every agent:

- **Briefs are complete.** Each brief names the goal, the files owned, the requirement IDs with their acceptance
  criteria, the tests to add, the gates to run and the definition of done (§5).
- **Files are owned.** Two writers never edit the same files at the same time. Shared upstream files (`canvas.rs`,
  `lib.rs`, `state.rs`, `menus.rs`) get small, sequenced edits (upstream `AGENTS.md` §6).
- **Gates are green before review.** `cargo test` for the touched crates, `cargo clippy -D warnings`,
  `cargo xtask layers`, `cargo xtask wasm`, the Android check, and the emulator smoke test where relevant.
- **Agents never touch secrets, accounts or money**, and never post outside the repository without the owner's
  explicit go-ahead. That covers upstream PRs, issues, Discord and store submissions; agents draft, the owner sends.
- **High-risk changes get two independent reviews and the owner's sign-off.** These are the JNI and `unsafe` module,
  the save and write path, signing and the release workflow, and the manifest and permissions.

---

## 2. What only the owner can do

| ID | Task | Needed by | Owner time |
|---|---|---|---|
| H-01 | Answer Q-01: official ArtCraft build or independent build. If official, contact the maintainers (agents draft the RFC message) | G0 | 1–2 h |
| H-02 | Approve the toolchain installs on this PC (§3) | Phase 0 start | 5 min |
| H-03 | Provide at least one Android device with USB debugging enabled, ideally a tablet with a stylus; plug it in when the QA agent asks | G0 (spike), then each gate | 1–3 h per gate |
| H-04 | GitHub: create or approve the fork; enable Actions; later add the signing secrets (§4) | Phase 1 | 30 min |
| H-05 | Create the Google Play developer account (registration fee, identity verification). Agents cannot create accounts or pay | Phase 4 (start early: verification takes days) | 1–2 h, plus waiting |
| H-06 | Android developer verification for APK distribution outside Play | Before the first public APK | 1 h, plus waiting |
| H-07 | Personal Play account only: recruit at least 12 testers for a 14-day closed test (Discord, friends) | G2 to G3 | A few hours |
| H-08 | Review the store listing, Data safety and content-rating answers; press Submit | G3 | 1–2 h |
| H-09 | Host the privacy policy (agents write it) | G3 | 30 min |
| H-10 | Brand and legal judgement (ArtCraft marks; trademark wording) | G3 | 1 h |
| H-11 | Hands-on checks: try the app on a device, judge stylus feel and usability, run the 6 core workflows, and recruit a few real users for the usability rounds ([05](05-ux-touch-design.md) §16) | G1, G2, G3 | 2–4 h per gate |
| H-12 | Keep the signing keystore and its passwords safe (two offline backups) | Phase 4 | 30 min |

The owner's total is roughly **20–35 hours** over the project, mostly at the gates.

---

## 3. Machine readiness (this PC, checked 2026-10-13)

| Item | Status | Action |
|---|---|---|
| CPU and RAM | Ryzen 7 3700X (16 threads), 32 GB | Enough for 2–3 concurrent heavy cargo builds plus one emulator. Limit parallel builds (`-j 6` each, or one shared build queue) |
| Disk | C: about 1.1 TB free | Fine. Each agent target directory takes about 10 GB (upstream `AGENTS.md`); clean up finished agents' target directories |
| Rust | 1.99.0 (workspace needs ≥ 1.95); host target only | `rustup target add aarch64-linux-android x86_64-linux-android` |
| Android Studio | Installed (bundled JDK 21) | Use its JDK for Gradle (`JAVA_HOME` → `jbr`) |
| Android SDK | `%LOCALAPPDATA%\Android\Sdk`: platforms 34 and 36, build-tools 36.1, emulator, platform-tools, a system image for API 36.1 | Add an **NDK r28+** and the **cmdline-tools** through the SDK Manager. Add an API 29 x86_64 system image for minSdk testing. Put `platform-tools` (adb) on PATH |
| cargo-ndk | Not installed | `cargo install cargo-ndk --locked` (pinned version) |
| GitHub CLI | Not installed | Optional: lets agents open PRs in the fork once the owner approves each one |
| Emulator acceleration | Unknown | Check Windows Hypervisor Platform (WHPX) or the Android Emulator hypervisor driver; the QA agent verifies with `emulator -accel-check` |

Every install above needs the owner's go-ahead (H-02). Downloads total roughly 5–8 GB.

---

## 4. Repository and branching

```text
github.com/<owner>/photocraft      fork of storytold/photocraft (owner creates it; H-04)
  main                             tracks upstream main (an agent syncs it daily)
  android                          integration branch for the Android work (CI: Android gates + upstream gates)
  android/<wbs-id>-<slug>          one branch and one worktree per work package, merged into android after review
local: C:\Users\COMPUMARTS\Desktop\Projects\PhotoCraft_android\
  brd\                             this BRD package (living documents, updated at every gate)
  upstream\                        the fork's clone; worktrees under upstream\.worktrees\<wbs-id>
  brd\LOG.md                       append-only delivery log: decisions, gate results, merges, open blockers
```

- **Upstream contributions.** When D-01 is "official", agents prepare small upstream PRs from the `android` branch:
  first the UC-xx changes that are useful on their own (the Android CI check, the font directories, the
  memory-detection branch, the document-origin seam), then the app crate. The owner submits them, or approves each
  `gh pr create`.
- **Credits.** Upstream `AGENTS.md` asks agents to offer the human contributor a `contributors/people.toml` entry once.
  The release agent asks the owner the first time a PR is prepared.
- **Daily sync.** An agent merges upstream `main` into the fork, rebuilds, and runs the Android check. This is how the
  port keeps up with upstream velocity (R-01).

---

## 5. Work-package brief template

Every implementer session starts from a brief like this one. The lead agent fills it from the WBS
([08](08-roadmap-wbs-estimates.md) §3) and the requirement tables.

```text
Work package: 1.6 Save, Save As and Export (SAF), safe write, Recents
Requirements: FR-FIO-05, FR-FIO-06, FR-FIO-07, FR-FIO-08, FR-FIO-11; NFR-DATA-03 (acceptance criteria: brd/02, brd/03)
Branch / worktree: android/1.6-saf-save   CARGO_TARGET_DIR=target/agent-1.6
Owns: apps/photocraft-android/src/android/documents.rs, packaging/android/.../Documents.kt
Touches (small, surgical): crates/ui-egui/src/file_dialog.rs (UC-03 origin), file_open.rs (Recents)
Depends on: 1.3 bridge (merged), 1.5 open (merged)
Tests to add: host tests for MIME/extension mapping and recents capping; instrumented SAF test against a test
  DocumentsProvider; a fault-injection test (encode error, write error, kill mid-write)
Gates: cargo test -p photocraft-ui-egui -p photocraft-android; clippy -D warnings; xtask layers; xtask wasm;
  xtask android check; emulator smoke
Done when: every acceptance criterion passes on the emulator; the reviewer agent has no open findings;
  LOG.md has an entry with the results
Report back: diff summary, test results, screenshots of the flows, open issues
```

---

## 6. Execution waves

Waves keep parallel sessions on separate files. The number of agents per wave is bounded by this PC's capacity (§3)
and by usage limits.

| Wave | Work packages run in parallel (one session each) | Merge order |
|---|---|---|
| **W0: Phase 0** | Spike: compile check plus stub app ([04](04-technical-architecture.md) §18) · Toolchain setup and emulator check | — |
| **W1: foundations** | 1.1 app crate plus 1.3 JNI bridge · 1.2 shared shell crate extraction · 1.14 packaging plus 1.15 CI | 1.2 → 1.1/1.3 → 1.14/1.15 |
| **W2: platform services** | 1.5 and 1.6 SAF open and save (sequential, one agent) · 1.9 lifecycle · 1.10 and 1.11 GPU plan and memory · 1.12 fonts, locale, theme and insets | 1.5 → 1.6; the others independent |
| **W3: remaining shell** | 1.7 folders · 1.8 share and intents · 1.13 clipboard · 1.4 storage polish | Independent |
| **Gate G1** | QA agent: G1 suite on the emulator; the owner checks on a device (H-11) | — |
| **W4: touch core** | 2.1 gesture layer (owns `canvas.rs`, `wheel_nav.rs`) · 2.4 text entry · 2.5 stylus bridge (Kotlin plus queue; its `canvas.rs` hook comes after 2.1) | 2.1 → 2.5 hook |
| **W5: touch UI** | 2.2 Touch shortcut · 2.3 density and long press (owns `theme.rs`, `press_menu.rs`) · 2.6 Touch workspace and start screen · 2.7 keyboard, mouse and DeX | 2.3 → 2.6 |
| **Gate G2** | QA agent plus the owner; usability round R2 with real users (H-11) | — |
| **W6: hardening** | 3.1 device matrix fixes · 3.2 performance · 3.4 stability · 3.6 localization · 3.7 size · 3.5 accessibility | Continuous |
| **W7: release** | 4.2 release CI · 4.3 store drafts and screenshots · 4.6 docs. The owner does H-05 to H-10 | — |
| **Gate G3** | Public v1.0 | — |
| **W8: v1.1** | 2.9 phone Compact workspace · FR-FEAT-08 printing | — |

---

## 7. Quality controls without human code review

| Control | How |
|---|---|
| Tests first | Every work package adds tests that encode its acceptance criteria before or with the code. CI must be green |
| Independent review | A reviewer agent that did not write the change reviews every merge into `android`; high-risk areas get two reviewers, one of which can be another AI tool, plus the owner's sign-off (§1) |
| Upstream gates | Unchanged: upstream tests, clippy, layers, wasm, `panic_hunt`, corpus tests where relevant |
| Visual checks | Agents render screenshots through the QA control channel and inspect them; the lead agent attaches them to the gate reports for the owner |
| Real-device truth | Emulators cannot reproduce stylus pressure and tilt, real GPU drivers or touch feel. Every gate includes the owner's device check and the QA agent's adb run on the owner's device |
| No silent scope cuts | An agent that cannot meet an acceptance criterion reports it. Only the owner can waive or defer a requirement (logged in `LOG.md`) |
| Traceability | Every PR names its requirement IDs; [10](10-traceability-matrix.md) is updated at each gate |

---

## 8. Effort, calendar and cost (AI delivery)

**Effort.** The WBS hours in [08](08-roadmap-wbs-estimates.md) §3 are agent-hours, in upstream's own calibration:

- 660–1,350 agent-hours for tablet-first v1.0;
- +80–160 for the phone layout.

Upstream estimates that about 80% of its work parallelizes. The serial parts here are the bridge, the save path and
the canvas-touching UX items.

**Calendar.** It is bound by three things more than by agent speed:

1. the owner's availability for decisions and device tests;
2. usage limits on concurrent agent sessions;
3. fixed external waits (Play account verification; the 14-day closed test for personal accounts).

Planning assumptions: 2–3 concurrent agent sessions, and the owner available about 1 hour a day plus the gate
sessions.

| Milestone | Target (start 2026-10-19) | Driven by |
|---|---|---|
| G0: feasibility | 2026-10-23 | Spike results; the owner's decisions D-01 to D-06, D-10, D-14 |
| G1: internal alpha | 2026-11-13 | W1–W3 (bridge and SAF are the critical path) |
| G2: closed beta | 2026-12-11 | W4–W5 (gestures and text entry); the owner's device checks |
| G3: public v1.0 | 2027-01-22 | W6–W7; device-matrix coverage; Play review and the 14-day test (start it at G2) |
| v1.1: phone layout | 2027-02-19 | W8 |

These dates are about 5 weeks earlier than the first draft of this BRD, which assumed two human engineers. They slip one-for-one with owner unavailability
and with device-only defects that need several round trips.

**Cost.**

| Item | Cost |
|---|---|
| Engineering salaries | None |
| AI usage | Many hundreds of agent-hours. Heavy multi-session use needs a plan with high limits or API budget. The lead agent reports usage at every gate (`/usage`), so the spend tracks the WBS |
| Devices | At least the owner's own Android device. Strongly recommended: one stylus tablet with an Adreno or Mali GPU (≈ US$300–900). A device farm is optional for rare GPUs |
| Accounts | Google Play registration US$25 (one-time); Android developer verification per Google's current terms |
| CI | GitHub Actions on a public fork: free standard runners |

---

## 9. Reporting cadence

- **Per merge:** an entry in `brd/LOG.md` with the work package, requirement IDs, results and open items.
- **Daily (while agents run):** a short status for the owner: what landed, what is blocked, what needs an owner
  action (H-xx).
- **Per gate:** a gate report with the checklist from [07](07-test-qa-plan.md) §6, screenshots, device results, usage
  so far, and the decisions requested.

---

## 10. D-01 decided: an independent build (2026-10-13)

The owner chose **their own build**, not an official ArtCraft release. That overrides the earlier documents wherever
they assume the official path:

| Topic | Earlier assumption | Independent build |
|---|---|---|
| Application ID | `ai.storyteller.photocraft` | The owner's own reverse-domain ID, for example `io.github.<github-user>.<appname>` (D-15). It must never use the `ai.storyteller` namespace |
| App name and listing | "PhotoCraft" | A distinct name (D-15). The listing may say in plain text "based on PhotoCraft by the ArtCraft team" (brand licence); it must not suggest it is the official app (Play's impersonation policy) |
| ArtCraft marks | Kept in official builds | **Removed.** A build flag replaces the brand mark that `crates/ui-egui/src/brand.rs` embeds and removes the ArtCraft logos from About and the splash (UC-16, **done 2026-10-14**: the `unbranded` feature of `photocraft-ui-egui`, on for the Android target; see [04](04-technical-architecture.md) UC-16). The kitsune app icon is MIT/Apache and may stay, but a distinct icon variant is recommended so users can tell the builds apart |
| Code home | Upstream `storytold/photocraft` | The owner's fork. Android code stays isolated (new crates and folders plus small, well-marked UC patches) so that the daily upstream merge stays cheap. Generic fixes (fonts, memory detection, Android CI check) may be offered upstream as PRs at the owner's discretion |
| Releases | Shared `craft-common-actions` workflow; getartcraft.com | The fork's own `android-release.yml`; APKs on the fork's GitHub Releases (D-11 = local workflow); no getartcraft.com |
| Accounts and keys | ArtCraft's Play account and signing | The owner's Play account (most likely personal, so the 12-tester, 14-day closed test applies), the owner's signing key, the owner's developer verification |
| Licences | — | MIT OR Apache-2.0 allows this. Keep NOTICE, ATTRIBUTION and the licence texts; state the changes made (Apache-2.0 §4) |
| Upstream velocity risk (R-01) | Upstream review speed | **Merge burden**: an agent merges upstream daily and fixes conflicts; the Android CI gate catches breakage the same day |

New work package: **UC-16, brand stripping for non-official builds** (about 4–8 agent-hours, Phase 1).

---

## 11. Next steps

1. ~~The owner answers Q-01 and approves the installs (H-01, H-02).~~ Done: independent build; installs approved;
   the owner has a phone and a tablet.
2. W0: toolchain setup plus the Phase 0 spike, **in progress**. Results go to `brd/12-phase0-spike-report.md`.
3. The owner picks an app name and ID (D-15) before the first public APK.
4. G0 review with the owner, then W1 starts.
