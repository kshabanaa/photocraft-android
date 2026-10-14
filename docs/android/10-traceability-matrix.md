# 10: Requirements traceability matrix

| | |
|---|---|
| **Version** | 1.0 draft, 2026-10-13 |
| **Purpose** | Shows that every business requirement (BR) is realized by functional and non-functional requirements, verified by tests and delivered in a planned phase |
| **Keys** | Test references point to [07](07-test-qa-plan.md) sections; WBS references point to [08](08-roadmap-wbs-estimates.md) §3 |

---

## 1. Business requirements to delivery

| BR | Business requirement (short) | Realized by (FR / NFR / UC) | Verified by ([07](07-test-qa-plan.md)) | WBS | Gate |
|---|---|---|---|---|---|
| BR-01 | Signed APKs (arm64, x86_64, universal) in every release | FR-UPD-02; NFR-COMP-02; NFR-SEC-08; NFR-SIZE-01; UC-12, UC-14 | §2 L3; §3.7 policy suite; release checklist §5 (items 1 and 9) | 1.14, 4.1, 4.2 | G2 (beta APK), G3 |
| BR-02 | Google Play AAB that complies with policy | NFR-COMP-01, 02; NFR-COMPL-01 to 07; NFR-SEC-09 | §3.7; Play pre-launch report | 4.2, 4.3, 4.4 | G3 |
| BR-03 | Same engine and command registry; documented exceptions only | FR-FEAT-01, 02, 03; UC-11 | §3.1 parity report | 1.2, 2.x, 3.x | G3 |
| BR-04 | Files compatible with desktop | FR-FIO-01, 05, 06, 07, 13; NFR-DATA-04; UC-03 | §3.3 golden outputs; §3.4 provider matrix | 1.5, 1.6 | G1 |
| BR-05 | Real work by touch and stylus on tablets; usable on phones | FR-INP-01 to 11; FR-UI-01 to 10; UC-07 to 10 | §3.5 input suite; usability rounds R1–R4 ([05](05-ux-touch-design.md) §16) | 2.1 to 2.9 | G2 (tablet), v1.1 (phone) |
| BR-06 | Never lose work | FR-LC-03, 04, 05; FR-FIO-07; NFR-DATA-01 to 03 | §3.2 lifecycle torture suite | 1.6, 1.9 | G1 |
| BR-07 | Offline and private (no network, no data collected) | NFR-SEC-01, 02, 04, 05, 09; NFR-OBS-04; FR-QA-01 (QA only); D-10 | §3.7 manifest audit; no-network test | 1.14, 4.3 | G1 (manifest), G3 (store) |
| BR-08 | Stability and performance targets | NFR-STAB-01 to 07; NFR-PERF-01 to 11; NFR-EN-01; UC-05 | §2 L7 performance; §3.6 GPU suite; Play vitals in the beta | 1.10, 1.11, 3.1 to 3.4 | G3 |
| BR-09 | All languages; system and per-app language | FR-L10N-01 to 04; NFR-L10N-01 to 03 | §3.8 localization pass | 3.6 | G3 |
| BR-10 | Licence, brand and trademark compliance | FR-HLP-02; NFR-COMPL-06, 09, 10; [06](06-build-release-distribution.md) §7 | Legal sign-off; licences screen check | 4.3 | G3 |
| BR-11 | Healthy upstream: CI gate, layering, isolated `unsafe` | NFR-MAINT-01 to 06; NFR-SEC-06; UC-01, 02, 12, 13 | §2 L1 to L3 on every PR | 0.2, 1.1, 1.2, 1.15 | G1 |
| BR-12 | Automated Android release builds | NFR-MAINT-04, 06; NFR-OBS-03; UC-14 | Release dry run (`workflow_dispatch`) | 4.2 | G2 |
| BR-13 | Support and diagnostics without telemetry | FR-HLP-04, 05; NFR-OBS-01 to 03; FR-LC-05 | §3.2 native-crash case; diagnostics zip check | 1.9, 3.4 | G2 |
| BR-14 | Accessibility baseline and roadmap | NFR-A11Y-01 to 05; FR-UI-01, 10 | §3.8 touch-target probe, contrast, keyboard | 2.3, 3.5 | G3 |
| BR-15 | Reusable shell for the other Crafting Apps | [04](04-technical-architecture.md) §17; UC-02; D-13 | Architecture review | 1.2, post-v1 | After v1.0 |

---

## 2. Functional requirements to tests and phases

| FR group | Requirements | Main tests | Phase |
|---|---|---|---|
| Lifecycle | FR-LC-01 to 11 | §3.2; §5 launch checks; back-navigation script | 1 (01–09), 2 (08 polish), 1 (11) |
| Files | FR-FIO-01 to 15 | §3.4 provider matrix; §3.3 golden outputs; §3.7 hostile names | 1 |
| Features | FR-FEAT-01 to 09 | §3.1 parity report; format samples | 1 (01–03), 3 (05–06), post-v1 (04, 07, 08) |
| Input | FR-INP-01 to 12 | §3.5 input suite; IME matrix | 2 |
| UI | FR-UI-01 to 10 | Usability R1–R4; layout probes; dialog inventory | 2 (Compact in 2.9 / v1.1) |
| Settings | FR-SET-01 to 05 | Persistence and backup tests | 1 |
| Text | FR-TXT-01 to 04 | Font listing; CJK rendering; PSD substitution | 1 |
| Clipboard | FR-CLP-01, 02 | Cross-app copy and paste | 1 |
| Localization | FR-L10N-01 to 04 | §3.8 | 3 |
| Help | FR-HLP-01 to 05 | About, licences, diagnostics checks | 1 (01–03), 3 (04–05) |
| Updates | FR-UPD-01, 02 | Update-over-install; certificate check | 4 |
| QA hooks | FR-QA-01, 02 | CI permission audit; smoke test | 1 |

---

## 3. Non-functional requirements to verification

| NFR group | Verification method | Owner | Cadence |
|---|---|---|---|
| NFR-COMP | Device matrix; Gradle configuration; alignment checks; GMS-free emulator | QA agent; owner on devices | Each gate and release |
| NFR-PERF | Control-channel timing; Perfetto; `am start -W` | QA | Weekly from Phase 3; each RC |
| NFR-STAB | Play vitals; exit reasons; fault injection; `panic_hunt` on devices | QA agent and implementer agents | Continuous during the beta |
| NFR-DATA | Lifecycle torture suite; golden outputs | QA | Each gate and RC |
| NFR-SIZE | CI size report | Implementer agents | Every build |
| NFR-SEC | Manifest audit; intent fuzzing; review | Implementer, reviewer and release agents | Every release |
| NFR-A11Y | Touch-target probe; contrast audit; keyboard pass | QA and UX | G2, G3 |
| NFR-L10N | Screenshot pass; pseudo-locale | QA | G3 |
| NFR-MAINT | CI gates; docs review; line-count check on Kotlin | Maintainers | Every PR |
| NFR-OBS | Diagnostics export; symbolication test | Implementer agents | G2 |
| NFR-EN | Idle-repaint check; background audit | QA | G3 |
| NFR-COMPL | Policy checklist ([03](03-non-functional-requirements.md) §12) | Release manager | Every release; yearly SDK bump |

---

## 4. Upstream changes to requirements

| UC | Enables |
|---|---|
| UC-01 (app crate) | BR-01, BR-11; everything else |
| UC-02 (shared shell crate) | BR-11, BR-15; FR-FIO, FR-LC, NFR-STAB-03, 04 |
| UC-03 (document origin) | FR-FIO-05, 06, 10, 11, 14; BR-04 |
| UC-04 (new `Services` fields) | FR-UI-02, FR-INP-08, 11, FR-FIO-09, NFR-STAB-05 |
| UC-05 (memory on Android) | NFR-STAB-05, FR-FIO-04 |
| UC-06 (fonts) | FR-TXT-01, 02 |
| UC-07 (stylus samples) | FR-INP-06, 07, 08 |
| UC-08 (gesture layer) | FR-INP-02, 03, 04 |
| UC-09 (touch density, long press, Touch shortcut) | FR-UI-01, 05; FR-INP-04, 05; NFR-A11Y-01 |
| UC-10 (workspaces) | FR-UI-03, 04 |
| UC-11 (platform exceptions) | FR-FEAT-02; BR-03 |
| UC-12 (build, CI, docs) | NFR-MAINT-01, 05, 06; NFR-COMP-02 |
| UC-13 (`unsafe` policy) | NFR-SEC-06; BR-11 |
| UC-14 (release job) | BR-01, BR-02, BR-12 |
| UC-15 (accesskit gating, if needed) | Build feasibility (A-03) |
