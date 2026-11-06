//! The presets, and a section of the software catalog to choose from.

use super::{group, heading};
use crate::window::Ui;
use gtk::prelude::*;
use nyx_catalog::{Item, Tier};
use nyx_ui::widgets as w;
use std::rc::Rc;

pub fn presets(ui: &Rc<Ui>) -> gtk::Widget {
    let page = w::vbox(18);
    page.append(&heading(
        "Starting point",
        "Pick any that fit: each checks a set of software in the next steps, where you can change every item. Or pick none and choose as you go.",
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
            let mut c = u.choices.borrow_mut();
            if b.is_active() {
                c.presets.insert(id.clone());
                if let Ok(items) = u.catalog.presets(&[&id]) {
                    c.selected.extend(items);
                }
            } else {
                c.presets.remove(&id);
                let others: Vec<&str> = c.presets.iter().map(String::as_str).collect();
                let keep = u.catalog.presets(&others).unwrap_or_default();
                for item in u.catalog.presets(&[&id]).unwrap_or_default() {
                    if !keep.contains(&item) {
                        c.selected.remove(&item);
                    }
                }
            }
        });
        grid.insert(&card, -1);
    }
    page.append(&grid);
    page.upcast()
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
    if item.unfree {
        out.push(("Unfree license".into(), "warning"));
    }
    if item.flatpak.is_some() {
        out.push(("Flathub".into(), ""));
    }
    if item.profile.is_some() {
        out.push(("For you".into(), ""));
    }
    for s in &item.services {
        out.push((format!("Service: {s}"), ""));
    }
    for g in &item.groups {
        out.push((format!("Group: {g}"), "warning"));
    }
    out
}

pub fn section(ui: &Rc<Ui>, id: &str) -> gtk::Widget {
    let page = w::vbox(18);
    let Some(section) = ui.catalog.section(id) else { return page.upcast() };
    page.append(&heading(&section.name, &section.summary));
    let have = ui.have.borrow();

    let base: Vec<&str> = ui.catalog.items_in(id, &[Tier::Base]).map(|i| i.name.as_str()).collect();
    if !base.is_empty() {
        let l = w::label(&format!("Included with NyxOS: {}.", base.join(", ")), "welcome-included");
        l.set_wrap(true);
        page.append(&l);
    }

    for category in ui.catalog.categories.iter().filter(|c| c.section == id) {
        let items: Vec<&Item> =
            ui.catalog.items.iter().filter(|i| i.category == category.id && i.tier == Tier::Welcome).collect();
        if items.is_empty() {
            continue;
        }
        let g = group(&category.name);
        for item in items {
            let installed = have.has(item);
            let check = gtk::CheckButton::new();
            check.set_active(installed || ui.choices.borrow().selected.contains(&item.id));
            check.set_sensitive(!installed);
            let mut desc = gtk::glib::markup_escape_text(&item.summary).to_string();
            if let Some(note) = &item.note {
                desc.push_str(&format!("\n<small>{}</small>", gtk::glib::markup_escape_text(note)));
            }
            let row = w::row(&item.name, &desc, None);
            row.prepend(&check);
            check.set_valign(gtk::Align::Center);
            let badges = w::hbox(4);
            badges.set_valign(gtk::Align::Center);
            if installed {
                badges.append(&w::tag("Installed", "success"));
            }
            for (text, kind) in tags(item) {
                badges.append(&w::tag(&text, kind));
            }
            row.append(&badges);
            row.add_css_class("welcome-item");
            if !installed {
                let (u, id) = (ui.clone(), item.id.clone());
                check.connect_toggled(move |b| {
                    let mut c = u.choices.borrow_mut();
                    if b.is_active() {
                        c.selected.insert(id.clone());
                    } else {
                        c.selected.remove(&id);
                    }
                });
            }
            g.append(&row);
        }
        page.append(&g.root);
    }
    let more = w::label("Nyx Store has the rest of nixpkgs and Flathub, any time.", "welcome-included");
    more.set_wrap(true);
    page.append(&more);
    page.upcast()
}
