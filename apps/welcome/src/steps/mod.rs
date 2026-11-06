//! The steps, in order, and what each page shows.

mod basics;
mod review;
mod software;

use crate::window::Ui;
use gtk::prelude::*;
use nyx_ui::widgets as w;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Intro,
    Appearance,
    Computer,
    Presets,
    /// A section of the software catalog.
    Software(&'static str),
    Defaults,
    Accounts,
    Care,
    Review,
}

pub struct Step {
    pub id: &'static str,
    pub title: &'static str,
    pub icon: &'static str,
    pub kind: Kind,
    pub optional: bool,
}

const fn step(id: &'static str, title: &'static str, icon: &'static str, kind: Kind) -> Step {
    Step { id, title, icon, kind, optional: true }
}

pub const STEPS: &[Step] = &[
    Step { optional: false, ..step("welcome", "Welcome", "starred-symbolic", Kind::Intro) },
    step("appearance", "Appearance", "applications-graphics-symbolic", Kind::Appearance),
    step("computer", "Computer and desktop", "computer-symbolic", Kind::Computer),
    step("presets", "Starting point", "view-grid-symbolic", Kind::Presets),
    step("nyx-apps", "Nyx apps", "view-app-grid-symbolic", Kind::Software("nyx-apps")),
    step("desktop", "Apps", "system-software-install-symbolic", Kind::Software("desktop")),
    step("cli", "Command-line tools", "utilities-terminal-symbolic", Kind::Software("cli")),
    step("toolchains", "Development", "applications-engineering-symbolic", Kind::Software("toolchains")),
    step("infrastructure", "Infrastructure", "network-server-symbolic", Kind::Software("infrastructure")),
    step("agents", "AI agents", "chat-message-new-symbolic", Kind::Software("agents")),
    step("defaults", "Default apps", "preferences-desktop-apps-symbolic", Kind::Defaults),
    step("accounts", "Accounts and sign-in", "system-users-symbolic", Kind::Accounts),
    step("care", "Updates and recovery", "software-update-available-symbolic", Kind::Care),
    Step { optional: false, ..step("review", "Review and apply", "view-list-bullet-symbolic", Kind::Review) },
];

pub fn build(ui: &Rc<Ui>, kind: Kind) -> gtk::Widget {
    match kind {
        Kind::Intro => basics::intro(ui),
        Kind::Appearance => basics::appearance(ui),
        Kind::Computer => basics::computer(ui),
        Kind::Presets => software::presets(ui),
        Kind::Software(section) => software::section(ui, section),
        Kind::Defaults => basics::defaults(ui),
        Kind::Accounts => basics::accounts(ui),
        Kind::Care => basics::care(ui),
        Kind::Review => review::build(ui),
    }
}

/// Skipping a step leaves out what was chosen in it.
pub fn skip(ui: &Rc<Ui>, kind: Kind) {
    let mut c = ui.choices.borrow_mut();
    match kind {
        Kind::Computer => {
            c.hostname = None;
            c.time_zone = None;
            c.keyboard = None;
        }
        Kind::Appearance => c.screens_theme = None,
        Kind::Presets => c.presets.clear(),
        Kind::Software(section) => {
            let ids: Vec<String> = ui
                .catalog
                .items_in(section, &[nyx_catalog::Tier::Welcome, nyx_catalog::Tier::Store])
                .map(|i| i.id.clone())
                .collect();
            for id in ids {
                c.selected.remove(&id);
            }
        }
        Kind::Care => c.update_after = false,
        _ => {}
    }
}

/// A page's heading: its title and what it's for.
pub fn heading(title: &str, text: &str) -> gtk::Box {
    let b = w::vbox(6);
    b.add_css_class("welcome-heading");
    b.append(&w::label(title, "welcome-title"));
    if !text.is_empty() {
        let l = w::label(text, "welcome-lede");
        l.set_wrap(true);
        b.append(&l);
    }
    b
}

/// A group of rows under a small title.
pub fn group(title: &str) -> gtk::Box {
    let b = w::vbox(6);
    b.add_css_class("welcome-group");
    if !title.is_empty() {
        b.append(&w::label(title, "section-title"));
    }
    b
}

/// A row with a button that opens a page of Nyx Settings.
pub fn settings_row(title: &str, desc: &str, label: &str, section: &'static str) -> gtk::Box {
    w::button_row(title, desc, label, move |_| crate::live::open_settings(section)).0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_unique_and_cover_the_catalog() {
        let catalog = nyx_catalog::Catalog::builtin().unwrap();
        let mut ids = std::collections::BTreeSet::new();
        for s in STEPS {
            assert!(ids.insert(s.id), "{} is listed twice", s.id);
            if let Kind::Software(section) = s.kind {
                assert!(catalog.section(section).is_some(), "the catalog has no section {section}");
            }
        }
        for section in &catalog.sections {
            assert!(
                STEPS.iter().any(|s| matches!(s.kind, Kind::Software(x) if x == section.id)),
                "no step offers {}",
                section.id
            );
        }
        assert_eq!(STEPS.last().map(|s| s.kind), Some(Kind::Review));
        assert!(!STEPS[0].optional && !STEPS[STEPS.len() - 1].optional);
    }
}
