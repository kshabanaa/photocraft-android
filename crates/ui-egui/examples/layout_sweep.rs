//! Open every dialog, panel and tool offscreen at one screen size, flag what doesn't fit, and save a
//! screenshot of each: a layout check for small screens (phones), tablets and desktop sizes alike.
//!
//! ```sh
//! # A phone in landscape (Galaxy S20+: 1120x520 points, status bar 33 pt, navigation bar 65 pt):
//! cargo run --release -p photocraft-ui-egui --example layout_sweep -- \
//!     --out sweep/phone --size 1120x520 --scale 2.077 --insets 0,33,65,0
//! ```
//!
//! `--insets left,top,right,bottom` are the system bars (points), as a phone reports them.
//! `--only <prefix>` limits the sweep to ids starting with it (`filter.`, `tool:`, `window.`).
//! `--no-shots` writes the report only.
//! `--theme <name>` runs the sweep in that theme (`pro`, the default, `studio`, `classic`, …: the
//! Window › Theme ids without `window.theme.`); each item starts from it.
//!
//! The sweep opens each enabled menu item whose label ends in "…" (it opens a dialog), each Window
//! menu item (a panel) and each tool (its options bar), one at a time from the same starting
//! state. A floating window, dialog or popup whose rectangle leaves the usable screen (the window
//! minus the system bars) is reported with how far it overflows. `report.md` lists every item;
//! the PNGs are named after the item for review.

use photocraft_ui_egui::control::{ControlRequest, handle};
use photocraft_ui_egui::state::Tool;
use photocraft_ui_egui::{PhotocraftApp, Services};
use serde_json::{Value, json};
use std::fmt::Write as _;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn pair(s: &str) -> Option<(f32, f32)> {
    let (a, b) = s.split_once('x')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

/// What one item did on screen.
struct Outcome {
    id: String,
    label: String,
    path: String,
    opened: String,
    overflow: Vec<String>,
    status: String,
}

type Harness = egui_kittest::Harness<'static, PhotocraftApp>;

fn call(harness: &mut Harness, method: &str, params: Value) -> Value {
    let ctx = harness.ctx.clone();
    let (req, _rx) = ControlRequest::new(method, params);
    let out = match handle(harness.state_mut(), &ctx, &req) {
        photocraft_ui_egui::control::Outcome::Done(v) => v,
        _ => Value::Null,
    };
    // The harness doesn't call raw_input_hook: feed queued synthetic input step by step.
    loop {
        let step = harness.state_mut().take_synthetic_step();
        if step.is_empty() {
            break;
        }
        for e in step {
            harness.event(e);
        }
        harness.step();
    }
    harness.run_steps(6);
    out
}

/// Every visible floating layer (windows, dialogs, popups, menus) that leaves the usable screen,
/// as "layer: overflows right 40 pt, bottom 120 pt". Full-screen layers (a modal's dimmed
/// backdrop) are skipped.
fn overflow(ctx: &egui::Context) -> Vec<String> {
    let room = ctx.content_rect();
    let screen = ctx.viewport_rect();
    let mut found = Vec::new();
    let layers: Vec<egui::LayerId> = ctx.memory(|m| m.layer_ids().collect());
    for layer in layers {
        if !matches!(layer.order, egui::Order::Middle | egui::Order::Foreground) {
            continue;
        }
        let Some(rect) = ctx.memory(|m| if m.areas().is_visible(&layer) { m.area_rect(layer.id) } else { None }) else { continue };
        if rect.width() >= screen.width() - 1.0 && rect.height() >= screen.height() - 1.0 {
            continue;
        }
        let sides = [
            ("left", room.left() - rect.left()),
            ("top", room.top() - rect.top()),
            ("right", rect.right() - room.right()),
            ("bottom", rect.bottom() - room.bottom()),
        ];
        let over: Vec<String> = sides.iter().filter(|(_, d)| *d > 2.0).map(|(s, d)| format!("{s} {d:.0} pt")).collect();
        if !over.is_empty() {
            found.push(format!("{:?} {:.0}x{:.0}: {}", layer.order, rect.width(), rect.height(), over.join(", ")));
        }
    }
    found
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(arg(&args, "--out").unwrap_or_else(|| "sweep".into()));
    let (w, h) = arg(&args, "--size").as_deref().and_then(pair).unwrap_or((1120.0, 520.0));
    let scale: f32 = arg(&args, "--scale").and_then(|s| s.parse().ok()).unwrap_or(2.0);
    let insets: Vec<f32> = arg(&args, "--insets").map(|s| s.split(',').filter_map(|v| v.trim().parse().ok()).collect()).unwrap_or_default();
    let only = arg(&args, "--only");
    let shots = !args.iter().any(|a| a == "--no-shots");
    let theme = format!("window.theme.{}", arg(&args, "--theme").unwrap_or_else(|| "pro".into()));
    std::fs::create_dir_all(&out).expect("create --out");

    let mut harness: Harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(w, h))
        .with_pixels_per_point(scale)
        .with_max_steps(64)
        .wgpu()
        .build_eframe(move |cc| {
            PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
            let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
            if let Some(rs) = cc.wgpu_render_state.as_ref() {
                app.set_wgpu(rs.clone());
            }
            app
        });
    if let [left, top, right, bottom] = insets[..] {
        harness.input_mut().safe_area_insets = Some(egui::SafeAreaInsets(egui::epaint::MarginF32 { left, top, right, bottom }));
    }
    harness.run_steps(8);
    call(&mut harness, "ui.menu.invoke", json!({"id": theme}));
    // A document with a pixel layer, so document and layer commands are enabled.
    call(&mut harness, "engine.execute", json!({"command": "file.new", "params": {"width": 1920, "height": 1080}}));
    call(&mut harness, "engine.execute", json!({"command": "layer.new", "params": {}}));
    let base = harness.state().ui.clone();

    let items: Vec<Value> = call(&mut harness, "ui.menu.list", Value::Null).get("result").and_then(Value::as_array).cloned().unwrap_or_default();
    // Items that leave the app, replace the document or need a file: not layout.
    let skip = ["file.open", "file.close", "file.revert", "file.exit", "file.quit", "file.place", "file.browse", "app.quit"];
    let mut todo: Vec<(String, String, String)> = Vec::new();
    for item in &items {
        let id = item["id"].as_str().unwrap_or_default().to_owned();
        let label = item["label"].as_str().unwrap_or_default().to_owned();
        let path: Vec<&str> = item["path"].as_array().map(|p| p.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        let dialog = label.ends_with('…') || label.ends_with("...");
        let panel = path.first() == Some(&"Window");
        if !item["enabled"].as_bool().unwrap_or(false) || !(dialog || panel) || skip.iter().any(|s| id.starts_with(s)) {
            continue;
        }
        todo.push((id, label, path.join(" › ")));
    }
    for tool in Tool::ALL {
        todo.push((format!("tool:{tool:?}"), tool.label().to_owned(), "Toolbar".into()));
    }
    todo.retain(|(id, _, _)| only.as_deref().is_none_or(|p| id.starts_with(p)));
    todo.dedup_by(|a, b| a.0 == b.0);
    eprintln!("sweep: {} items at {w}x{h} pt, scale {scale}", todo.len());

    let mut results = Vec::new();
    for (n, (id, label, path)) in todo.iter().enumerate() {
        // The same starting point every time: no dialogs, the base panels and tool, a document.
        {
            let app = harness.state_mut();
            app.ui = base.clone();
            app.ui.dialogs.clear();
            app.ui.status.clear();
        }
        harness.key_press(egui::Key::Escape);
        harness.run_steps(4);
        // Theme items change a preference that outlives `ui`.
        if results.last().is_some_and(|r: &Outcome| r.id.starts_with("window.theme.")) {
            call(&mut harness, "ui.menu.invoke", json!({"id": theme}));
        }
        if harness.state().session.documents().is_empty() {
            call(&mut harness, "engine.execute", json!({"command": "file.new", "params": {"width": 1920, "height": 1080}}));
        }
        let opened = if let Some(name) = id.strip_prefix("tool:") {
            if let Some(tool) = Tool::ALL.iter().find(|t| format!("{t:?}") == name) {
                harness.state_mut().ui.tool = *tool;
            }
            harness.run_steps(6);
            "tool".to_owned()
        } else {
            let reply = call(&mut harness, "ui.menu.invoke", json!({"id": id}));
            if reply.get("ok") == Some(&Value::Bool(false)) {
                format!("error: {}", reply.get("error").and_then(Value::as_str).unwrap_or("?"))
            } else if !harness.state().ui.dialogs.is_empty() {
                format!("dialog x{}", harness.state().ui.dialogs.len())
            } else {
                "panel/other".to_owned()
            }
        };
        // Let windows find their size and fade in.
        harness.run_steps(10);
        let found = overflow(&harness.ctx);
        let status = harness.state().ui.status.clone();
        if shots {
            let file = out.join(format!("{n:03}-{}.png", id.replace([':', '/', '\\'], "_")));
            match harness.render() {
                Ok(img) => {
                    if let Err(e) = img.save(&file) {
                        eprintln!("{id}: save failed: {e}");
                    }
                }
                Err(e) => eprintln!("{id}: render failed: {e}"),
            }
        }
        eprintln!("{n:03} {id}: {opened}{}", if found.is_empty() { String::new() } else { format!("  OVERFLOW {}", found.join(" | ")) });
        results.push(Outcome { id: id.clone(), label: label.clone(), path: path.clone(), opened, overflow: found, status });
    }

    let mut md = String::new();
    let bad = results.iter().filter(|r| !r.overflow.is_empty()).count();
    let _ = writeln!(md, "# Layout sweep: {w}x{h} pt, scale {scale}, insets {insets:?}, {theme}\n");
    let _ = writeln!(md, "{} items, **{bad} overflow** the usable screen.\n", results.len());
    let _ = writeln!(md, "| # | Item | Menu | Opened | Overflow | Status |\n|---|---|---|---|---|---|");
    for (n, r) in results.iter().enumerate() {
        let over = if r.overflow.is_empty() { "fits".to_owned() } else { format!("**{}**", r.overflow.join("; ")) };
        let _ = writeln!(md, "| {n:03} | `{}` {} | {} | {} | {over} | {} |", r.id, r.label, r.path, r.opened, r.status.replace('|', "/"));
    }
    std::fs::write(out.join("report.md"), md).expect("write report");
    println!("{} items, {bad} overflow; report: {}", results.len(), out.join("report.md").display());
}
