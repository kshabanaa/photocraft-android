//! Independent builds (`brand::Brand::Independent`, the `unbranded` feature) show none of the
//! ArtCraft marks: not in the menus, the title bar, the start screen or About, and the commands that
//! lead to ArtCraft refuse to run. Official builds keep them.

use crate::brand::Brand;
use crate::{PhotocraftApp, Services};
use egui::accesskit::Role;
use egui::vec2;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable, by},
};
use photocraft_engine::Session;
use serde_json::json;
use std::sync::{Arc, Mutex};

const CREDIT: &str = "An independent build, based on PhotoCraft by the ArtCraft team.";

/// The app on its start screen with the app-drawn title bar (which carries the brand mark), as a
/// `brand` build.
fn harness(brand: Brand) -> Harness<'static, PhotocraftApp> {
    let mut h = Harness::builder().with_size(vec2(1440.0, 900.0)).with_max_steps(64).build_eframe(|cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        PhotocraftApp::new(Session::new(), Services::default())
    });
    h.state_mut().brand = brand;
    h.state_mut().custom_titlebar = true;
    h.run_steps(4);
    h
}

/// The text of every widget whose text contains `needle` (a label's text is its value).
fn texts_with(h: &Harness<'_, PhotocraftApp>, needle: &str) -> Vec<String> {
    h.query_all(by().label_contains(needle))
        .filter_map(|n| {
            let node = n.accesskit_node();
            if node.role() == Role::Label { node.value() } else { node.label() }
        })
        .collect()
}

#[test]
fn independent_builds_show_no_artcraft_marks_on_the_start_screen_title_bar_or_about() {
    let mut h = harness(Brand::Independent);
    assert!(crate::brand::mark_rect(&h.ctx).is_some(), "the title bar still has a mark");
    // Start screen and title bar: no Discord button or link, and ArtCraft only in the plain-text credit.
    assert_eq!(texts_with(&h, "Discord"), Vec::<String>::new());
    assert_eq!(texts_with(&h, "ArtCraft"), [CREDIT]);
    assert!(h.query_by_label("PhotoCraft on GitHub").is_some(), "the start screen links to PhotoCraft's source");
    assert!(h.query_by_label(crate::brand::INDEPENDENT_MAINTAINER.0).is_some(), "the start screen credits the build's maintainer");
    // About: the same credit in place of the community links.
    h.state_mut().ui.open_dialog(crate::state::DialogKind::About, Default::default());
    h.run_steps(6);
    assert_eq!(texts_with(&h, "Discord"), Vec::<String>::new());
    assert_eq!(texts_with(&h, "ArtCraft"), [CREDIT, CREDIT], "start screen and About");
}

/// The counterpart: the queries above do find the marks where a build has them.
#[cfg(not(feature = "unbranded"))]
#[test]
fn official_builds_keep_the_artcraft_links() {
    let mut h = harness(Brand::Official);
    assert!(h.query_by_label("Join us on Discord").is_some(), "start screen Discord button");
    assert!(h.query_by_label("Discord").is_some(), "title bar Discord button");
    assert!(h.query_by_label("ArtCraft").is_some(), "start screen ArtCraft link");
    assert!(texts_with(&h, CREDIT).is_empty(), "no independent-build credit");
    h.state_mut().ui.open_dialog(crate::state::DialogKind::About, Default::default());
    h.run_steps(6);
    assert_eq!(h.query_all_by_label("Join us on Discord").count(), 2, "start screen and About");
}

#[test]
fn independent_builds_leave_the_artcraft_links_out_of_the_menus() {
    let mut app = PhotocraftApp::new(Session::new(), Services::default());
    app.brand = Brand::Independent;
    let items = crate::menus::menu_items(&app);
    let help: Vec<&str> = items.iter().filter(|i| i.path == ["Help"]).map(|i| i.id.as_str()).collect();
    assert_eq!(help, ["help.github", "help.reportIssue", "---", "help.systemInfo", "help.about"]);
    // Nowhere else either (the command palette and the shortcut editor list these same items).
    let branded: Vec<&str> = items.iter().filter(|i| i.label.contains("ArtCraft") || i.label.contains("Discord")).map(|i| i.id.as_str()).collect();
    assert!(branded.is_empty(), "{branded:?}");
    for id in crate::links::ARTCRAFT_COMMANDS {
        assert!(crate::menus::is_live(id), "{id} stays a known command");
        assert!(!crate::menus::is_enabled(&app, id), "{id}");
    }
    #[cfg(not(feature = "unbranded"))]
    {
        app.brand = Brand::Official;
        let help = crate::menus::menu_items(&app).into_iter().filter(|i| i.path == ["Help"]).count();
        assert_eq!(help, 8, "official builds list all five links");
    }
}

#[test]
fn artcraft_link_commands_refuse_to_run_in_independent_builds() {
    let opened: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let rec = opened.clone();
    let services = Services {
        open_url: Some(Box::new(move |u: &str| {
            rec.lock().unwrap().push(u.to_string());
            Ok(())
        })),
        ..Default::default()
    };
    let mut app = PhotocraftApp::new(Session::new(), services);
    app.brand = Brand::Independent;
    let ctx = egui::Context::default();
    for id in crate::links::ARTCRAFT_COMMANDS {
        let err = crate::menus::invoke(&mut app, &ctx, id, json!({})).unwrap_err();
        assert!(err.contains("not in this build"), "{id}: {err}");
    }
    assert!(opened.lock().unwrap().is_empty(), "no ArtCraft page opened");
    // PhotoCraft's own source and issue tracker still open.
    for id in ["help.github", "help.reportIssue"] {
        let r = crate::menus::invoke(&mut app, &ctx, id, json!({})).unwrap();
        assert_eq!(r["url"], crate::links::url_for(id).unwrap());
    }
    assert_eq!(opened.lock().unwrap().len(), 2);
}
