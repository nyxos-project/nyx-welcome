//! The steps, in order, and what each page shows.

mod basics;
mod review;
mod software;

use crate::window::Ui;
use gtk::prelude::*;
use nyx_ui::widgets as w;
use std::rc::Rc;

pub use software::{basket, picked};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Intro,
    Appearance,
    Computer,
    Presets,
    /// Every section of the software catalog, a tab each.
    Software,
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
    /// Its place in [`GROUPS`].
    pub group: usize,
}

/// The sidebar's headings, with their sections' colours.
pub const GROUPS: &[(&str, &str)] = &[
    ("Make it yours", nyx_ui::sidebar::ACCENT_SECTION),
    ("Software", "swatch-green"),
    ("You", "swatch-violet"),
    ("Finish", "swatch-amber"),
];

const fn step(id: &'static str, title: &'static str, icon: &'static str, kind: Kind, group: usize) -> Step {
    Step { id, title, icon, kind, optional: true, group }
}

pub const STEPS: &[Step] = &[
    Step { optional: false, ..step("welcome", "Welcome", "starred-symbolic", Kind::Intro, 0) },
    step(
        "appearance",
        "Appearance",
        "preferences-desktop-appearance-symbolic|applications-graphics-symbolic",
        Kind::Appearance,
        0,
    ),
    step("computer", "Computer and desktop", "video-display-symbolic|computer-symbolic", Kind::Computer, 0),
    step("presets", "Starting point", "media-playback-start-symbolic", Kind::Presets, 1),
    step("software", "Choose software", "view-app-grid-symbolic", Kind::Software, 1),
    step("defaults", "Default apps", "power-profile-performance-symbolic", Kind::Defaults, 2),
    step("accounts", "Accounts and sign-in", "avatar-default-symbolic|system-users-symbolic", Kind::Accounts, 2),
    step("care", "Updates and recovery", "security-high-symbolic|software-update-available-symbolic", Kind::Care, 3),
    Step {
        optional: false,
        ..step("review", "Review and apply", "emblem-default-symbolic|object-select-symbolic", Kind::Review, 3)
    },
];

/// The step `id` names, and the software tab when it names a catalog section (the
/// software steps were one per section before they became one page).
pub fn find(catalog: &nyx_catalog::Catalog, id: &str) -> Option<(usize, Option<String>)> {
    if let Some(i) = STEPS.iter().position(|s| s.id == id) {
        return Some((i, None));
    }
    let section = catalog.section(id)?;
    STEPS.iter().position(|s| s.kind == Kind::Software).map(|i| (i, Some(section.id.clone())))
}

pub fn build(ui: &Rc<Ui>, kind: Kind) -> gtk::Widget {
    match kind {
        Kind::Intro => basics::intro(ui),
        Kind::Appearance => basics::appearance(ui),
        Kind::Computer => basics::computer(ui),
        Kind::Presets => software::presets(ui),
        Kind::Software => software::page(ui),
        Kind::Defaults => basics::defaults(ui),
        Kind::Accounts => basics::accounts(ui),
        Kind::Care => basics::care(ui),
        Kind::Review => review::build(ui),
    }
}

/// When the step's changes happen, for the footer: icons (as for [`icon`]) and a few words.
pub fn note(kind: Kind) -> Option<(&'static str, &'static str)> {
    const LIVE: (&str, &str) = ("power-profile-performance-symbolic", "Changes right away, for you");
    match kind {
        Kind::Appearance | Kind::Defaults | Kind::Accounts => Some(LIVE),
        Kind::Computer => Some(("package-x-generic-symbolic", "Applied at Review, with the rest")),
        Kind::Software => Some(("package-x-generic-symbolic", "Installed at Review, all together")),
        _ => None,
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
        Kind::Software => {
            let ids: Vec<String> = ui
                .catalog
                .sections
                .iter()
                .flat_map(|s| ui.catalog.items_in(&s.id, &[nyx_catalog::Tier::Welcome, nyx_catalog::Tier::Store]))
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

/// An icon from the first of `names` ("a|b") the icon theme has: Umbra's, then Adwaita's.
pub fn icon(names: &str) -> gtk::Image {
    let names: Vec<&str> = names.split('|').collect();
    gtk::Image::from_gicon(&gtk::gio::ThemedIcon::from_names(&names))
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

/// A group of rows in one card under a small title, as in Nyx Settings.
pub struct Group {
    pub root: gtk::Box,
    list: gtk::Box,
}

impl Group {
    pub fn append(&self, row: &impl IsA<gtk::Widget>) {
        self.list.append(row);
    }
}

pub fn group(title: &str) -> Group {
    let root = w::vbox(0);
    root.add_css_class("settings-group");
    if !title.is_empty() {
        root.append(&w::label(&title.to_uppercase(), "group-title"));
    }
    let list = w::vbox(0);
    list.add_css_class("group-list");
    root.append(&list);
    Group { root, list }
}

/// A row with a button that opens a page of Nyx Settings.
pub fn settings_row(title: &str, desc: &str, label: &str, section: &'static str) -> gtk::Box {
    w::button_row(title, desc, label, move |_| crate::live::open_settings(section)).0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_unique_and_grouped_in_order() {
        let mut ids = std::collections::BTreeSet::new();
        for s in STEPS {
            assert!(ids.insert(s.id), "{} is listed twice", s.id);
            assert!(s.group < GROUPS.len(), "{} has no group", s.id);
        }
        assert!(STEPS.windows(2).all(|w| w[1].group == w[0].group || w[1].group == w[0].group + 1));
        assert_eq!(STEPS.last().map(|s| s.group), Some(GROUPS.len() - 1));
        assert_eq!(STEPS.last().map(|s| s.kind), Some(Kind::Review));
        assert!(!STEPS[0].optional && !STEPS[STEPS.len() - 1].optional);
    }

    #[test]
    fn a_section_opens_its_tab() {
        let catalog = nyx_catalog::Catalog::builtin().unwrap();
        let software = STEPS.iter().position(|s| s.kind == Kind::Software).unwrap();
        for section in &catalog.sections {
            assert!(!ids_clash(&section.id), "{} is a step and a section", section.id);
            assert_eq!(find(&catalog, &section.id), Some((software, Some(section.id.clone()))));
        }
        assert_eq!(find(&catalog, "review"), Some((STEPS.len() - 1, None)));
        assert_eq!(find(&catalog, "nothing"), None);
    }

    fn ids_clash(id: &str) -> bool {
        STEPS.iter().any(|s| s.id == id)
    }
}
