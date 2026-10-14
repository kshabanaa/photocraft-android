# 05: UX: touch, stylus and form-factor design

| | |
|---|---|
| **Version** | 1.0 draft, 2026-10-13 |
| **Implements** | FR-INP-*, FR-UI-*, parts of FR-FIO-* ([02](02-functional-requirements.md)) |
| **Constraint** | Upstream goal is 1:1 Photoshop parity in menus, shortcuts and behaviour, studied for behaviour only (clean-room). Touch adaptations add input paths; they do not remove or rename desktop commands |

---

## 1. Design principles

1. **Same app, adapted input.** Every desktop command stays reachable. Touch adds gestures, a modifier control and
   larger targets. It does not fork the feature set.
2. **Canvas first.** The canvas uses the full window (edge to edge). Chrome is padded by the safe areas and can be
   hidden (full-screen canvas toggle).
3. **No hover-only behaviour.** Anything shown on hover (tooltips, rollover highlights, Object Finder previews, blend
   mode previews) is also reachable by long press or a visible control (FR-UI-05).
4. **The stylus draws, fingers navigate.** Once a stylus is detected, fingers default to navigation, with palm
   rejection. Finger painting stays available as a preference.
5. **Large screens get the desktop layout; small screens get a re-composed one.** Expanded windows keep the
   Photoshop layout with touch metrics. Compact windows re-compose the same panels into drawers and sheets.
6. **Avoid edge-swipe gestures.** Android's gesture navigation owns the screen edges (back and home). Panels open
   from visible buttons, not from edge swipes. Where an edge must be used, request system gesture exclusion
   rectangles sparingly (the platform caps them at 200 dp per edge).
7. **Familiar mobile conventions where they don't conflict with Photoshop habits**: two-finger tap to undo, pinch to
   zoom, long press for context menus, bottom sheets on phones.

---

## 2. Window size classes and layout modes

| Size class (window width) | Typical devices | Workspace (auto) | UI density (auto) |
|---|---|---|---|
| **Expanded** (≥ 840 dp) | Tablets in landscape, Chromebooks, DeX and desktop windowing, unfolded foldables in landscape | **Touch** (desktop layout with touch metrics). Desktop workspaces stay available | Touch on touch-only devices; Compact when a mouse or trackpad is attached |
| **Medium** (600–839 dp) | Tablets in portrait, unfolded foldables in portrait, split screen on tablets | **Touch** with the right dock as an overlay drawer | Touch |
| **Compact** (< 600 dp) | Phones, folded foldables, narrow split screen | **Compact** (phone layout) | Touch |

The automatic choice can be pinned by the user (Window › Workspace). Switching size class (fold, rotate, resize)
keeps all document and tool state; only the arrangement changes (FR-LC-08).

---

## 3. Layouts

### 3.1 Expanded: the Touch workspace

```text
+--------------------------------------------------------------------------------+
| Menu: File Edit Image Layer Type Select Filter View Window Help  [Search] [Undo][Redo] [Share] |  <- top bar, 48 dp
+--------------------------------------------------------------------------------+
| [Brush v]  Size 30 [-][+]  Hardness 80%  Opacity 100%  Flow 100%  [...]        |  <- options bar, touch metrics
+----+---------------------------------------------------------+-----------------+
| T1 |                                                         | Layers      [v] |
| T2 |                                                         |  [o] Curves 1   |
| T3 |                                                         |  [o] Title  (T) |
| T4 |                CANVAS (edge to edge)                    |  [o] Photo      |
| T5 |                                                         |  [+][fx][o][x]  |
| T6 |                                                         +-----------------+
| .. |                                       +-----+           | Properties  [>] |
| FG |                                       | SHF |  <- Touch | Adjustments [>] |
| BG |                                       +-----+  shortcut | Color       [>] |
+----+---------------------------------------------------------+-----------------+
| 66.7%  |  4000 x 3000 px  |  RGB/8  |  GPU: Vulkan                              |  <- status bar
+--------------------------------------------------------------------------------+
```

- Toolbox: one column with 48 dp buttons. Tool groups show a corner marker; a long press opens the flyout of hidden
  tools (FR-INP-04).
- Right dock: collapsible panel groups. Tapping a header expands or collapses it. Floating panels stay available but
  are not the default on touch.
- The top-bar Search opens the command palette (desktop ⌘F), the fastest route to any of the 628 menu commands.

### 3.2 Medium: the Touch workspace, narrow

Same as 3.1, but the right dock becomes an **overlay drawer** opened by a dock button in the top bar. The options bar
wraps into two rows or scrolls horizontally. The toolbox can collapse to a 2 × N floating palette.

### 3.3 Compact: the phone layout

```text
+------------------------------+
| <-  wave.psd    [Undo][Redo] [Menu] |  <- top app bar
+------------------------------+
| Brush - 30 px - 80%      [v] |  <- condensed options (tap: options sheet)
+------------------------------+
|                              |
|                              |
|           CANVAS             |
|                              |
|                      +-----+ |
|                      | SHF | |  <- Touch shortcut
|                      +-----+ |
+------------------------------+
| [Tools] [Adjust] [Layers] [Props] |  <- bottom bar: opens drawer or sheets
+------------------------------+

Layers sheet (half height):
+------------------------------+
|            -----             |  <- drag handle: peek / half / full
| Layers                   [x] |
|  [o] Curves 1           [:]  |
|  [o] Title (T)          [:]  |
|  [o] Photo              [:]  |
| [+] [fx] [Mask] [Group] [Del] |
+------------------------------+
```

- **Menu** opens the full menu tree as a drill-down list, with a search field (the command palette) on top and a back
  arrow per level.
- **Tools** opens a drawer with every tool grouped as on desktop. The recently used tools are pinned at the top.
- **Bottom sheets** (Layers, Properties, Adjustments, Color, Brushes, History) have three detents (peek, half, full).
  The canvas stays interactive above a half sheet.
- Dialogs open full-screen with a top app bar (Cancel / title / OK) (FR-UI-06).

---

## 4. Gesture map

| Gesture | Default action | Rules and conflicts | Preference |
|---|---|---|---|
| 1-finger drag | Active tool | When a stylus has been used and "Finger actions" is set to Navigate, it pans instead | Touch › Finger actions: Paint / Navigate |
| 2-finger drag | Pan | A second finger within 150 ms of the first cancels the first finger's stroke uncommitted; a stroke older than 150 ms continues (FR-INP-02) | — |
| Pinch | Zoom around the centroid | Optional soft snap to 100% and Fit | — |
| 2-finger rotate | Rotate the view | Starts only after 10° of rotation (avoids accidental rotation); snaps near 0°, 90°, 180° and 270°; double-tapping the rotation indicator resets | Touch › Rotate with two fingers |
| 2-finger tap | Undo | ≤ 250 ms, ≤ 10 dp movement | Touch › Two-finger tap |
| 3-finger tap | Redo | Same thresholds | Touch › Three-finger tap |
| 2-finger double tap | Fit on Screen | — | — |
| 4-finger tap | Toggle full-screen canvas (desktop: Tab) | Could | — |
| Long press (finger) | Secondary click: context menu, tooltip, or tool flyout | 500 ms, ≤ 8 dp movement; the menu opens offset from the finger | — |
| Fling after a 2-finger pan | Kinetic pan (`flick_pan.rs`) | Stops on touch | View › Kinetic panning |
| Stylus tip | Active tool, with pressure and tilt | Palm rejection active (FR-INP-07) | Tools › Use Tablet Pressure |
| Stylus eraser end | Eraser tool, restored when flipped back | Desktop behaviour | — |
| Stylus hover | Brush outline preview; tooltip after 700 ms | Only if the hardware reports hover | Touch › Stylus hover preview |
| Stylus button (hold) | Alt (eyedropper, clone source) | — | Stylus › Button 1 |
| Stylus button (click) or second button | Secondary click (context menu) | — | Stylus › Button 2 |

---

## 5. The Touch shortcut (on-screen modifiers)

Photoshop workflows depend on modifier keys: Shift to constrain, Alt to sample or duplicate, Ctrl to select a mask,
Space to pan. Without a keyboard, the **Touch shortcut** supplies them (FR-INP-05).

| Aspect | Specification |
|---|---|
| Forms | **Ring** (default): a 56 dp floating circle. Holding the centre gives Shift; sliding out to the ring (28–56 dp from the centre) gives Alt. **Key bar**: four explicit 48 dp keys (Shift, Alt, Ctrl, Space). **Off**. (Preferences › Touch › Modifier control) |
| States | Idle; **momentary** while held; **latched** after a tap (released after the next completed tool gesture); **locked** after a double tap (until tapped again). The state shows on the control and in the status bar |
| Merging | Touch-shortcut modifiers combine (OR) with physical keyboard modifiers and stylus-button mappings before tool dispatch |
| Placement | Draggable; position remembered per orientation and size class; default bottom-left (right-handed), mirrored for left-handed use (Preferences › Touch › Handedness) |
| Feedback | Haptic tick on latch and lock (Could); a short label pops up the first 5 times ("Shift") |
| Discoverability | Shown by default on touch-only devices; hidden automatically while a hardware keyboard is connected (user can override) |

---

## 6. Stylus behaviour

| Topic | Behaviour |
|---|---|
| Pressure | Mapped through the existing pressure curve (`pressure_curve_ui.rs`); "Use Tablet Pressure" off makes the pen paint like a mouse (desktop behaviour) |
| Tilt and orientation | Converted to W3C tilt X and Y and fed to Brush Pose and Shape Dynamics as on desktop |
| Eraser end | Switches to the Eraser and back (desktop behaviour) |
| Buttons | Mappable (§4); defaults: hold = Alt, click = secondary click |
| Hover | Brush outline and cursor preview; no painting |
| Palm rejection | While the pen is in range or down, finger contacts on the canvas are ignored for tools; navigation follows "Finger actions" |
| Full-rate input | Every digitizer sample, including batched ones, becomes a stroke point (FR-INP-08) |
| Prediction (Could) | Motion prediction for the live-stroke preview only (never for committed pixels), to hide latency |

---

## 7. Context menus, flyouts and tooltips

- Long press on the canvas, a layer row, a panel item, a swatch or a tool group opens the same menu right-click opens
  on desktop.
- Placement: above-left of the finger for right-handed users (mirrored for left-handed), never under the finger.
  On Compact, context menus open as bottom sheets.
- Items are at least 48 dp tall in Touch density, with dividers and submenus as drill-downs.
- Tooltips: long press on a control shows the tooltip without activating the control. Lifting the finger outside the
  control cancels the press.

---

## 8. Menus and the command palette

- **Expanded:** the desktop menu bar stays (parity), with 48 dp targets. Submenus open on tap, never on hover. Tapping
  outside closes them.
- **Medium:** menu bar or a single Menu button, depending on width.
- **Compact:** a Menu button opens a drill-down list (§3.3).
- **Command palette:** always one tap away (Search button). It is the recommended way to reach deep commands on
  phones.
- **Keyboard shortcuts:** shown in menus only while a hardware keyboard is connected.

---

## 9. Panels

| Panel family | Expanded | Medium | Compact |
|---|---|---|---|
| Layers, Channels, Paths | Right dock (expanded by default) | Overlay drawer | Bottom sheet |
| Properties, Adjustments | Right dock (collapsed) | Drawer | Bottom sheet |
| Color, Swatches, Brushes, Brush Settings | Right dock group | Drawer | Bottom sheet; Brushes also from the options bar |
| History, Actions, Info, Histogram, Navigator | Right dock (collapsed) | Drawer | Bottom sheet from Menu › Window |
| Character and Paragraph (type) | Right dock when the Type tool is active | Drawer | Sheet that opens with the Type tool |

Layer rows grow to 56 dp in Touch density. The visibility toggle and the thumbnail are separate 48 dp targets. A row
is dragged by a handle; a long press on the row opens the layer context menu.

---

## 10. Dialogs and controls

| Control | Touch adaptation |
|---|---|
| Sliders | 48 dp hit height; tapping the value opens numeric entry; dragging 48 dp or more vertically away from the slider enables fine control (Could) |
| Numeric fields | Horizontal drag scrubs (`scrub_wrap.rs`); a tap opens the numeric keyboard (IME `number` input type, or the native text-entry fallback) |
| Curves and Levels editors | 24 dp point hit radius; a long press on a point deletes it (desktop: drag off the graph); the dialog is full-screen on Compact |
| Colour picker and eyedropper | The pixel loupe draws above the finger; release samples |
| Transform and crop handles | 44 dp hit areas; **floating Commit and Cancel buttons** next to the transformed bounds, as well as in the options bar |
| Selections | "Select Subject" and "Object Selection" promoted in the selection options bar; Quick Selection brush size controlled from the options bar |
| Modal dialogs | Scrollable content; buttons pinned to the bottom (Expanded and Medium) or the top app bar (Compact) |

---

## 11. Text editing

- **Type tool:** a tap creates point text; a drag creates a paragraph box; double-tapping a type layer edits it. The
  soft keyboard appears, and the canvas scrolls so the caret stays visible above the IME (safe area).
- **Selection:** double tap selects a word, triple tap a paragraph; drag handles adjust the selection (Should).
  Select All is in the options bar.
- **IME:** composition (Chinese, Japanese, Korean) inline where GameTextInput through winit is reliable. Otherwise the
  **native text sheet** opens with the run's text, and its result replaces the text while keeping the Character and
  Paragraph formatting (FR-INP-11).
- **Fields:** layer names, search and numeric fields use the same keyboard path.

---

## 12. File flows

### 12.1 Start screen (no document open, FR-UI-08)

```text
+------------------------------------------------------------+
|  PhotoCraft                                    [Help] [Settings] |
|                                                            |
|  [ New... ]  [ Open... ]  [ From Photos ]                  |
|                                                            |
|  Recent                                                    |
|  +--------+ +--------+ +--------+ +--------+               |
|  | thumb  | | thumb  | | thumb  | |  (n/a) |               |
|  |wave.psd| |logo.pc.| |cat.jpg | |old.psd |               |
|  +--------+ +--------+ +--------+ +--------+               |
+------------------------------------------------------------+
```

### 12.2 Open, Save, Export, Share

| Flow | Behaviour |
|---|---|
| Open | System picker (multi-select). While a large or remote file loads, a progress job with Cancel shows |
| Save | If the document has a writable origin: save in place and show "Saved to <folder>/<name>". Otherwise: system Save As picker |
| Save As / Save a Copy | System create-document picker with the suggested name and format |
| Export As | The existing dialog (format, quality, scale, preview) plus a destination choice: **Save to Gallery** / **Choose location…** / **Share…** |
| Share | Export, then the share sheet, from File › Share… and the top-bar Share button |
| Recovery | At start-up, after an abnormal end: "PhotoCraft closed unexpectedly (reason). N documents were recovered." with [Open all] [Review] [Discard] |

### 12.3 Persona P3 tasks on a phone (acceptance for FR-UI-04)

1. Open a PSD from Google Drive that was shared by email.
2. Hide two layers and show a third.
3. Change the text of a type layer and its colour.
4. Add a Brightness/Contrast adjustment layer clipped to one layer.
5. Export a JPEG at quality 80 and share it to a messaging app.
6. Save the PSD back to Drive and reopen it to verify.

---

## 13. Onboarding and help

- **First run:** three dismissible cards: gestures (§4), the Touch shortcut (§5), stylus settings (§6). "Show again"
  under Help.
- **Help › Touch and Stylus:** a page in the user docs, with short clips recorded through the QA control channel on
  public-domain art (upstream rule for demo images).
- **Empty states:** a tool that needs a modifier on desktop shows a one-line hint the first time, for example "Hold
  Alt (Touch shortcut ring) and tap to set the clone source."

---

## 14. Accessibility (UX level)

- Touch density meets 48 dp targets (NFR-A11Y-01). Contrast follows the theme tokens (NFR-A11Y-02).
- Respect the system font scale within clamps. Respect "Remove animations" (no kinetic or animated transitions when
  set).
- Gesture alternatives: every gesture action is also a visible command (Undo and Redo buttons, Fit on Screen in the
  View menu, the rotation reset in the status bar).
- Known gap: no screen reader support in v1 (NFR-A11Y-05).

---

## 15. Design tokens (additions to `theme::Tokens`)

| Token | Compact (desktop) | Touch |
|---|---|---|
| `target.min` | desktop value | 48 dp |
| `toolbox.button` | desktop value | 48 dp |
| `row.layer` | desktop value | 56 dp |
| `menu.item` | desktop value | 48 dp |
| `slider.hit` | desktop value | 48 dp |
| `spacing.gap` | desktop value | +4 dp |
| `handle.hit` (transform, path points) | desktop value | 44 dp |

Colours and radii keep coming from the theme tokens (upstream rule: never hard-coded).

---

## 16. UX validation

| Round | When | Participants | Tasks | Success criteria |
|---|---|---|---|---|
| R1 (internal) | Before G1 | The owner plus 1–2 people new to PhotoCraft; agents run the same workflows through the control channel first and review the screenshots | 6 core workflows on a tablet | No blockers; issues logged |
| R2 (moderated) | Before G2 | 5 users from P1 and P2 | 6 core workflows (retouch, composite, cut-out, design, paint, PSD exchange) on a tablet with a stylus | 5 of 5 complete each workflow; SUS ≥ 70 |
| R3 (beta) | G2 to G3 | Closed beta (Discord, Play closed track) | Free use plus a survey | ≥ 80% "usable for my work" among tablet users; top-10 issues fixed |
| R4 (phone) | Before v1.1 (or G3 if D-02 = phone in v1.0) | 5 users from P3 | §12.3 tasks | 5 of 5 complete; median time ≤ 2× that on a tablet |
