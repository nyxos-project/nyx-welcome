//! Review and apply: everything chosen, what has to be agreed to, then one system
//! rebuild through the Nyx System Manager, the Flathub installs and the update, with
//! their progress, and a retry when something fails.

use super::{STEPS, group, heading};
use crate::system::{self, Event, Job};
use crate::window::{Choices, Ui};
use gtk::{glib, prelude::*};
use nyx_catalog::Plan;
use nyx_system_proto::Change;
use nyx_ui::widgets as w;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

fn system_changes(c: &Choices) -> Vec<Change> {
    let mut out = Vec::new();
    if let Some(v) = &c.hostname {
        out.push(Change::Hostname { value: Some(v.clone()) });
    }
    if let Some(v) = &c.time_zone {
        out.push(Change::TimeZone { value: Some(v.clone()) });
    }
    if let Some(v) = &c.keyboard {
        out.push(Change::KeyboardLayout { value: Some(v.clone()) });
    }
    if let Some(v) = &c.screens_theme {
        out.push(Change::BootTheme { value: Some(v.clone()) });
        out.push(Change::LoginTheme { value: Some(v.clone()) });
    }
    out
}

fn describe(change: &Change) -> Option<(String, String)> {
    Some(match change {
        Change::Hostname { value: Some(v) } => ("Computer name".into(), v.clone()),
        Change::TimeZone { value: Some(v) } => ("Time zone".into(), v.clone()),
        Change::KeyboardLayout { value: Some(v) } => ("Keyboard".into(), v.clone()),
        Change::BootTheme { value: Some(v) } => ("Boot and sign-in screens".into(), format!("The {v} theme")),
        _ => return None,
    })
}

pub fn build(ui: &Rc<Ui>) -> gtk::Widget {
    let page = w::vbox(18);
    page.append(&heading(
        "Review and apply",
        "What will change. System changes are built first and only switched to when the build works.",
    ));
    let choices = ui.choices.borrow().clone();
    let plan = match ui.catalog.plan(&choices.selected, &ui.user, &ui.have.borrow()) {
        Ok(p) => p,
        Err(e) => {
            page.append(&w::banner(&glib::markup_escape_text(&format!("{e:#}")), true));
            return page.upcast();
        }
    };
    let mut changes = system_changes(&choices);
    let settings_rows: Vec<(String, String)> = changes.iter().filter_map(describe).collect();
    changes.extend(plan.changes.iter().cloned());

    if let Some(e) = ui.manager_error.borrow().as_ref()
        && !changes.is_empty()
    {
        page.append(&w::banner(
            &format!(
                "The Nyx System Manager can't be reached, so system changes can't be applied: {}",
                glib::markup_escape_text(e)
            ),
            true,
        ));
    }

    if !settings_rows.is_empty() {
        let g = group("System settings");
        for (k, v) in &settings_rows {
            g.append(&w::row(k, &glib::markup_escape_text(v), None));
        }
        page.append(&g);
    }

    let have = ui.have.borrow();
    let new_items: Vec<&nyx_catalog::Item> =
        choices.selected.iter().filter_map(|id| ui.catalog.item(id)).filter(|i| !have.has(i)).collect();
    let everyone: Vec<&str> = new_items.iter().filter(|i| i.flatpak.is_none()).map(|i| i.name.as_str()).collect();
    if !everyone.is_empty() {
        let g = group(&format!("Software for everyone ({})", everyone.len()));
        let l = w::label(&everyone.join(", "), "welcome-list");
        l.set_wrap(true);
        g.append(&l);
        page.append(&g);
    }
    let flathub: Vec<&str> = new_items.iter().filter(|i| i.flatpak.is_some()).map(|i| i.name.as_str()).collect();
    if !flathub.is_empty() {
        let g = group("From Flathub, for you");
        let l = w::label(&flathub.join(", "), "welcome-list");
        l.set_wrap(true);
        g.append(&l);
        page.append(&g);
    }
    drop(have);
    if choices.update_after {
        let g = group("Afterwards");
        g.append(&w::row("Update NyxOS", "The latest NyxOS, apps and packages.", None));
        page.append(&g);
    }

    let nothing = plan.is_empty() && changes.is_empty() && !choices.update_after;
    let apply = gtk::Button::with_label(if nothing { "Finish" } else { "Apply" });
    apply.add_css_class("suggested-action");
    apply.set_halign(gtk::Align::End);

    let agreed: Rc<RefCell<BTreeSet<usize>>> = Rc::default();
    if !plan.consent.is_empty() {
        let g = group("Needs your agreement");
        let total = plan.consent.len();
        for (i, consent) in plan.consent.iter().enumerate() {
            let check = gtk::CheckButton::with_label("I agree");
            let row = w::row(&consent.describe(), "", Some(check.upcast_ref()));
            let (agreed, apply) = (agreed.clone(), apply.clone());
            check.connect_toggled(move |b| {
                if b.is_active() {
                    agreed.borrow_mut().insert(i);
                } else {
                    agreed.borrow_mut().remove(&i);
                }
                apply.set_sensitive(agreed.borrow().len() == total);
            });
            g.append(&row);
        }
        apply.set_sensitive(false);
        page.append(&g);
    }

    if nothing {
        page.append(&w::row(
            "Nothing to apply",
            "Your theme and the other changes made along the way are already in place.",
            None,
        ));
    } else if !changes.is_empty() {
        page.append(&w::banner(
            "Applying asks for an administrator's password, then builds the new system. That can take a while; you can keep using the computer.",
            false,
        ));
    }
    let actions = w::hbox(8);
    actions.set_halign(gtk::Align::End);
    actions.append(&apply);
    page.append(&actions);

    let u = ui.clone();
    let update = choices.update_after;
    apply.connect_clicked(move |_| {
        if nothing {
            finish(&u);
        } else {
            run(&u, changes.clone(), plan.clone(), update);
        }
    });
    page.upcast()
}

/// The progress page: what's happening now, a bar, and the output underneath.
struct Progress {
    root: gtk::Box,
    stage: gtk::Label,
    bar: gtk::ProgressBar,
    log: gtk::TextView,
}

impl Progress {
    fn new() -> Rc<Progress> {
        let root = w::vbox(14);
        root.append(&heading("Setting up", "You can keep using the computer while this runs."));
        let stage = w::label("", "welcome-stage");
        let bar = gtk::ProgressBar::new();
        let log = gtk::TextView::new();
        log.set_editable(false);
        log.set_monospace(true);
        log.set_wrap_mode(gtk::WrapMode::WordChar);
        log.add_css_class("welcome-log");
        let scroll = gtk::ScrolledWindow::builder().min_content_height(260).child(&log).build();
        let details = gtk::Expander::new(Some("Details"));
        details.set_child(Some(&scroll));
        root.append(&stage);
        root.append(&bar);
        root.append(&details);
        let p = Rc::new(Progress { root, stage, bar, log });
        let weak = Rc::downgrade(&p);
        glib::timeout_add_local(std::time::Duration::from_millis(150), move || match weak.upgrade() {
            Some(p) if p.root.parent().is_some() => {
                p.bar.pulse();
                glib::ControlFlow::Continue
            }
            Some(_) => glib::ControlFlow::Continue,
            None => glib::ControlFlow::Break,
        });
        p
    }

    fn line(&self, text: &str) {
        let buf = self.log.buffer();
        let mut end = buf.end_iter();
        buf.insert(&mut end, text);
        buf.insert(&mut end, "\n");
        let mark = buf.create_mark(None, &buf.end_iter(), false);
        self.log.scroll_mark_onscreen(&mark);
    }
}

fn run(ui: &Rc<Ui>, changes: Vec<Change>, plan: Plan, update: bool) {
    let mut stages: Vec<(&'static str, Job)> = Vec::new();
    if !changes.is_empty() {
        stages.push(("Building and switching to the new system", Job::Apply(changes)));
    }
    if !plan.flatpaks.is_empty() {
        stages.push(("Installing from Flathub", Job::Flatpaks(plan.flatpaks.clone())));
    }
    if update {
        stages.push(("Updating NyxOS", Job::Update));
    }
    let progress = Progress::new();
    ui.take_over(&progress.root);
    ui.busy.set(true);
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        for (label, job) in stages {
            progress.stage.set_text(label);
            progress.line(&format!("== {label}"));
            let rx = system::run(job);
            let mut outcome = Err("it stopped without saying why".to_string());
            while let Ok(ev) = rx.recv().await {
                match ev {
                    Event::Line(l) => progress.line(&l),
                    Event::Done(r) => {
                        outcome = r;
                        break;
                    }
                }
            }
            match outcome {
                Ok(summary) => progress.line(&summary),
                Err(e) => {
                    progress.line(&e);
                    failed(&ui, &progress, label, &e);
                    return;
                }
            }
        }
        ui.busy.set(false);
        ui.refresh_installed(finish);
    });
}

fn failed(ui: &Rc<Ui>, progress: &Rc<Progress>, stage: &str, error: &str) {
    ui.busy.set(false);
    progress.stage.set_text(&format!("{stage} didn't work"));
    progress.bar.set_fraction(0.0);
    let mut text = error.trim_end_matches('.').to_string();
    if let Some(first) = text.get(..1) {
        text = first.to_uppercase() + &text[1..];
    }
    let msg = w::banner(&glib::markup_escape_text(&format!("{text}.")), true);
    progress.root.insert_child_after(&msg, Some(&progress.bar));
    let actions = w::hbox(8);
    actions.set_halign(gtk::Align::End);
    let back = gtk::Button::with_label("Back to Review");
    let retry = gtk::Button::with_label("Try Again");
    retry.add_css_class("suggested-action");
    actions.append(&back);
    actions.append(&retry);
    progress.root.append(&actions);
    let review = STEPS.len() - 1;
    let u = ui.clone();
    back.connect_clicked(move |_| u.refresh_installed(move |ui| ui.show(review)));
    let u = ui.clone();
    // What already worked shows as installed, so the plan is only what's left.
    retry.connect_clicked(move |_| {
        u.refresh_installed(move |ui| {
            let choices = ui.choices.borrow().clone();
            let plan = ui.catalog.plan(&choices.selected, &ui.user, &ui.have.borrow());
            match plan {
                Ok(plan) => {
                    let mut changes = system_changes(&choices);
                    changes.extend(plan.changes.iter().cloned());
                    run(ui, changes, plan, choices.update_after);
                }
                Err(_) => ui.show(review),
            }
        })
    });
}

fn finish(ui: &Rc<Ui>) {
    {
        let mut saved = ui.saved.borrow_mut();
        saved.done = true;
        saved.save();
    }
    {
        let mut c = ui.choices.borrow_mut();
        c.hostname = None;
        c.time_zone = None;
        c.keyboard = None;
        c.screens_theme = None;
        c.update_after = false;
    }
    let page = w::vbox(18);
    page.append(&heading(
        "You're all set",
        "NyxOS is ready. Welcome is in the launcher whenever you want to change something here again.",
    ));
    let next = group("Next");
    next.append(
        &w::button_row("Nyx Store", "Everything else in nixpkgs and Flathub.", "Open Store", |_| {
            crate::live::launch("nyx-store");
        })
        .0,
    );
    next.append(&super::settings_row("Nyx Settings", "Every setting, in one place.", "Open Settings", "home"));
    page.append(&next);
    let close = gtk::Button::with_label("Close");
    close.add_css_class("suggested-action");
    close.set_halign(gtk::Align::End);
    let win = ui.window.clone();
    close.connect_clicked(move |_| win.close());
    page.append(&close);
    ui.take_over(&page);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_settings_become_changes() {
        let c = Choices {
            hostname: Some("nyx-laptop".into()),
            time_zone: Some("Europe/Berlin".into()),
            screens_theme: Some("nord".into()),
            ..Default::default()
        };
        let changes = system_changes(&c);
        assert_eq!(changes.len(), 4);
        for ch in &changes {
            ch.validate().unwrap();
        }
        assert!(changes.contains(&Change::LoginTheme { value: Some("nord".into()) }));
        assert!(system_changes(&Choices::default()).is_empty());
    }
}
