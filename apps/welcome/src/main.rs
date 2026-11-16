//! Nyx Welcome: NyxOS's setup after installation. It opens once at the first sign-in
//! and can be opened again any time from the launcher.

mod live;
mod state;
mod steps;
mod system;
mod window;
mod wrap;

use gtk::prelude::*;

pub const APP_ID: &str = "io.github.design_nexus.nyx.Welcome";

fn prefs() -> nyx_theme::ThemePrefs {
    let suite = nyx_theme::read_suite().unwrap_or_default();
    nyx_theme::ThemePrefs { mode: suite.mode, theme: suite.theme, glow: false, reduce_motion: false }
}

thread_local! {
    static UI: std::cell::RefCell<Option<std::rc::Rc<window::Ui>>> = const { std::cell::RefCell::new(None) };
}

/// What a launch asks for: `--step`, `--select`, `--preset`.
struct Request {
    step: Option<String>,
    select: Vec<String>,
    presets: Vec<String>,
}

fn request(args: &[String]) -> Request {
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let list =
        |flag: &str| -> Vec<String> { value(flag).map(|s| s.split(',').map(str::to_string).collect()).unwrap_or_default() };
    Request { step: value("--step"), select: list("--select"), presets: list("--preset") }
}

/// Add `select` and `presets` to the choices.
fn choose(ui: &window::Ui, select: Vec<String>, presets: Vec<String>) {
    let mut c = ui.choices.borrow_mut();
    c.selected.extend(select.into_iter().filter(|id| ui.catalog.item(id).is_some()));
    for id in presets {
        if let Ok(items) = ui.catalog.presets(&[&id]) {
            c.selected.extend(items);
            c.presets.insert(id);
        }
    }
}

/// Open Welcome as `r` asks, or move the open one there (Settings' and the Store's buttons
/// open it at a step while it may already be up).
fn open(app: &gtk::Application, r: Request) {
    if let Some(ui) = UI.with(|u| u.borrow().clone())
        && app.active_window().is_some()
    {
        let asked = r.step.is_some() || !r.select.is_empty() || !r.presets.is_empty();
        if asked && !ui.busy.get() {
            choose(&ui, r.select, r.presets);
            ui.rebuild();
            if let Some(step) = &r.step {
                ui.go_to(step);
            }
        }
        ui.window.present();
        return;
    }
    build(app, r.step, r.select, r.presets);
}

fn build(app: &gtk::Application, step: Option<String>, select: Vec<String>, presets: Vec<String>) {
    nyx_theme::use_design_2();
    nyx_theme::install(nyx_theme::Style {
        window_class: "welcome-window",
        base_skip: &[],
        css: include_str!("style.css"),
        no_glow_css: "",
        extra_colors: None,
        prefs,
        set_theme: None,
    });
    let ui = window::build(app, step.as_deref());
    choose(&ui, select, presets);
    ui.rebuild();
    UI.with(|u| *u.borrow_mut() = Some(ui.clone()));
    let keep = ui.clone();
    ui.window.connect_close_request(move |window| {
        if !keep.busy.get() {
            return gtk::glib::Propagation::Proceed;
        }
        // Closing now would drop the app installs that haven't started.
        let dialog = gtk::AlertDialog::builder()
            .modal(true)
            .message("Welcome is still setting up")
            .detail("Closing now stops the app installs that haven't started yet. Changes to the system already underway finish on their own.")
            .buttons(["Keep going", "Close anyway"])
            .cancel_button(0)
            .default_button(0)
            .build();
        let w = window.clone();
        dialog.choose(Some(window), gtk::gio::Cancellable::NONE, move |answer| {
            if answer == Ok(1) {
                w.destroy();
            }
        });
        gtk::glib::Propagation::Stop
    });
    ui.window.present();
}

fn main() -> gtk::glib::ExitCode {
    nyx_core::app::init(nyx_core::app::App { slug: "nyx-welcome", name: "Welcome" });
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help") {
        println!(
            "Usage: nyx-welcome [--first-run] [--step ID] [--select ITEM,...] [--preset ID,...]\n\n  --first-run  open only if setup hasn't been finished yet (the first sign-in)\n  --step       open at this step, or at this section of the software page\n  --select     start with these catalog items chosen\n  --preset     start with these starting points on"
        );
        return gtk::glib::ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--first-run") && state::Saved::load().done {
        return gtk::glib::ExitCode::SUCCESS;
    }
    // NYX_WELCOME_APP_ID runs a test copy under its own ID, beside the real one.
    let id = std::env::var("NYX_WELCOME_APP_ID").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| APP_ID.to_string());
    gtk::glib::set_prgname(Some(id.as_str()));
    // One Welcome: opening it again (the launcher while the first-run window is up)
    // brings the open one forward, and a step or choices given move it there.
    let app = gtk::Application::builder()
        .application_id(id.as_str())
        .flags(gtk::gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    app.connect_activate(|app| open(app, request(&[])));
    app.connect_command_line(|app, cmd| {
        let args: Vec<String> = cmd.arguments().iter().skip(1).map(|a| a.to_string_lossy().into_owned()).collect();
        open(app, request(&args));
        gtk::glib::ExitCode::SUCCESS
    });
    app.run()
}

#[cfg(test)]
#[test]
fn style_uses_only_defined_colours() {
    for theme in nyx_theme::all() {
        let sheet = nyx_theme::sheet_for_check("welcome-window", &theme, true) + include_str!("style.css");
        assert_eq!(nyx_theme::undefined_colors(&sheet), Vec::<String>::new(), "{}", theme.id);
    }
}
