//! The steps that aren't software: the introduction, appearance, the computer and
//! desktop, default apps, accounts, and updates, backup and recovery.

use super::{group, heading, settings_row};
use crate::live;
use crate::window::Ui;
use gtk::prelude::*;
use nyx_system_proto::valid_hostname;
use nyx_ui::widgets as w;
use std::rc::Rc;

pub fn intro(ui: &Rc<Ui>) -> gtk::Widget {
    let page = w::vbox(18);
    let logo = w::logo(72);
    logo.set_halign(gtk::Align::Start);
    page.append(&logo);
    let name = live::real_name(&ui.user);
    let first = name.split_whitespace().next().unwrap_or(&name).to_string();
    let again = ui.saved.borrow().done;
    page.append(&heading(
        &if again { "Welcome back".to_string() } else { format!("Welcome to NyxOS, {first}") },
        if again {
            "Change how NyxOS looks, add software, or set up what you skipped last time. Nothing changes until you apply it on the last step."
        } else {
            "A few steps to make this computer yours: how it looks, the software you want, and your accounts. Every step but the last can be skipped, and Welcome is in the launcher if you want it again."
        },
    ));
    let how = group("How changes are made");
    how.append(&w::row(
        "Right away, for you",
        "Your theme, default apps and accounts change as you choose them.",
        Some(gtk::Image::from_icon_name("media-playback-start-symbolic").upcast_ref()),
    ));
    how.append(&w::row(
        "Together, at the end",
        "The computer's name, time zone, keyboard and software for everyone are part of the system configuration. Review shows them all, and they're applied in one go: NyxOS builds the new system first and only switches to it when it works.",
        Some(gtk::Image::from_icon_name("view-refresh-symbolic").upcast_ref()),
    ));
    how.append(&w::row(
        "Always reversible",
        "Each change is a new system generation. An earlier one is in the boot menu, and in Settings.",
        Some(gtk::Image::from_icon_name("edit-undo-symbolic").upcast_ref()),
    ));
    page.append(&how.root);
    page.upcast()
}

pub fn appearance(ui: &Rc<Ui>) -> gtk::Widget {
    let page = w::vbox(18);
    page.append(&heading("Appearance", "Pick a theme. Nyx apps and the desktop change as you click."));
    let current = nyx_theme::read_suite().map(|s| s.theme).unwrap_or_default();
    let lock = Rc::new(std::cell::Cell::new(true));

    let grid = gtk::FlowBox::new();
    grid.add_css_class("welcome-themes");
    grid.set_selection_mode(gtk::SelectionMode::None);
    grid.set_homogeneous(true);
    grid.set_max_children_per_line(4);
    grid.set_min_children_per_line(2);
    grid.set_column_spacing(10);
    grid.set_row_spacing(10);
    for theme in nyx_theme::all() {
        let card = gtk::Button::new();
        card.add_css_class("welcome-theme");
        if theme.id == current {
            card.add_css_class("selected");
        }
        let inner = w::vbox(8);
        let preview = w::hbox(0);
        preview.add_css_class("welcome-theme-preview");
        w::paint(&preview, &theme.palette.bg);
        let surface = w::vbox(4);
        surface.add_css_class("welcome-theme-surface");
        w::paint(&surface, &theme.palette.surface);
        surface.set_hexpand(true);
        for (colour, width) in [(&theme.palette.text, 60), (&theme.palette.dim_text, 40)] {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            line.add_css_class("welcome-theme-line");
            line.set_size_request(width, 6);
            line.set_halign(gtk::Align::Start);
            w::paint(&line, colour);
            surface.append(&line);
        }
        let accent = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        accent.add_css_class("welcome-theme-accent");
        accent.set_size_request(28, 10);
        accent.set_halign(gtk::Align::Start);
        w::paint(&accent, &theme.palette.accent);
        surface.append(&accent);
        preview.append(&surface);
        inner.append(&preview);
        inner.append(&w::label(&theme.name, "welcome-theme-name"));
        card.set_child(Some(&inner));
        let (u, id, lock) = (ui.clone(), theme.id.clone(), lock.clone());
        card.connect_clicked(move |_| {
            live::set_theme(&id, lock.get());
            let mut c = u.choices.borrow_mut();
            if c.screens_theme.is_some() {
                c.screens_theme = Some(id.clone());
            }
            drop(c);
            u.rebuild();
        });
        grid.insert(&card, -1);
    }
    page.append(&grid);

    let more = group("");
    let l = lock.clone();
    more.append(
        &w::switch_row("Use it for the lock screen too", "The lock screen shows the theme you pick here.", true, move |on| {
            l.set(on)
        })
        .0,
    );
    let u = ui.clone();
    let chosen = current.clone();
    more.append(
        &w::switch_row(
            "Use it for the boot and sign-in screens",
            "For everyone on this computer. A system setting: applied with the rest at the end.",
            ui.choices.borrow().screens_theme.is_some(),
            move |on| u.choices.borrow_mut().screens_theme = on.then(|| chosen.clone()),
        )
        .0,
    );
    let density = nyx_theme::read_suite().map(|s| s.density).filter(|d| !d.is_empty()).unwrap_or_else(|| "comfortable".into());
    more.append(&w::segmented_row(
        "Density",
        "How much space Nyx apps leave around things.",
        w::opts(&[("comfortable", "Comfortable"), ("compact", "Compact")]),
        &density,
        |d| live::set_density(&d),
    ));
    more.append(&settings_row("More", "Fonts, accents, icons and the wallpaper are in Settings.", "Open Settings", "theme"));
    page.append(&more.root);
    page.upcast()
}

pub fn computer(ui: &Rc<Ui>) -> gtk::Widget {
    let page = w::vbox(18);
    page.append(&heading(
        "Computer and desktop",
        "The computer's name, time zone and keyboard are system settings: they're applied together with your software at the end.",
    ));
    let c = ui.choices.borrow().clone();
    let this = group("This computer");

    let entry = gtk::Entry::new();
    entry.set_text(c.hostname.as_deref().unwrap_or(&live::hostname()));
    entry.set_width_chars(22);
    let note = w::label("", "welcome-error");
    note.set_visible(false);
    let (u, n, now) = (ui.clone(), note.clone(), live::hostname());
    entry.connect_changed(move |e| {
        let text = e.text().trim().to_string();
        let ok = valid_hostname(&text);
        n.set_visible(!ok);
        n.set_text("Letters, digits and dashes, not at either end, up to 63");
        u.choices.borrow_mut().hostname = (ok && text != now).then_some(text);
    });
    let name_row = w::row("Name", "How this computer shows on your network.", Some(entry.upcast_ref()));
    this.append(&name_row);
    this.append(&note);

    let zones = live::time_zones();
    let tz_now = c.time_zone.clone().unwrap_or_else(live::time_zone);
    let strings: Vec<&str> = zones.iter().map(String::as_str).collect();
    let tz = gtk::DropDown::from_strings(&strings);
    tz.set_enable_search(true);
    tz.set_expression(Some(gtk::PropertyExpression::new(gtk::StringObject::static_type(), None::<gtk::Expression>, "string")));
    if let Some(i) = zones.iter().position(|z| *z == tz_now) {
        tz.set_selected(i as u32);
    }
    let (u, now) = (ui.clone(), live::time_zone());
    tz.connect_selected_notify(move |d| {
        if let Some(z) = zones.get(d.selected() as usize) {
            u.choices.borrow_mut().time_zone = (*z != now).then(|| z.clone());
        }
    });
    this.append(&w::row("Time zone", "", Some(tz.upcast_ref())));

    let kb_now = c.keyboard.clone().unwrap_or_else(live::keyboard);
    let mut layouts: Vec<(String, String)> =
        live::KEYBOARDS.iter().map(|(id, name)| (id.to_string(), name.to_string())).collect();
    if !layouts.iter().any(|(id, _)| *id == kb_now) {
        layouts.insert(0, (kb_now.clone(), kb_now.clone()));
    }
    let names: Vec<&str> = layouts.iter().map(|(_, n)| n.as_str()).collect();
    let kb = gtk::DropDown::from_strings(&names);
    if let Some(i) = layouts.iter().position(|(id, _)| *id == kb_now) {
        kb.set_selected(i as u32);
    }
    let (u, now) = (ui.clone(), live::keyboard());
    kb.connect_selected_notify(move |d| {
        if let Some((id, _)) = layouts.get(d.selected() as usize) {
            u.choices.borrow_mut().keyboard = (*id != now).then(|| id.clone());
        }
    });
    this.append(&w::row("Keyboard", "For the desktop and the console.", Some(kb.upcast_ref())));
    page.append(&this.root);

    let screens = group("Screens");
    let found = live::screens();
    if found.is_empty() {
        screens.append(&w::row("No screens found", "Hyprland didn't list any.", None));
    }
    for s in found {
        let (name, detail) = s.split_once(": ").unwrap_or((&s, ""));
        screens.append(&w::row(name, detail, None));
    }
    screens.append(&settings_row(
        "Arrange and scale",
        "Position, resolution, scale and refresh rate change right away.",
        "Open Displays",
        "displays",
    ));
    page.append(&screens.root);

    let desk = group("Desktop");
    desk.append(&settings_row("Bar and notifications", "What the top bar shows.", "Open", "bar"));
    desk.append(&settings_row("Dock", "Pinned apps, size and hiding.", "Open", "dock"));
    desk.append(&settings_row("Tiling and workspaces", "How windows are arranged, and workspace rules.", "Open", "workspaces"));
    desk.append(&settings_row("Keyboard shortcuts", "Every shortcut, and your own.", "Open", "keybindings"));
    desk.append(&settings_row("Mouse and trackpad", "Speed, scrolling and gestures.", "Open", "trackpad"));
    page.append(&desk.root);
    page.upcast()
}

pub fn defaults(_ui: &Rc<Ui>) -> gtk::Widget {
    use nyx_ui::default_apps::{self as defaults, CATEGORIES, WEB_BROWSER};
    let page = w::vbox(18);
    page.append(&heading(
        "Default apps",
        "What opens links, mail and files. This changes right away. Apps you're installing in this setup can be chosen afterwards in Settings.",
    ));
    let none = "Nothing installed opens these yet.";
    let main = group("Defaults");
    let row = |title: &str, desc: &str, (options, current): (Vec<(String, String)>, String), set: Box<dyn Fn(&str)>| {
        if options.is_empty() {
            return w::row(title, none, None);
        }
        w::choice_row(title, desc, options, &current, move |id| set(&id)).0
    };
    main.append(&row(WEB_BROWSER.title, WEB_BROWSER.desc, defaults::options(&WEB_BROWSER), Box::new(|id| {
        let _ = defaults::set(&WEB_BROWSER, id);
    })));
    main.append(&row("Terminal", "Opens with Super Enter.", defaults::terminals(), Box::new(|id| {
        let _ = defaults::set_terminal(id);
    })));
    main.append(&row("Text editor", "For config files and the terminal.", defaults::editors(), Box::new(|id| {
        let _ = defaults::set_editor(id);
    })));
    page.append(&main.root);
    let files = group("Files and links");
    for c in CATEGORIES {
        files.append(&row(c.title, c.desc, defaults::options(c), Box::new(move |id| {
            let _ = defaults::set(c, id);
        })));
    }
    page.append(&files.root);
    page.append(&settings_row("More", "Every default in one place.", "Open Default Apps", "apps"));
    page.upcast()
}

pub fn accounts(ui: &Rc<Ui>) -> gtk::Widget {
    let page = w::vbox(18);
    page.append(&heading("Accounts and sign-in", "Your online accounts, and signing in with your face or a fingerprint. These open in Settings and take effect right away."));
    let me = group("You");
    me.append(&w::row(
        &live::real_name(&ui.user),
        &format!("Signed in as {}", ui.user),
        Some(gtk::Image::from_icon_name("avatar-default-symbolic").upcast_ref()),
    ));
    me.append(&settings_row("Your account", "Your name, picture and password.", "Open", "accounts"));
    page.append(&me.root);
    let online = group("Online accounts");
    online.append(&settings_row(
        "Mail, calendars, contacts and files",
        "Add Google, Microsoft, Nextcloud, IMAP and other accounts once, for every Nyx app.",
        "Add Account",
        "online-accounts",
    ));
    page.append(&online.root);
    let bio = group("Biometrics");
    bio.append(&settings_row(
        "Face and fingerprint",
        "Sign in, unlock and approve system changes without typing your password. Your password still works.",
        "Set Up",
        "sign-in",
    ));
    page.append(&bio.root);
    page.upcast()
}

pub fn care(ui: &Rc<Ui>) -> gtk::Widget {
    let page = w::vbox(18);
    page.append(&heading(
        "Updates, backup and recovery",
        "Keep NyxOS current, your files safe, and a way back if something goes wrong.",
    ));
    let updates = group("Updates");
    let u = ui.clone();
    updates.append(
        &w::switch_row(
            "Update NyxOS when setup finishes",
            "Gets the latest NyxOS, apps and packages, then switches to them. It can take a while the first time.",
            ui.choices.borrow().update_after,
            move |on| u.choices.borrow_mut().update_after = on,
        )
        .0,
    );
    updates.append(&settings_row(
        "Later",
        "Settings → Software checks for updates and applies them.",
        "Open Software",
        "software",
    ));
    page.append(&updates.root);

    let backup = group("Backup");
    if live::has_program("nyx-backup") {
        backup.append(
            &w::button_row(
                "Nyx Backup",
                "Back up your home folder on a schedule, to a disk, a server or the cloud.",
                "Set Up",
                |_| {
                    live::launch("nyx-backup");
                },
            )
            .0,
        );
    } else {
        backup.append(&w::row("Nyx Backup", "Isn't installed. Add it under Nyx apps.", None));
    }
    page.append(&backup.root);

    let recovery = group("Recovery");
    let (count, current) = live::generations();
    recovery.append(&w::row(
        "System generations",
        &match current {
            Some(n) => format!("This is generation {n}; {count} are kept. Every system change makes a new one."),
            None => "Every system change makes a new generation.".into(),
        },
        None,
    ));
    recovery.append(&w::row(
        "If something goes wrong",
        "Pick an earlier generation in the boot menu, or go back to the previous one in Settings → Software (or with <tt>nyx-system rollback</tt>). Your files and settings stay as they are.",
        None,
    ));
    page.append(&recovery.root);
    page.upcast()
}
