# 12: Phase 0 spike report (in progress)

| | |
|---|---|
| **Started** | 2026-10-13 |
| **Lead** | Lead agent (Claude Code), per [11](11-ai-delivery-plan.md) |
| **Upstream** | `storytold/photocraft` `main` at `74cb2935` (2026-10-13, version 0.8.2), cloned to `upstream/`, local branch `android/phase0-spike` (not pushed) |
| **Status** | Compile feasibility: **passed**. Packaging: **passed** (16 KB-aligned, ≈ 25 MB compressed per ABI). On-device: **runs** on the owner's Galaxy S20+ (Vulkan, Mali-G77) and on the Android 16 emulator (OpenGL ES). Blockers found: keyboard and text input, safe areas, oversized dialogs, phone layout. Remaining: tablet run, a painting and pinch test, pause and resume cycles |

---

## 1. Environment (this PC)

| Item | Version or state |
|---|---|
| OS and hardware | Windows 11 Pro 26200, Ryzen 7 3700X (16 threads), 32 GB RAM, about 1.1 TB free |
| Rust | 1.99.0, targets `aarch64-linux-android` and `x86_64-linux-android` added |
| C/C++ for host build scripts | Visual Studio 2019 Build Tools |
| cargo-ndk | 4.1.2 |
| Android NDK | r29 (`29.0.14206865`, stable channel), installed with sdkmanager |
| Android SDK | Platforms 34 and 36, build-tools 34.0.0 and 36.1.0, cmdline-tools 23.0 (newly installed), emulator, platform-tools |
| JDK | Android Studio's bundled JBR 21.0.8 |
| Gradle and AGP | Gradle 9.8.1 (wrapper generated); AGP 9.4.1 |
| Emulator | WHPX acceleration usable; AVD `Medium_Phone_API_36.1` (x86_64) |

---

## 2. Results

| Check | Command | Result |
|---|---|---|
| Core crates for Android | `cargo check --locked --target aarch64-linux-android -p photocraft-engine -p photocraft-io -p photocraft-format -p photocraft-gpu -p photocraft-text` | **Pass**: 0 errors, 0 warnings, 1 min 50 s (cold) |
| UI crate alone | `cargo check --target aarch64-linux-android -p photocraft-ui-egui` | Fails as expected: android-activity requires the app to choose `game-activity` or `native-activity`. This is a feature selection, not a code defect |
| Stub app crate (`apps/photocraft-android`, eframe `wgpu` + `android-game-activity`) | `cargo ndk -t arm64-v8a -P 29 check -p photocraft-android` | **Pass**: the whole graph (ui-egui, eframe, egui-winit, winit, wgpu, accesskit, engine, all codecs) compiles for Android with 0 errors and 0 warnings in 1 min 01 s |
| Release `.so` (arm64, `heif` on) | `cargo ndk -t arm64-v8a -P 29 build --release -p photocraft-android --features heif` | **Pass**. Size: 63.6 MB raw, 50.2 MB stripped, ≈ 25.3 MB gzip-compressed, so a per-ABI APK download of about 25–30 MB (target ≤ 60 MB, NFR-SIZE-01). `NEEDED`: only `liblog`, `libandroid`, `libdl`, `libm`, `libc` (no `libc++_shared`). Every `LOAD` segment is aligned to `0x4000`, so it is **16 KB-page ready** with NDK r29 defaults, without extra linker flags |
| APK packaging | Gradle 9.8.1, AGP 9.4.1, `androidx.games:games-activity:4.4.0`, `appcompat` 1.8.0, no Kotlin/Java sources (the manifest names `com.google.androidgamesdk.GameActivity`) | **Pass**: `app-debug.apk` 68.9 MB (the `.so` was left unstripped because `ndkVersion` was unset; now set to `29.0.14206865`) |
| Emulator | AVD `Medium_Phone_API_36.1` headless: Android 16, x86_64, `ro.hardware.egl=emulation`, `ro.hardware.vulkan=ranchu` | Run 1: **failed at device creation**: `max_uniform_buffer_binding_size` 65536 requested, adapter allows 16384 (fixed, see §4). Run 2: device created, then a **SIGSEGV inside the emulator's gfxstream Vulkan driver** (`vulkan.ranchu.so`, `ResourceTracker::on_vkQueueSubmit`) on egui's first paint. Symbolized frames: `egui_wgpu::winit::Painter::paint_and_update_textures`, then `wgpu Queue::submit`, then the driver. Run 3: deny-list picked GLES, but with both backends enabled the EGL surface failed (`BadAlloc`, window already bound to Vulkan). Run 4: **one backend per launch, chosen before the instance** (`ro.hardware.vulkan=ranchu` selects OpenGL ES): **runs**, renders, no crashes; launch 1.6–1.9 s on SwiftShader. Usable as the CI smoke-test device |
| **Owner's phone** | Samsung Galaxy S20+ (SM-G985F), Android 13 (API 33), Exynos 990 / **Mali-G77**, 7.6 GB RAM, 1080×2400 at 450 dpi | **PhotoCraft runs.** wgpu picked **Vulkan on Mali-G77** (driver r38p1); GPU canvas `Rgba8Unorm`, 16F canvas budget; max texture 8192 at first, **16384** after the limits fix. Launch `TotalTime` **540–627 ms** after the fixes (3.0 s on the very first start). Idle memory with no document: PSS 132 MB, RSS 199 MB. Full desktop UI renders correctly (screenshots in the session) |
| On-device start (emulator, phone, tablet) | — | Pending |

**New lockfile entries** added by the stub: `android_logger` 0.15.1, `android_log-sys` 0.3.2, `env_filter` 0.1.4.

---

## 3. Facts that resolve [verify] items in the BRD

| BRD item | Finding |
|---|---|
| GameActivity version pairing ([04](04-technical-architecture.md) §4.1) | `android-activity` 0.6.1 bundles GameActivity C glue **4.4.0** (`GameActivity.h`), so the Gradle dependency must be `androidx.games:games-activity:4.4.0` exactly |
| eframe Android API | eframe 0.36.2 has the features `android-game-activity` and `android-native-activity`, and `NativeOptions::android_app: Option<winit::platform::android::activity::AndroidApp>`. eframe does not re-export winit, so the app crate depends on `winit = "=0.30.13"` directly, as the desktop app already does on Linux |
| `accesskit` on Android (UC-15) | Compiles; no gating needed. Whether TalkBack works stays untested (NFR-A11Y-05) |
| Pure-Rust dependency graph (A-03) | Confirmed for compilation. The only C code is android-activity's own GameActivity glue, which cargo-ndk compiles with the NDK |
| Text clipboard | egui-winit leaves out `arboard` on Android, so text copy and paste also needs the bridge (FR-CLP-02), not only image clipboard |

---

## 4. Problems found on this PC (and fixes)

| Problem | Fix |
|---|---|
| `sdkmanager.bat` splits package IDs at `;` when called from PowerShell (`Package ndk not found`) | Call it through `cmd /c` with the ID in double quotes: `"ndk;29.0.14206865"` |
| The new cmdline-tools print an opt-out notice for usage metrics | The first sdkmanager call ran before the opt-out was known. Pass `--no-metrics` where the tool accepts it |
| Gradle fails with `Unable to establish loopback connection` (Java NIO pipe over Unix domain sockets cannot use the default temp location) | Set `-Djdk.net.unixdomain.tmpdir=C:\t` (via `JAVA_TOOL_OPTIONS` for the launcher, and in `org.gradle.jvmargs` for the daemon). Verified with a minimal NIO selector test |
| wgpu device creation fails on the emulator (requested limits exceed the adapter) | Stub requests `gpu_canvas::device_limits(adapter).or_worse_values_from(&adapter.limits())` (upstream fix: UC-17) |
| Emulator gfxstream Vulkan driver crashes on the first submit | Stub's `native_adapter_selector`: Vulkan first, OpenGL ES fallback, `gfxstream` on the deny-list (a seed for the real GPU plan in [04](04-technical-architecture.md) §10) |
| `PowerShell` `Set-Location` does not change the working directory of `cmd /c` children | Set `[Environment]::CurrentDirectory`, or call `gradlew.bat` by its full path |

---

## 5. On-device findings (Galaxy S20+, Android 13)

| # | Observation | Meaning | BRD item |
|---|---|---|---|
| 1 | The UI renders; taps hit the right controls (New document opens the dialog, presets select) | The core stack works on a real Mali GPU | A-02 confirmed for rendering and basic touch |
| 2 | Rotating the phone switches portrait and landscape **without restarting** the app (same process; `configChanges` honoured) | In-process configuration changes work | FR-LC-02 |
| 3 | The menu bar is hidden under the status bar; the right-hand panels run under the 3-button navigation bar | Edge-to-edge insets are not applied to the UI | FR-UI-02 (safe areas), UC-04 `safe_area` |
| 4 | The New Document dialog is larger than the screen; its Create button is off-screen in both orientations | Dialogs must fit and scroll on small windows | FR-UI-06 |
| 5 | Focusing a text field **shows the soft keyboard** (Samsung Keyboard) | The IME show path works | FR-INP-11 (partly) |
| 6 | **Typed text does not arrive** (`adb input text`, Samsung Keyboard), and **Enter does nothing**; Back hides the keyboard (system behaviour) | Text and key events through GameActivity are not reaching egui | **R-02 confirmed on hardware.** Highest-priority work: the text-entry path (GameTextInput to winit IME, or the native text sheet) and the key-event path (FR-INP-09, FR-INP-11) |
| 7 | Once a stroke test is possible (needs a document; blocked by #4 and #6 in this run) | — | Next spike step: test painting by touch and pinch zoom |
| 8 | The emulator's gfxstream Vulkan driver crashes in `vkQueueSubmit` | Real-world proof that the GPU fallback chain and driver deny-list are required | R-03, NFR-STAB-04 |
| 9 | wgpu's default device limits exceed some mobile and emulator adapters | `gpu_canvas::device_limits` should clamp to the adapter on every backend (`or_worse_values_from`) | New **UC-17** (small upstream change) |
| 10 | Enabling Vulkan and GL in the same wgpu instance breaks the GL fallback on Android (`BadAlloc` when creating the EGL surface) | The GPU plan must pick **one** backend before creating the instance; a fallback means a relaunch, or recreating the instance, not adapter switching | [04](04-technical-architecture.md) §10 (refined) |
| 11 | Portrait phone (emulator, 1080×2400): the right dock takes about two-thirds of the width, and the start page and canvas are squeezed into a narrow strip | The desktop layout does not work on a portrait phone without the Compact workspace | FR-UI-04, D-02 |

---

## 6. Grep audit: Android-relevant assumptions (non-test code, L0–L6)

| Pattern | Hits | Assessment |
|---|---|---|
| `target_os = "linux"` (Android is not "linux" in Rust) | `text/cjk.rs` (14), `ui-egui/monitor_status.rs` (10), `text/fonts.rs` (5), `ui-egui/shortcuts.rs` (2), `ui-egui/gpu_canvas.rs` (2), `ui-egui/cjk_fonts.rs` (2), `ui-egui/lib.rs` (1), `tablet/lib.rs` (1), `engine/prefs.rs` (1) | Review each: fonts and CJK (UC-06), memory (UC-05) and `prefs.rs` probably need an Android branch; `monitor_status` and `tablet` stay desktop-only |
| `std::process::Command` | `automation/workspace.rs`, `engine/layer_menu_cmds.rs`, `text/cjk.rs` (3), `text/fonts.rs`, `ui-egui/gpu_canvas.rs` (2) | Desktop helpers (system font lookups, `sysctl`, opening external editors). On Android they must be gated off or return "unsupported" (FR-FEAT-02e) |
| `std::env::temp_dir()` | automation (3 files), engine: `actions_cmds`, `exr_cmds`, `installed_profiles`, `lens_cmds`, `link_watch`, `migrate_cmds`, `notes_cmds` | Works once `TMPDIR` points at the cache directory (set from Kotlin, [04](04-technical-architecture.md) §5.2) |
| `HOME` / `XDG_*` reads | `engine/installed_profiles.rs`, `text/cjk.rs`, `text/fonts.rs` (2), `ui-egui/file_ui.rs` (2) | `HOME` is unset for Android apps, so these fall back; the font and profile directories get Android paths (UC-06) |
| `"/tmp` literals | Only inside inline `#[cfg(test)]` modules | No action |

---

## 7. Landscape sizing and touch fixes (owner: "the sizing is so broken", "mainly horizontal")

The app is now landscape-first (`android:screenOrientation="sensorLandscape"`); [FR-UI-09](02-functional-requirements.md)
was revised to match. Every fix below was checked by screenshot on the emulator (2400×1080, OpenGL ES); the
phone re-test waits for USB debugging to be re-authorised.

| # | Problem seen | Fix | Where |
|---|---|---|---|
| 1 | Menu bar under the status bar, dock under the navigation bar | Insets read through JNI (`getRootWindowInsets`: system bars, cutout, IME) and passed as `RawInput.safe_area_insets`; the app's UI is laid out in a child `Ui` limited to `ctx.content_rect()` | shell `bridge.rs`, `lib.rs` |
| 2 | UI too small or growing/shrinking in a loop | Auto UI scale fits at least 900×520 points (`fit_scale`), set only on the first pass, with a 3-frame settle | `prefs_ui.rs` |
| 3 | Dialogs taller than the screen; Create off-screen | Non-resizable dialogs scroll vertically inside the content rect; the scroll area gets the full room (an egui `Area` only offers last frame's size, so a dialog that shrank under the keyboard used to stay short) | `dialogs.rs` |
| 4 | Short landscape screen: toolbar and dock cramped | Under 600 points high: two-column toolbar and Properties collapsed, once per run | `prefs_ui.rs` |
| 5 | Preferences body cut off, section list overflowing, OK/Cancel unreachable | On short screens the body gets what title, search and buttons leave, and the section list scrolls | `prefs_ui.rs` |
| 6 | Typed text never arrived | GameTextInput state polled each frame and diffed into egui text, Backspace and Enter events | shell `lib.rs` |
| 7 | Keyboard popped up by itself (dialogs focus their first field) and covered ~70% of the landscape screen | The keyboard shows only for a field the user tapped; a tap on a focused field brings it back | shell `gate_keyboard` |
| 8 | Keyboard flickered on and off | egui stops reporting IME output for a field scrolled out of sight; the shell keeps it for the focused field, and the dialog scrolls the focused field back into view when the keyboard changes the room | shell, `dialogs.rs` |
| 9 | Letter keyboard for number fields | Number pad (`TYPE_CLASS_NUMBER`, decimal, signed) for egui `DragValue`s | shell `start_editor` |
| 10 | The number pad's Enter did nothing | `PhotoCraftActivity.onEditorAction` forwards it as an Enter key (commits, then the dialog's default button, as on desktop) | Java activity |
| 11 | Quick taps on menu items did nothing | A release in the same frame as its press (or as the last moves of a drag) is held to the next frame | shell |
| 12 | Finger strokes hairline-thin | Finger contact "pressure" stripped until the stylus bridge (FR-INP-06) | shell |
| 13 | A swipe on a dialog or panel moved a slider or scrubby label instead of scrolling | Touch gestures: a finger on a small control is held until it moves 8 pt; vertical-first swipes scroll (with fling), sideways moves and 300 ms holds go to the control. The canvas, colour field, previews, splitters and scroll bars get the finger at once | shell `gesture.rs` (7 host tests) |
| 14 | Ctrl/Shift shortcuts from a hardware keyboard arrived without modifiers (Ctrl+K typed K) | winit 0.30 never reports modifiers on Android: the activity records `KeyEvent` meta state, the shell reads it through JNI and stamps it on key and pointer events | Java activity, shell |
| 15 | Back closed PhotoCraft and its documents | Back sends the app to the background (`moveTaskToBack`) | Java activity |
| 16 | `std::env::temp_dir()` pointed at an unwritable directory | `TMPDIR` set to the cache directory before the native library loads | Java activity |

Known and left for later: layer rows now need a sideways move or a long press to start a drag (a vertical swipe
scrolls the list); horizontal scroll areas don't scroll by finger; the OpenGL ES (emulator) path uses the CPU canvas.

---

## 8. Still to do in Phase 0

1. ~~Release `.so`: size, `NEEDED`, 16 KB alignment.~~ Done.
2. ~~APK, emulator start.~~ Done (emulator via OpenGL ES).
3. ~~Phone: start, GPU, launch time, keyboard, memory.~~ Done. **Tablet: still to do** (a second GPU family, plus a
   stylus).
4. ~~Painting by touch.~~ Done on the emulator (§7). Pinch zoom and the phone re-test are still to do.
5. Pause and resume 20 times (surface loss).
6. ~~Grep audit.~~ Done (§6).
7. The G0 review with the owner. A first **G0 recommendation: GO.** The stack builds, packages and runs on real
   hardware. The blockers found are the ones the BRD already planned for (R-02, R-03, FR-UI-02, FR-UI-04, FR-UI-06)
   and are scheduled in Phases 1–2.

**How to reproduce this spike:**

1. Build the libraries. In `upstream/` on branch `android/phase0-spike`, with `ANDROID_NDK_HOME` set to NDK r29:

   ```sh
   cargo ndk -t arm64-v8a -t x86_64 -P 29 -o ../android-spike/app/src/main/jniLibs build --release -p photocraft-android --features heif
   ```

2. Package the APK. In `android-spike/`, with `JAVA_HOME` set to the Android Studio JBR and
   `JAVA_TOOL_OPTIONS=-Djdk.net.unixdomain.tmpdir=C:\t`, run `gradlew.bat assembleDebug`.
3. Install and launch:

   ```sh
   adb install -r -t app/build/outputs/apk/debug/app-debug.apk
   adb shell am start -n local.photocraft.spike/.PhotoCraftActivity
   ```

The spike changes in `upstream/` are uncommitted on purpose: nothing is committed or pushed without the owner's
go-ahead.
