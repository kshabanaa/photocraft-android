# 09: Risks, decisions and open questions

| | |
|---|---|
| **Version** | 1.0 draft, 2026-10-13 |
| **Review** | At every gate, and weekly during Phases 2–4 |
| **Scoring** | Likelihood (L) and impact (I) on a 1–5 scale; score = L × I. **≥ 15** red, **8–14** amber, **≤ 7** green |

---

## 1. Risk register

| ID | Risk | L | I | Score | Mitigation | Owner | Early warning |
|---|---|---|---|---|---|---|---|
| R-01 | Upstream does not accept Android as a target, or reviews too slowly; at upstream velocity a fork drifts within days | 3 | 5 | **15** | RFC issue and Discord alignment in Phase 0; small, well-tested PRs; propose the "never break Android" CI gate early; offer reuse to the other Crafting Apps (BO-5). Fallback: an out-of-tree app crate plus a small patch set, rebased daily by CI | Sponsor, tech lead | No maintainer response within 1 week; PRs open more than 5 days |
| R-02 | winit 0.30 Android input gaps (IME composition, stylus tilt and tool type, batched samples, hover) are worse than expected | 4 | 4 | **16** | Kotlin bridge for stylus and text (native text sheet); measure in the Phase 0 spike; follow upstream egui and winit upgrades (the new pointer API in winit's next release) | Tech lead | Spike shows IME failures on Gboard or Samsung Keyboard |
| R-03 | Mobile GPU drivers or wgpu backends crash, mis-render or lack the needed formats (float render targets) | 4 | 4 | **16** | Fallback chain (Vulkan → GLES → CPU canvas) with crash marker; deny-list; GPU-vs-CPU oracle tests on Adreno, Mali, Xclipse and PowerVR; Safe GPU shortcut | Tech lead, QA | Oracle mismatches; device-lost errors in the beta |
| R-04 | Memory pressure and low-memory kills with large documents on 4–6 GB devices | 4 | 4 | **16** | RAM-tier budgets; open-time size guard; trim-memory purges; autosave on pause; clear refusal messages; tune on the BD device | Engineers | Low-memory exit reasons in beta telemetry from diagnostics exports |
| R-05 | The dense desktop UI is not usable enough by touch, especially on phones | 4 | 4 | **16** | Tablet-first scope (D-02); Touch density and workspace; the Touch shortcut; usability rounds R1–R4; phone layout in v1.1 | Product owner, UX | R2 success below 5 of 5; beta feedback |
| R-06 | Performance on mid and low-end devices is unacceptable (upstream meets only 3 of 25 desktop budgets) | 4 | 3 | 12 | Mobile budgets; profile early (Phase 2); prefer GPU paths; minimum 4 GB RAM; reduce defaults on low tiers (history states, cache sizes) | Engineers | BD frame time above 50 ms during strokes |
| R-07 | Data loss through SAF write semantics (truncate-on-open) or process death during a save | 3 | 5 | **15** | Encode before touching the target; keep a temp copy and the recovery entry until verified; lifecycle torture suite; provider matrix | Engineers, QA | Any S1 data-loss report |
| R-08 | Play rejects the app, or a third party complains about trademarks ("Photoshop" in the listing) | 2 | 4 | 8 | Listing rules ([06](06-build-release-distribution.md) §7); legal review; trademark disclaimer; no Adobe marks | Release manager, legal | Review feedback |
| R-09 | Policy changes: yearly target-SDK deadline; developer verification goes global in 2027; new Play requirements | 4 | 3 | 12 | Compliance calendar (Q2 target-SDK bump task each year); register for verification in Phase 4; subscribe to Play policy updates | Release manager | Play Console policy notices |
| R-10 | Signing key lost or compromised | 1 | 5 | 5 | Play App Signing; offline backups in two places; secrets limited to the `release` environment; rehearsed rotation | Release manager | — |
| R-11 | JNI or `unsafe` defects cause crashes or memory corruption | 3 | 4 | 12 | One isolated module; `SAFETY:` comments; exception checks; `-Xcheck:jni` in QA; tests; minimal surface; mandatory review | Tech lead | Native crashes in `libphotocraft_android.so` JNI frames |
| R-12 | APK too large (> 100 MB per ABI) | 3 | 3 | 9 | Per-ABI splits; size profile; ship symbols separately; font decision D-09; size gate in CI | Engineers | Phase 0 measurement |
| R-13 | Maintainers reject the Kotlin glue as contrary to the "Rust only" principle | 3 | 4 | 12 | Keep it ≤ about 1,500 lines with no logic; Java as an alternative; documented rationale (no Java-free way to receive activity results); decision D-03 at G0 | Sponsor, tech lead | RFC discussion |
| R-14 | Brand or legal issues if distributed as a fork (D-01 = fork) | 2 | 4 | 8 | Remove the ArtCraft marks; own name and application ID; state "based on PhotoCraft by the ArtCraft team" in plain text, as the brand licence allows | Legal | — |
| R-15 | The screen-reader gap limits adoption (education buyers) or draws criticism | 3 | 3 | 9 | Disclose; publish the accessibility roadmap; contribute AccessKit Android integration upstream to egui | Product owner | Procurement questions (P2) |
| R-16 | An upstream dependency upgrade (eframe, egui, winit, wgpu) breaks the Android build or behaviour | 3 | 3 | 9 | The CI Android gate on every PR; emulator smoke; an upgrade checklist in `docs/android.md` | Engineers | A red Android CI job on an upgrade PR |
| R-17 | Device fragmentation costs more test time than planned | 3 | 3 | 9 | Tiered matrix; device-farm bursts; beta testers with diverse devices (Discord) | QA | Beta device spread |
| R-18 | The 14-day, 12-tester closed test (personal account) or store review delays the launch | 3 | 2 | 6 | Organization account (D-12); start the closed test at G2 | Release manager | Account type |
| R-19 | Sideload friction from developer verification (now in BR, ID, SG and TH; global in 2027) | 4 | 3 | 12 | Register the developer, package and certificate before the first public APK; document install steps | Release manager | Users reporting blocked installs |
| R-20 | Missing system fonts change PSD text appearance on Android | 3 | 2 | 6 | `SystemFonts` API; user font import; substitution warnings (FR-TXT-04); bundled craft-fonts where the size budget allows | Engineers | Font-substitution reports |
| R-21 | Upstream's Android-relevant code paths (`std::process::Command`, desktop config directories, `/tmp` assumptions) are more widespread than the spike shows | 3 | 3 | 9 | A grep audit in Phase 0; `TMPDIR` set from Kotlin; the "never break Android" gate prevents new cases | Engineers | New `cfg(target_os = "linux")` assumptions in PRs |
| R-22 | Agent-introduced defects: plausible but wrong code, invented or outdated APIs (JNI, AndroidX, winit and eframe Android details) | 4 | 4 | **16** | Tests written from the acceptance criteria; every claim about an API checked against the pinned crate source or the official docs; compile and emulator gates on every merge; independent reviewer agents; a second review for high-risk areas | Lead agent | Review findings; flaky or failing smoke tests |
| R-23 | The owner becomes the bottleneck (decisions, device checks, account tasks) | 3 | 4 | 12 | Requests batched into gate reports; recommended defaults with a deadline for each decision; owner tasks scheduled ahead of need (H-05 and H-06 start in Phase 2) | Lead agent and owner | Agent sessions idle while waiting on the owner |
| R-24 | Only one physical device is available, so GPU-family and form-factor coverage is narrow | 4 | 3 | 12 | Beta testers with diverse devices; an optional device farm before G3; conservative GPU fallback; deny-list updates from diagnostics exports | QA agent and owner | Device-specific beta reports |
| R-25 | AI usage limits or cost slow the waves | 3 | 3 | 9 | Measure usage per work package in Phase 0; keep the critical path (bridge, SAF, lifecycle, text entry) first; size waves to the plan's limits | Lead agent | Sessions throttled; usage reports at gates |
| R-26 | Parallel agents collide: shared files, the Cargo build lock, CPU and disk contention on one PC | 3 | 3 | 9 | File ownership per wave ([11](11-ai-delivery-plan.md) §6); a separate `CARGO_TARGET_DIR` per agent; at most 3 heavy builds at once; clean up target directories | Lead agent | Merge conflicts; build times > 2× baseline |
| R-27 | Upstream maintainers push back on a large volume of AI-written PRs from an outside contributor | 2 | 3 | 6 | Upstream is itself agent-built; keep PRs small with tests; follow `AGENTS.md` exactly; the owner submits and discusses | Owner | Review comments |

**Owner column mapping (D-14):**

| Role used above | Means |
|---|---|
| Sponsor, product owner, legal | The **owner** |
| Tech lead | The **lead agent** |
| Engineers | **Implementer agents** |
| QA | The **QA agent** |
| Release manager | The **release agent**, with the **owner** for accounts and keys |

**Top risks to manage weekly:**

- Score 16: R-02, R-03, R-04, R-05, R-22.
- Score 15: R-01, R-07.
- For a solo owner with a personal Play account, R-18 (the 12-tester closed test) rises to L4 (score 8).

---

## 2. Decision log

| ID | Decision | Options | Recommendation and rationale | Owner | Due | Status |
|---|---|---|---|---|---|---|
| D-01 | Who publishes and how it is delivered | (a) Official upstream target published by the ArtCraft team; (b) independent fork with its own name and ID | The owner chose **(b) independent build**. Consequences and overrides: [11](11-ai-delivery-plan.md) §10 (own ID and name, no ArtCraft marks, own releases, daily upstream merge) | Owner | G0 | **Decided 2026-10-13** |
| D-15 | App name and application ID | Owner's choice | A distinct name that does not imply it is the official app; ID in the owner's namespace | Owner | Before the first public APK | Open |
| D-02 | Form-factor scope of v1.0 | (a) Tablets, Chromebooks and desktop mode first, phones basic; (b) phones fully optimized in v1.0 (+4–6 weeks); (c) tablets only (exclude phones on Play) | The owner chose **phones and tablets, in landscape**, for v1.0 (2026-10-15), replacing the tablet-first recommendation; [FR-UI-09](02-functional-requirements.md) makes landscape the primary orientation | Product owner | G0 | **Decided 2026-10-15** |
| D-03 | Platform glue language | (a) Thin Kotlin in `packaging/android`; (b) thin Java; (c) Rust-only JNI with NativeActivity | The owner left it to the lead agent, who chose **(b) thin Java** (2026-10-15): the spike activity already works in Java, it needs no Kotlin toolchain, and it uses the same AndroidX APIs | Maintainers | G0 | **Decided 2026-10-15** |
| D-04 | Activity type | GameActivity or NativeActivity | **GameActivity** (IME through GameTextInput; AndroidX base class) — **accepted by the owner (2026-10-15)** | Tech lead | G0 | **Decided 2026-10-15** |
| D-05 | Placement of `unsafe` | (a) Isolated module in the app crate (`deny` crate-wide, `allow` in `bridge/ffi.rs`); (b) a separate `photocraft-android-sys` crate | **(a)**: the smallest surface; mirrors the `crates/tablet` precedent; easy to audit — **accepted by the owner (2026-10-15)** | Maintainers | G0 | **Decided 2026-10-15** |
| D-06 | SDK levels and ABIs | minSdk 26, 28 or 29; ABIs with or without 32-bit | **minSdk 29, targetSdk 36, `arm64-v8a` and `x86_64`**: Vulkan 1.1 is mandatory on 64-bit devices launching with Android 10; scoped storage baseline; memory-heavy workloads — **accepted by the owner (2026-10-15)** | Tech lead | G0 | **Decided 2026-10-15** |
| D-07 | Distribution channels and order | GitHub, website, Play, F-Droid, Galaxy Store | **GitHub and website APK at G2 (beta) and G3; Play at G3; F-Droid and Galaxy Store after v1.0** | Product owner | G1 | Open |
| D-08 | Signing strategy | (a) One app signing key for GitHub and Play (own key uploaded to Play App Signing); (b) a Google-generated Play key, with separate channels | **(a)**: users can move between channels without losing data | Release manager | G1 (before the first public APK) | Open |
| D-09 | Bundled fonts (craft-fonts Japanese) | Bundle in the APK; none (system CJK); on-demand download | **Bundle if the APK stays within the size budget after the Phase 0 measurement; otherwise use system CJK.** On-demand download conflicts with the no-network principle | Product owner | G1 | Open |
| D-10 | Network and telemetry | (a) No `INTERNET`, no telemetry; (b) opt-in crash reports | **(a)**: the product promise; F-Droid friendly; Play vitals is enough. Diagnostics export covers support — **accepted by the owner (2026-10-15)** | Sponsor | G0 | **Decided 2026-10-15** |
| D-11 | Where the release pipeline lives | (a) Shared `craft-common-actions` job; (b) local `android-release.yml` | **(b) for v1.0, then (a)** once other apps adopt Android | Release manager | G1 | Open |
| D-12 | Play developer account type | Organization or personal | **Organization** (exempt from the 12-tester, 14-day rule; organization identity). Needs a D-U-N-S number | Sponsor | G1 | Open |
| D-13 | Reusable Android shell for the other Crafting Apps | Now, later or never | **Later (after v1.0)**, but design for extraction now ([04](04-technical-architecture.md) §17) | Sponsor | G2 | Open |
| D-14 | Delivery model | Human engineers; AI agents; a mix | **AI agents build everything**; the owner keeps decisions, accounts, keys, money and device testing ([11](11-ai-delivery-plan.md)) | Owner | — | **Decided 2026-10-13** |

---

## 3. Open questions

| ID | Question | Why it matters | Linked to |
|---|---|---|---|
| Q-01 | Is this plan for the ArtCraft and PhotoCraft team (official build) or for an independent build? | Brand rules, application ID, accounts, signing and upstream strategy | D-01, R-14 |
| Q-02 | Have the upstream maintainers been consulted? Who reviews Android PRs? | Feasibility of the upstream-first approach | R-01, A-01 |
| Q-03 | Who is the primary audience for v1.0: tablet artists, Chromebook education or phone users? | Scope and UX priorities | D-02 |
| Q-04 | Is Google Play required at launch, or is the GitHub APK enough at first? | Phase 4 scope and timeline | D-07 |
| Q-05 | Which Android device(s) does the owner have (model, RAM, stylus)? Is there a budget for a stylus tablet or a device farm? | Device matrix, G0 spike | R-17, R-24 |
| Q-06 | Which Claude plan or usage budget is available, and how many concurrent agent sessions does it allow? | Wave size and calendar | A-08, R-25 |
| Q-15 | May agents create a GitHub fork and branches under the owner's account (the owner approves each push or PR)? | Repository setup ([11](11-ai-delivery-plan.md) §4) | H-04 |
| Q-16 | Approve installing the NDK, cmdline-tools, Rust Android targets and cargo-ndk on this PC (about 5–8 GB)? | Phase 0 start | H-02 |
| Q-07 | Is an organization Play account available (the legal entity behind the existing macOS Developer ID), with a D-U-N-S number? | Closed-test rule, listing ownership | D-12, R-18 |
| Q-08 | Are there launch-date constraints (events, marketing) and adoption targets? | Gates and KPI targets | KPIs |
| Q-09 | Should Android builds ship in every release from G2 as a labelled pre-release, or only from v1.0? | Release cadence and support load | D-07 |
| Q-10 | Are there accessibility requirements, such as education procurement needing screen-reader support? | Priority of the TalkBack work | R-15 |
| Q-11 | Confirm: free, no ads, no in-app purchases? | Listing and policy declarations | A-05 |
| Q-12 | Confirm the stance of no telemetry beyond Play vitals? | Observability design | D-10 |
| Q-13 | Do phone-native formats (HEIC, DNG, Ultra HDR JPEG, AVIF) have priority for v1.0? | FR-FEAT-03 and 04 scope | FR-FEAT |
| Q-14 | Which languages should the store listing and release notes cover? | Phase 4 effort | FR-L10N |

---

## 4. Assumptions to validate

The assumptions are listed in [01](01-business-requirements.md) §11. Validation plan:

| Assumption | Validated by | When | If false |
|---|---|---|---|
| A-01 Upstream acceptance | RFC response | Phase 0 | Use the D-01 (b) fallback, with the fork-maintenance budget added to the estimates |
| A-02 eframe and winit Android backend adequate | Spike on devices | Phase 0 | Larger Kotlin bridge (input surface); or contribute fixes to winit and egui (+ time) |
| A-03 Pure-Rust graph compiles | `cargo check` and `cargo ndk build` | Phase 0 | Patch, replace or gate dependencies (UC additions) |
| A-04 Kotlin acceptable | D-03 | G0 | Java (same design) |
| A-08 Staffing (AI agents, 2–3 sessions) | Usage measured in Phase 0 | Phase 0 | Re-plan with the Lean or Fast scenario ([08](08-roadmap-wbs-estimates.md) §4) |
| A-10 Organization account | Account check | Phase 0 | Personal account plus a closed test started at G2 |
