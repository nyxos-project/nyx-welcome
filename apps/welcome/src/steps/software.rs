//! The presets, and the software catalog to choose from: one page with a tab per
//! section, and what's picked so far beside it.

use super::{group, heading};
use crate::window::Ui;
use crate::wrap::wrap_box;
use gtk::prelude::*;
use nyx_catalog::{Item, Tier};
use nyx_ui::widgets as w;
use std::rc::Rc;

pub fn presets(ui: &Rc<Ui>) -> gtk::Widget {
    let page = w::vbox(18);
    page.append(&heading(
        "Starting point",
        "Pick any that fit: each checks a set of software on the next page, where you can change every item. Or pick none and choose as you go.",
    ));
    let grid = gtk::FlowBox::new();
    grid.add_css_class("welcome-presets");
    grid.set_selection_mode(gtk::SelectionMode::None);
    grid.set_homogeneous(true);
    grid.set_min_children_per_line(2);
    grid.set_max_children_per_line(2);
    grid.set_column_spacing(12);
    grid.set_row_spacing(12);
    for p in &ui.catalog.presets {
        let card = gtk::ToggleButton::new();
        card.add_css_class("welcome-preset");
        let inner = w::vbox(4);
        inner.set_valign(gtk::Align::Start);
        let top = w::hbox(8);
        let name = w::label(&p.name, "welcome-preset-name");
        name.set_hexpand(true);
        top.append(&name);
        let mark = gtk::Image::from_icon_name("object-select-symbolic");
        mark.add_css_class("welcome-preset-mark");
        top.append(&mark);
        inner.append(&top);
        let summary = w::label(&p.summary, "welcome-preset-summary");
        summary.set_wrap(true);
        summary.set_max_width_chars(34);
        inner.append(&summary);
        let names: Vec<&str> = p.items.iter().filter_map(|i| ui.catalog.item(i)).map(|i| i.name.as_str()).collect();
        let list = w::label(&preset_items(&names), "welcome-preset-items");
        list.set_wrap(true);
        list.set_max_width_chars(34);
        inner.append(&list);
        card.set_tooltip_text(Some(&names.join(", ")));
        card.set_child(Some(&inner));
        let active = ui.choices.borrow().presets.contains(&p.id);
        card.set_active(active);
        mark.set_opacity(if active { 1.0 } else { 0.0 });
        let (u, id) = (ui.clone(), p.id.clone());
        card.connect_toggled(move |b| {
            mark.set_opacity(if b.is_active() { 1.0 } else { 0.0 });
            set_preset(&u, &id, b.is_active());
        });
        grid.insert(&card, -1);
    }
    page.append(&grid);
    page.upcast()
}

/// Turn preset `id` on (checking its items) or off (unchecking those no other preset
/// that's on brings).
fn set_preset(ui: &Rc<Ui>, id: &str, on: bool) {
    let mut c = ui.choices.borrow_mut();
    if on {
        c.presets.insert(id.to_string());
        if let Ok(items) = ui.catalog.presets(&[id]) {
            c.selected.extend(items);
        }
    } else {
        c.presets.remove(id);
        let others: Vec<&str> = c.presets.iter().map(String::as_str).collect();
        let keep = ui.catalog.presets(&others).unwrap_or_default();
        for item in ui.catalog.presets(&[id]).unwrap_or_default() {
            if !keep.contains(&item) {
                c.selected.remove(&item);
            }
        }
    }
}

/// A preset's first few items and how many more, for its card.
fn preset_items(names: &[&str]) -> String {
    const SHOWN: usize = 4;
    match names.len() {
        n if n <= SHOWN => names.join(", "),
        n => format!("{}, and {} more", names[..SHOWN].join(", "), n - SHOWN),
    }
}

/// What an item brings with it, as tags.
fn tags(item: &Item) -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    match item.scope() {
        nyx_catalog::Scope::User => out.push(("For you".into(), "")),
        nyx_catalog::Scope::System => out.push(("For everyone".into(), "")),
    }
    if item.flatpak.is_some() {
        out.push(("Flathub".into(), ""));
    }
    if item.unfree {
        out.push(("Unfree licence".into(), "warning"));
    }
    for s in &item.services {
        out.push((format!("Service: {s}"), ""));
    }
    for g in &item.groups {
        out.push((format!("Group: {g}"), "warning"));
    }
    out
}

fn nyx_agent(args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("nyx").arg("agent").args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The agent `nyx agent` and Super Shift Ctrl A start, from the catalog's coding agents.
/// It's saved at once, so an agent this setup installs can be chosen before it's there.
fn default_agent(ui: &Rc<Ui>) -> super::Group {
    let g = group("Default agent");
    let mut options = vec![("none".to_string(), "None".to_string())];
    options.extend(
        ui.catalog.items.iter().filter(|i| i.category == "coding-agents").filter_map(|i| Some((i.bin.clone()?, i.name.clone()))),
    );
    let current = nyx_agent(&["default"]).filter(|c| !c.is_empty()).unwrap_or_else(|| "none".into());
    let (row, _) = w::choice_row(
        "Coding agent",
        "Opens in a terminal with <tt>Super Shift Ctrl A</tt>, set to work without stopping to ask.",
        options,
        &current,
        |cmd| {
            let _ = nyx_agent(&["default", &cmd]);
        },
    );
    g.append(&row);
    g
}

/// The items chosen that aren't installed yet, in catalog order.
pub fn picked(ui: &Ui) -> Vec<&Item> {
    let (c, have) = (ui.choices.borrow(), ui.have.borrow());
    ui.catalog.items.iter().filter(|i| c.selected.contains(&i.id) && !have.has(i)).collect()
}

/// The first preset that's on and brings `item`.
fn preset_of<'a>(ui: &'a Ui, item: &str) -> Option<&'a str> {
    let c = ui.choices.borrow();
    ui.catalog.presets.iter().find(|p| c.presets.contains(&p.id) && p.items.iter().any(|i| i == item)).map(|p| p.name.as_str())
}

fn lede(ui: &Ui) -> String {
    let c = ui.choices.borrow();
    let on: Vec<&str> = ui.catalog.presets.iter().filter(|p| c.presets.contains(&p.id)).map(|p| p.name.as_str()).collect();
    if on.is_empty() {
        "Check what you want in each section. Nyx Store has the rest of nixpkgs and Flathub any time.".into()
    } else {
        format!(
            "Your starting point ({}) checked some of these. Change anything; Nyx Store has the rest any time.",
            on.join(", ")
        )
    }
}

pub fn page(ui: &Rc<Ui>) -> gtk::Widget {
    let page = w::vbox(14);
    page.append(&heading("Choose software", &lede(ui)));
    let current = ui.section.borrow().clone();
    let picked_ids: Vec<String> = picked(ui).iter().map(|i| i.id.clone()).collect();

    let tabs = wrap_box(6, 6);
    tabs.add_css_class("welcome-tabs");
    let mut counts = Vec::new();
    for s in &ui.catalog.sections {
        let tab = gtk::Button::new();
        tab.add_css_class("welcome-tab");
        if s.id == current {
            tab.add_css_class("on");
        }
        let inner = w::hbox(7);
        inner.append(&gtk::Label::new(Some(&s.name)));
        let count = w::label("", "welcome-tab-count");
        inner.append(&count);
        tab.set_child(Some(&inner));
        tab.set_tooltip_text(Some(&s.summary));
        counts.push((s.id.clone(), count));
        let (u, id) = (ui.clone(), s.id.clone());
        tab.connect_clicked(move |_| {
            *u.section.borrow_mut() = id.clone();
            u.rebuild();
        });
        tabs.append(&tab);
    }
    *ui.tab_counts.borrow_mut() = counts;
    page.append(&tabs);

    let have = ui.have.borrow();
    let categories: Vec<(&str, Vec<&Item>)> = ui
        .catalog
        .categories
        .iter()
        .filter(|c| c.section == current)
        .map(|c| {
            (
                c.name.as_str(),
                ui.catalog.items.iter().filter(|i| i.category == c.id && i.tier == Tier::Welcome).collect::<Vec<_>>(),
            )
        })
        .filter(|(_, items)| !items.is_empty())
        .collect();
    let titled = categories.len() > 1;
    for (name, items) in categories {
        let g = group(if titled { name } else { "" });
        g.root.add_css_class("welcome-items");
        for (n, item) in items.into_iter().enumerate() {
            if n > 0 {
                let line = gtk::Separator::new(gtk::Orientation::Horizontal);
                line.add_css_class("welcome-item-line");
                g.append(&line);
            }
            g.append(&item_row(ui, item, have.has(item), picked_ids.contains(&item.id)));
        }
        page.append(&g.root);
    }
    drop(have);

    let base: Vec<&str> = ui.catalog.items_in(&current, &[Tier::Base]).map(|i| i.name.as_str()).collect();
    if !base.is_empty() {
        let l = w::label(&format!("Included with NyxOS: {}.", base.join(", ")), "welcome-included");
        l.set_wrap(true);
        page.append(&l);
    }
    if current == "agents" {
        page.append(&default_agent(ui).root);
    }
    page.upcast()
}

/// An item: its check, name and summary (the whole of it toggles), and its tags.
fn item_row(ui: &Rc<Ui>, item: &Item, installed: bool, chosen: bool) -> gtk::Box {
    let row = w::hbox(12);
    row.add_css_class("welcome-item");
    let text = w::vbox(2);
    text.set_hexpand(true);
    let top = w::hbox(8);
    top.append(&w::label(&item.name, "welcome-item-name"));
    let from = preset_of(ui, &item.id).map(|p| w::label(&format!("from {p}"), "welcome-item-from"));
    if let Some(f) = &from {
        f.set_visible(chosen);
        top.append(f);
    }
    text.append(&top);
    let summary = w::label(&item.summary, "welcome-item-summary");
    summary.set_wrap(true);
    text.append(&summary);
    if let Some(note) = &item.note {
        let l = w::label(note, "welcome-item-note");
        l.set_wrap(true);
        text.append(&l);
    }
    let check = gtk::CheckButton::new();
    check.set_child(Some(&text));
    check.set_hexpand(true);
    check.set_active(installed || chosen);
    check.set_sensitive(!installed);
    row.append(&check);

    let badges = wrap_box(4, 4);
    badges.add_css_class("welcome-item-tags");
    badges.set_valign(gtk::Align::Center);
    badges.set_halign(gtk::Align::End);
    if installed {
        badges.append(&w::tag("Installed", "success"));
    }
    for (text, kind) in tags(item) {
        badges.append(&w::tag(&text, kind));
    }
    row.append(&badges);
    if !installed {
        let (u, id) = (ui.clone(), item.id.clone());
        check.connect_toggled(move |b| {
            {
                let mut c = u.choices.borrow_mut();
                if b.is_active() {
                    c.selected.insert(id.clone());
                } else {
                    c.selected.remove(&id);
                }
            }
            if let Some(f) = &from {
                f.set_visible(b.is_active());
            }
            u.picks_changed();
        });
    }
    row
}

/// What picking these asks to be agreed to on Review, said once, or `None`.
fn agreement(items: &[&Item]) -> Option<String> {
    const SHOWN: usize = 2;
    let mut parts: Vec<String> = items
        .iter()
        .filter_map(|i| {
            let mut does = Vec::new();
            if !i.services.is_empty() {
                does.push("runs a background service".to_string());
            }
            for g in &i.groups {
                does.push(format!("adds you to the {g} group"));
            }
            if i.unfree {
                does.push("has an unfree licence".to_string());
            }
            (!does.is_empty()).then(|| format!("{} {}", i.name, and_list(&does)))
        })
        .collect();
    if parts.is_empty() {
        return None;
    }
    if parts.len() > SHOWN {
        let more = parts.len() - SHOWN;
        parts.truncate(SHOWN);
        parts.push(format!("{more} more ask for it too"));
    }
    Some(format!("{}; Review asks you to agree.", and_list(&parts)))
}

/// "a", "a and b", "a, b and c".
fn and_list(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// Beside the software page: what's picked so far, by section, the starting points,
/// and what Review will ask to be agreed to.
pub fn basket(ui: &Rc<Ui>) -> gtk::Box {
    let b = w::vbox(0);
    b.add_css_class("welcome-basket");
    let items = picked(ui);
    let title = w::hbox(6);
    title.append(&w::label("Your picks", "welcome-basket-title"));
    if !items.is_empty() {
        title.append(&w::label(&items.len().to_string(), "welcome-basket-count"));
    }
    b.append(&title);
    if items.is_empty() {
        b.append(&w::label("Nothing picked yet.", "welcome-basket-empty"));
    }
    for s in &ui.catalog.sections {
        let names: Vec<&str> =
            items.iter().filter(|i| ui.catalog.section_of(i).is_some_and(|x| x.id == s.id)).map(|i| i.name.as_str()).collect();
        if names.is_empty() {
            continue;
        }
        let pick = w::vbox(2);
        pick.add_css_class("welcome-pick");
        pick.append(&w::label(&s.name, "welcome-pick-section"));
        let l = w::label(&names.join(", "), "welcome-pick-items");
        l.set_wrap(true);
        pick.append(&l);
        b.append(&pick);
    }

    let title = w::label("Starting point", "welcome-basket-title");
    title.add_css_class("later");
    b.append(&title);
    let chips = wrap_box(6, 6);
    for p in &ui.catalog.presets {
        let on = ui.choices.borrow().presets.contains(&p.id);
        let chip = gtk::ToggleButton::new();
        chip.add_css_class("welcome-chip");
        let inner = w::hbox(5);
        if on {
            inner.append(&gtk::Image::from_icon_name("object-select-symbolic"));
        }
        inner.append(&gtk::Label::new(Some(&p.name)));
        chip.set_child(Some(&inner));
        chip.set_active(on);
        chip.set_tooltip_text(Some(&p.summary));
        let (u, id) = (ui.clone(), p.id.clone());
        chip.connect_toggled(move |c| {
            set_preset(&u, &id, c.is_active());
            u.rebuild_keeping_place();
        });
        chips.append(&chip);
    }
    b.append(&chips);

    if let Some(text) = agreement(&items) {
        let note = w::hbox(8);
        note.add_css_class("welcome-basket-note");
        let icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
        icon.set_valign(gtk::Align::Start);
        note.append(&icon);
        let l = w::label(&text, "");
        l.set_wrap(true);
        l.set_hexpand(true);
        note.append(&l);
        b.append(&note);
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agreement_names_what_needs_it() {
        let catalog = nyx_catalog::Catalog::builtin().unwrap();
        let docker = catalog.items.iter().find(|i| !i.groups.is_empty()).unwrap();
        let text = agreement(&[docker]).unwrap();
        assert!(text.starts_with(&docker.name), "{text}");
        assert!(text.contains(&format!("adds you to the {} group", docker.groups[0])), "{text}");
        assert!(text.ends_with("; Review asks you to agree."), "{text}");
        let plain = catalog.items.iter().find(|i| i.groups.is_empty() && i.services.is_empty() && !i.unfree).unwrap();
        assert_eq!(agreement(&[plain]), None);
    }

    #[test]
    fn and_lists() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(and_list(&s(&["a"])), "a");
        assert_eq!(and_list(&s(&["a", "b"])), "a and b");
        assert_eq!(and_list(&s(&["a", "b", "c"])), "a, b and c");
    }
}
