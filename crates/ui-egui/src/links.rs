//! Community and project links: the Help menu, the About dialog, the start screen and the title
//! bar all open these. URLs follow the crafting-app pattern (`getartcraft.com/apps/<app>`,
//! `github.com/storytold/<app>`). Independent builds ([`Brand::Independent`]) leave out the ones
//! that lead to ArtCraft ([`ARTCRAFT_COMMANDS`]).

use serde_json::{Value, json};

use crate::PhotocraftApp;
use crate::brand::Brand;

pub const DISCORD: &str = "https://discord.gg/artcraft";
pub const ARTCRAFT_WEBSITE: &str = "https://getartcraft.com";
pub const APP_PAGE: &str = "https://getartcraft.com/apps/photocraft";
pub const GITHUB: &str = "https://github.com/storytold/photocraft";
pub const ISSUES: &str = "https://github.com/storytold/photocraft/issues";

/// Help-menu link commands: (id, url). Labels live in `menus::UI_COMMANDS`.
pub const COMMANDS: &[(&str, &str)] =
    &[("help.discord", DISCORD), ("help.website", APP_PAGE), ("help.artcraftWebsite", ARTCRAFT_WEBSITE), ("help.github", GITHUB), ("help.reportIssue", ISSUES)];

/// The link commands that lead to ArtCraft: its Discord, its website and PhotoCraft's page on it.
/// Independent builds leave them out of the menus, and running one there is an error.
pub const ARTCRAFT_COMMANDS: &[&str] = &["help.discord", "help.website", "help.artcraftWebsite"];

pub fn url_for(id: &str) -> Option<&'static str> {
    COMMANDS.iter().find(|c| c.0 == id).map(|c| c.1)
}

/// Does a `brand` build offer the command `id`? Everything but [`ARTCRAFT_COMMANDS`] in an
/// independent one.
pub fn offers(brand: Brand, id: &str) -> bool {
    brand.is_official() || !ARTCRAFT_COMMANDS.contains(&id)
}

/// Run the link command `id`: open its URL, or say that this build leaves it out. `None` when `id`
/// isn't a link command.
pub fn run(app: &mut PhotocraftApp, ctx: &egui::Context, id: &str) -> Option<Result<Value, String>> {
    let url = url_for(id)?;
    Some(if offers(app.brand, id) {
        Ok(open(app, ctx, url))
    } else {
        Err(format!("{id} is not in this build: independent builds leave out the ArtCraft links"))
    })
}

/// Open `url` in the system browser (a new tab on the web) and note it in the status bar. Prefers
/// the platform `open_url` service (reliable on Windows/macOS/Linux); falls back to `ctx.open_url`.
pub fn open(app: &mut PhotocraftApp, ctx: &egui::Context, url: &str) -> Value {
    let opened = app.services.open_url.as_ref().map(|f| f(url).is_ok()).unwrap_or(false);
    if !opened {
        ctx.open_url(egui::OpenUrl::new_tab(url));
    }
    let template = "Opened {url}";
    app.ui.status = crate::i18n::fmt(tl!(template), &[("url", url)]);
    json!({"url": url})
}

/// The prominent "Join us on Discord" button.
pub fn discord_button(app: &mut PhotocraftApp, ui: &mut egui::Ui, min_width: f32) -> egui::Response {
    let r = crate::widgets::primary_button(ui, tl!("Join us on Discord"), min_width).on_hover_text(DISCORD);
    if r.clicked() {
        open(app, ui.ctx(), DISCORD);
    }
    r
}

/// The links under the About text and under the start screen's buttons, `gap` apart. Official
/// builds: the Discord button (`button_width` wide), then "PhotoCraft website · GitHub · ArtCraft".
/// Independent builds: a plain-text credit, all the brand licence lets a modified version say about
/// ArtCraft, then the build's maintainer ([`crate::brand::INDEPENDENT_MAINTAINER`]) and PhotoCraft's
/// source.
pub fn community(app: &mut PhotocraftApp, ui: &mut egui::Ui, button_width: f32, gap: f32) {
    if app.brand.is_official() {
        discord_button(app, ui, button_width);
        ui.add_space(gap);
        link_row(app, ui, &[(tl!("PhotoCraft website"), APP_PAGE), (tl!("GitHub"), GITHUB), (tl!("ArtCraft"), ARTCRAFT_WEBSITE)]);
    } else {
        let t = crate::theme::Tokens::get(ui.ctx());
        ui.label(egui::RichText::new(tl!("An independent build, based on PhotoCraft by the ArtCraft team.")).color(t.text_dim).size(12.5));
        ui.add_space(gap);
        let (maintainer, page) = crate::brand::INDEPENDENT_MAINTAINER;
        link_row(app, ui, &[(maintainer, page), (tl!("PhotoCraft on GitHub"), GITHUB)]);
    }
}

/// `links` as (label, url) links, centred, separated by dots. Clicks route through [`open`] (the
/// platform browser service) rather than `ui.hyperlink_to`, which uses the unreliable `ctx.open_url`.
fn link_row(app: &mut PhotocraftApp, ui: &mut egui::Ui, links: &[(&str, &str)]) {
    let t = crate::theme::Tokens::get(ui.ctx());
    let font = egui::FontId::proportional(12.5);
    let sep = "  ·  ";
    let width: f32 = links.iter().map(|(l, _)| ui.painter().layout_no_wrap((*l).into(), font.clone(), t.text).size().x).sum::<f32>()
        + links.len().saturating_sub(1) as f32 * ui.painter().layout_no_wrap(sep.into(), font.clone(), t.text).size().x;
    let mut clicked: Option<&str> = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
        for (i, (label, url)) in links.iter().enumerate() {
            if i > 0 {
                ui.label(egui::RichText::new(sep).font(font.clone()).color(t.text_faint));
            }
            if ui.link(egui::RichText::new(*label).font(font.clone()).color(t.accent)).on_hover_text(*url).clicked() {
                clicked = Some(url);
            }
        }
    });
    if let Some(url) = clicked {
        open(app, ui.ctx(), url);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_open_through_the_platform_service() {
        // Issue #14: links (Help menu, Discord button, start-page links) must open in the browser.
        // They route through the `open_url` service rather than the unreliable `ctx.open_url`.
        use std::sync::{Arc, Mutex};
        let opened: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let rec = opened.clone();
        let services = crate::Services {
            open_url: Some(Box::new(move |u: &str| {
                rec.lock().unwrap().push(u.to_string());
                Ok(())
            })),
            ..Default::default()
        };
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), services);
        let ctx = egui::Context::default();
        // Every Help-menu link id reaches the service with its URL.
        let brand = app.brand;
        for (id, url) in COMMANDS.iter().filter(|c| offers(brand, c.0)) {
            crate::menus::invoke(&mut app, &ctx, id, json!({})).unwrap();
            assert_eq!(opened.lock().unwrap().last().map(String::as_str), Some(*url), "{id}");
        }
        // The direct open() helper (Discord button / start-page links) also uses it.
        open(&mut app, &ctx, DISCORD);
        assert_eq!(opened.lock().unwrap().last().map(String::as_str), Some(DISCORD));
    }

    #[test]
    fn help_menu_lists_links_then_separator_then_system_info_and_about() {
        let app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
        let help: Vec<String> = crate::menus::menu_items(&app).into_iter().filter(|i| i.path == ["Help"]).map(|i| i.id).collect();
        let all = ["help.discord", "help.website", "help.artcraftWebsite", "help.github", "help.reportIssue", "---", "help.systemInfo", "help.about"];
        // Independent builds leave the ArtCraft links out (brand_tests.rs).
        assert_eq!(help, all.into_iter().filter(|id| offers(app.brand, id)).collect::<Vec<_>>());
        for (id, _) in COMMANDS.iter().filter(|c| offers(app.brand, c.0)) {
            assert!(crate::menus::is_live(id) && crate::menus::is_enabled(&app, id), "{id}");
        }
    }

    #[test]
    fn link_commands_open_their_urls() {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        for (id, url) in [
            ("help.discord", "https://discord.gg/artcraft"),
            ("help.website", "https://getartcraft.com/apps/photocraft"),
            ("help.github", "https://github.com/storytold/photocraft"),
        ]
        .into_iter()
        .filter(|(id, _)| offers(Brand::BUILD, id))
        {
            let r = crate::menus::invoke(&mut app, &ctx, id, serde_json::json!({})).unwrap();
            assert_eq!(r["url"], url);
            assert_eq!(app.ui.status, format!("Opened {url}"));
        }
    }
}
