# Android port: handoff to cloud sessions (2026-10-15)

The owner (`kshabanaa`) wants the remaining work done in Claude Code **cloud sessions**. This file says
where things stand and what each session should do. Work on the fork `kshabanaa/photocraft-android`.
Start from the branch **`android-phase0-sizing`**, which holds everything done so far.

## Read first

- `AGENTS.md`: the project rules. Never panic. No `unsafe` outside
  `apps/photocraft-android/src/bridge.rs`. Never break wasm. Respect layering. Every change comes with
  tests.
- `docs/android/README.md`, then `11-ai-delivery-plan.md` (§5 is the work-package template, §6 the
  waves), and `12-phase0-spike-report.md` (§7 lists every fix so far).
- `docs/android/09-risks-decisions-questions.md`: the owner's decisions.
  - D-01: an independent build.
  - D-02: phones and tablets, **landscape-first**.
  - D-03: the glue code is **Java** (`packaging/android/.../PhotoCraftActivity.java`).
  - D-04 to D-06, D-10: GameActivity; minSdk 29, targetSdk 36; arm64-v8a and x86_64; no INTERNET
    permission.

## Where things stand

- Phase 0 spike: the app runs on a Galaxy S20+ (Android 13, Vulkan) and on the Android 16 emulator.
  Working so far:
  - insets;
  - auto UI scale;
  - dialogs that scroll inside the screen;
  - soft-keyboard text and a number pad, with Enter;
  - modifier keys;
  - swipes that scroll while drags stay with the control;
  - Back sends the app to the background;
  - landscape lock.
- **Responsive layout:** there is no CSS in egui, so PhotoCraft uses a *size class*. It is
  `photocraft_ui_egui::touch::small_screen(ctx)`: content height under 600 pt or width under 760 pt.
  `touch::short_screen` checks the height only.
  - Phone layout rules key on the size class, not on the platform. A phone, a small desktop window and
    the web build on a phone get the same layout.
  - Only the UI-scale rule (`prefs_ui::sync_display_scale`) stays Android-only.
  - Floating panels: `touch::FitScreen::fit_screen` (an `egui::Window` that scrolls on short screens).
  - 2D controls: `touch::pad`.
- **Layout sweep tool:** `crates/ui-egui/examples/layout_sweep.rs`.
  - It opens every dialog ("…" menu items), every Window menu item and every tool, offscreen, at a
    given size.
  - It flags any floating window, dialog or popup that leaves the usable screen, writes `report.md`,
    and saves a PNG per item.
  - Phone run:
    ```sh
    cargo run --release -p photocraft-ui-egui --example layout_sweep -- \
        --out sweep/phone --size 1120x520 --scale 2.077 --insets 0,33,65,0
    ```
    It needs a wgpu adapter. On a headless Linux machine, install software Vulkan first:
    `apt-get install -y mesa-vulkan-drivers`.
  - `--only <prefix>` limits the run. `--theme studio` runs it in another theme.
  - The full run is 324 items and takes about 30 minutes, because filter dialogs render previews.
  - The 2026-10-15 run was stopped after 136 items (File, Edit, Image and part of Filter). It found
    **no overflow**, once the Studio-theme floating Properties card was made to scroll (fixed in
    `panels.rs`).
  - The screenshots showed these dialogs fitting and readable: New Document, Save for Web, Fill,
    Stroke, Preset Manager, Color Settings and Print Settings.

## Work for cloud sessions

Run them in this order. Each one pushes **only its own new branch**: never `main`, never a force
push, never deleting a branch. Report back with the diff summary, the gate results, and a list of
what needs checking on a real device. The owner tests on the phone and tablet from a local session.

### Session A: finish the phone-layout sweep and fix what it finds

Branch: `android-phase0-sweep`, from `android-phase0-sizing`.

1. Run the full phone sweep, then a second pass with `--theme studio`.
2. For every item the report flags, and for anything wrong in the PNGs, fix it in `crates/ui-egui`
   with the size-class helpers, never `cfg!(target_os)`. Things to look for in the PNGs:
   - text or controls cut off inside a dialog;
   - an options bar or tab row clipped at the right edge;
   - a popup or menu taller than the screen.
3. Floating containers made with `egui::Area`, which `FitScreen` does not cover, need their own
   height limit, like the Properties card in `panels.rs`.
4. Re-run the sweep until it reports 0 overflow.
5. Commit `sweep/phone/report.md` to `docs/android/sweeps/` (the report only, not the PNGs).
6. Then do the same at tablet sizes:
   - `--size 1280x800 --scale 2`, a 10" tablet in landscape;
   - `--size 1180x820 --scale 2`.
7. Gates: run the "Gates" section below before every push.

### Session B: Phase 1 foundations, then file open and save

Branch: `android-phase1`, from `android-phase0-sizing`. If session A has pushed by then, merge its
branch first.

1. **Cherry-pick these three commits** (each of their branches is `cadbd51b`, a superseded snapshot,
   plus one real commit):
   - `fb2039cb`: memory and fonts (WP 1.11/1.12);
   - `b4907624`: the Android CI gate (WP 1.15);
   - `389b20f9`: the `unbranded` build (UC-16).
   Resolve conflicts in favour of `android-phase0-sizing`.
2. **WP 1.5/1.6: open and save through the Storage Access Framework.** Today File › Open reports
   "cancelled" on Android.
   - Requirements: FR-FIO-05 to FR-FIO-08 and FR-FIO-11 (`02`). Design: `04` (Services seam, SAF,
     JNI bridge).
   - Java: ACTION_OPEN_DOCUMENT and ACTION_CREATE_DOCUMENT with activity results, in
     `PhotoCraftActivity`.
   - Rust: the JNI calls in `bridge.rs`. Bytes cross the bridge, never paths.
   - Tests:
     - host tests for MIME/extension mapping, recents capping and error paths;
     - a fault-injection test: an encode error, a write error and a kill mid-write must never lose the
       document.

## Gates (before every push)

```sh
cargo test -p photocraft-ui-egui --lib
cargo test -p photocraft-android --lib
cargo clippy -p photocraft-ui-egui -p photocraft-android --all-targets -- -D warnings
cargo xtask layers
cargo xtask wasm
```

If the Android NDK (r29) and `cargo-ndk` can be installed, also build for Android. Use the draft
settings (env vars only; never edit the Cargo.toml profiles):

```sh
CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=256 CARGO_PROFILE_RELEASE_INCREMENTAL=true \
  cargo ndk -t arm64-v8a -P 29 build --release -p photocraft-android --features heif
```

## Not for cloud sessions

These stay with the owner:

- device testing on the phone (SM-G985F) and the tablet;
- size and start-up measurements with `FULL=1 packaging/android/deploy.sh`;
- the G0 review;
- any account, signing key or store submission.
