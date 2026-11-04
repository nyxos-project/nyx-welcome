//! Nyx Welcome: NyxOS's setup after installation. It opens once at the first sign-in
//! and can be opened again any time from the launcher.

mod live;
mod state;
mod steps;
mod system;
mod window;

use gtk::prelude::*;

pub const APP_ID: &str = "io.github.design_nexus.nyx.Welcome";

fn prefs() -> nyx_theme::ThemePrefs {
    let suite = nyx_theme::read_suite().unwrap_or_default();
    nyx_theme::ThemePrefs { mode: suite.mode, theme: suite.theme, glow: false, reduce_motion: false }
}

fn build(app: &gtk::Application, step: Option<String>, select: Vec<String>) {
    if let Some(w) = app.active_window() {
        w.present();
        return;
    }
    nyx_theme::use_design_2();
    nyx_theme::install(nyx_theme::Style {
        window_class: "welcome-window",
        base_skip: &[],
        css: include_str!("style.css"),
        no_glow_css: "",
        extra_colors: None,
        prefs,
        set_theme: Some(|mode, theme| {
            nyx_theme::update_suite(|s| {
                s.mode = mode;
                s.theme = theme;
            })
        }),
    });
    let ui = window::build(app, step.as_deref());
    ui.choices.borrow_mut().selected.extend(select.into_iter().filter(|id| ui.catalog.item(id).is_some()));
    ui.rebuild();
    let keep = ui.clone();
    ui.window.connect_close_request(move |_| {
        let _ = &keep;
        gtk::glib::Propagation::Proceed
    });
    ui.window.present();
}

fn main() -> gtk::glib::ExitCode {
    nyx_core::app::init(nyx_core::app::App { slug: "nyx-welcome", name: "Welcome" });
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help") {
        println!(
            "Usage: nyx-welcome [--first-run] [--step ID] [--select ITEM,...]\n\n  --first-run  open only if setup hasn't been finished yet (the first sign-in)\n  --select     start with these catalog items chosen"
        );
        return gtk::glib::ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--first-run") && state::Saved::load().done {
        return gtk::glib::ExitCode::SUCCESS;
    }
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let step = value("--step");
    let select: Vec<String> = value("--select").map(|s| s.split(',').map(str::to_string).collect()).unwrap_or_default();
    // NYX_WELCOME_APP_ID runs a test copy under its own ID, beside the real one.
    let id = std::env::var("NYX_WELCOME_APP_ID").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| APP_ID.to_string());
    gtk::glib::set_prgname(Some(id.as_str()));
    let app = gtk::Application::builder().application_id(id.as_str()).flags(gtk::gio::ApplicationFlags::NON_UNIQUE).build();
    app.connect_activate(move |app| build(app, step.clone(), select.clone()));
    app.run_with_args::<&str>(&[])
}
