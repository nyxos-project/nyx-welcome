//! The window: the steps down the side, the step in the middle, and Back, Skip and
//! Next under it. Pages are built each time they're shown, from the choices so far.

use crate::state::Saved;
use crate::steps::{self, Kind, STEPS};
use crate::system::{self, Event, Job};
use gtk::{gdk, glib, prelude::*};
use nyx_catalog::{Catalog, Installed};
use nyx_system_proto::SystemSettings;
use nyx_ui::{chrome, widgets as w};
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

/// What the person chose; nothing on the system changes until Review applies it, except
/// what's live (the theme, default apps) and says so.
#[derive(Debug, Clone, Default)]
pub struct Choices {
    /// Catalog items to install.
    pub selected: BTreeSet<String>,
    /// Presets turned on.
    pub presets: BTreeSet<String>,
    pub hostname: Option<String>,
    pub time_zone: Option<String>,
    pub keyboard: Option<String>,
    /// The theme chosen here, for the boot and login screens too.
    pub screens_theme: Option<String>,
    pub update_after: bool,
}

pub struct Ui {
    pub window: gtk::ApplicationWindow,
    pub catalog: Catalog,
    pub have: RefCell<Installed>,
    /// The System Manager's settings couldn't be read (shown on Review).
    pub manager_error: RefCell<Option<String>>,
    pub user: String,
    pub choices: RefCell<Choices>,
    pub saved: RefCell<Saved>,
    header: chrome::AppHeader,
    list: gtk::ListBox,
    marks: Vec<gtk::Image>,
    page: gtk::Box,
    scroller: gtk::ScrolledWindow,
    current: Cell<usize>,
    /// Steps left with Next.
    completed: RefCell<BTreeSet<usize>>,
    footer: gtk::Box,
    back: gtk::Button,
    skip: gtk::Button,
    next: gtk::Button,
    /// Applying: the steps and buttons wait.
    pub busy: Cell<bool>,
}

impl Ui {
    pub fn kind(&self) -> Kind {
        STEPS[self.current.get()].kind
    }

    /// Show step `i`.
    pub fn show(self: &Rc<Self>, i: usize) {
        if self.busy.get() {
            return;
        }
        let i = i.min(STEPS.len() - 1);
        self.current.set(i);
        if let Some(row) = self.list.row_at_index(i as i32) {
            self.list.select_row(Some(&row));
        }
        let step = &STEPS[i];
        self.header.set_subtitle(if step.kind == Kind::Intro { "" } else { step.title });
        chrome::set_title(&self.window, "Welcome", step.title);
        self.rebuild();
        self.back.set_visible(i > 0);
        self.skip.set_visible(step.optional);
        self.next.set_visible(step.kind != Kind::Review);
        self.next.set_label(if step.kind == Kind::Intro { "Get started" } else { "Next" });
        self.footer.set_visible(true);
        self.update_marks();
    }

    /// Build the current page again (after a change that shows elsewhere on it).
    pub fn rebuild(self: &Rc<Self>) {
        while let Some(c) = self.page.first_child() {
            self.page.remove(&c);
        }
        self.page.append(&steps::build(self, self.kind()));
        self.scroller.vadjustment().set_value(0.0);
    }

    /// Replace the page with `widget` (Review's progress and results) and hide the footer.
    pub fn take_over(&self, widget: &impl IsA<gtk::Widget>) {
        while let Some(c) = self.page.first_child() {
            self.page.remove(&c);
        }
        self.page.append(widget);
        self.footer.set_visible(false);
    }

    pub fn update_marks(&self) {
        let saved = self.saved.borrow();
        for (i, (mark, step)) in self.marks.iter().zip(STEPS).enumerate() {
            let skipped = saved.skipped.contains(step.id);
            let (icon, visible) = if skipped {
                ("action-unavailable-symbolic", true)
            } else if self.completed.borrow().contains(&i) || (saved.done && step.kind != Kind::Review) {
                ("object-select-symbolic", true)
            } else {
                ("", false)
            };
            mark.set_icon_name(Some(icon));
            mark.set_visible(visible);
            mark.set_tooltip_text(Some(if skipped { "Skipped" } else { "Done" }));
        }
    }

    fn advance(self: &Rc<Self>, skipped: bool) {
        let step = &STEPS[self.current.get()];
        if skipped {
            self.completed.borrow_mut().remove(&self.current.get());
            steps::skip(self, step.kind);
            self.saved.borrow_mut().skipped.insert(step.id.into());
        } else {
            self.completed.borrow_mut().insert(self.current.get());
            self.saved.borrow_mut().skipped.remove(step.id);
        }
        self.saved.borrow().save();
        self.show(self.current.get() + 1);
    }

    /// Read the System Manager's settings and what's installed again, then `then`.
    pub fn refresh_installed(self: &Rc<Self>, then: impl Fn(&Rc<Ui>) + 'static) {
        let rx = system::run(Job::Settings);
        let ui = self.clone();
        glib::spawn_future_local(async move {
            while let Ok(ev) = rx.recv().await {
                if let Event::Done(result) = ev {
                    let settings = match result {
                        Ok(json) => {
                            *ui.manager_error.borrow_mut() = None;
                            serde_json::from_str::<SystemSettings>(&json).unwrap_or_default()
                        }
                        Err(e) => {
                            *ui.manager_error.borrow_mut() = Some(e);
                            SystemSettings::default()
                        }
                    };
                    *ui.have.borrow_mut() = Installed::probe(settings);
                    then(&ui);
                    break;
                }
            }
        });
    }
}

pub fn build(app: &gtk::Application, step: Option<&str>) -> Rc<Ui> {
    let window =
        gtk::ApplicationWindow::builder().application(app).title("Welcome").default_width(1000).default_height(700).build();
    window.add_css_class("welcome-window");
    window.set_size_request(720, 520);
    let overlay = gtk::Overlay::new();
    let outer = w::vbox(0);
    outer.add_css_class("window-frame");
    overlay.set_child(Some(&outer));
    window.set_child(Some(&overlay));
    let header = chrome::app_header(&window, crate::APP_ID, "Welcome");
    outer.append(&header.root);

    let body = w::hbox(0);
    body.set_vexpand(true);
    outer.append(&body);

    let list = gtk::ListBox::new();
    list.add_css_class("welcome-steps");
    list.set_selection_mode(gtk::SelectionMode::Single);
    let mut marks = Vec::new();
    for (i, step) in STEPS.iter().enumerate() {
        let row = w::hbox(10);
        row.add_css_class("welcome-step");
        let num = w::label(&format!("{}", i + 1), "welcome-step-number");
        num.set_width_chars(2);
        row.append(&num);
        // "a|b": the first of the names the icon theme has (Umbra's, then Adwaita's).
        let names: Vec<&str> = step.icon.split('|').collect();
        let icon = gtk::Image::from_gicon(&gtk::gio::ThemedIcon::from_names(&names));
        row.append(&icon);
        let title = w::label(step.title, "");
        title.set_hexpand(true);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        row.append(&title);
        let mark = gtk::Image::new();
        mark.add_css_class("welcome-step-mark");
        mark.set_visible(false);
        row.append(&mark);
        marks.push(mark);
        list.append(&row);
    }
    let side = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&list).build();
    side.add_css_class("welcome-side");
    side.set_size_request(250, -1);
    body.append(&side);

    let main = w::vbox(0);
    main.set_hexpand(true);
    let page = w::vbox(0);
    page.add_css_class("welcome-page");
    let clamp = nyx_ui::clamp::Clamp::new(&page, 760);
    let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&clamp).build();
    main.append(&scroller);

    let footer = w::hbox(8);
    footer.add_css_class("welcome-footer");
    let back = gtk::Button::with_label("Back");
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    let skip = gtk::Button::with_label("Skip");
    skip.set_tooltip_text(Some("Leave this step out: nothing from it is applied"));
    let next = gtk::Button::with_label("Next");
    next.add_css_class("suggested-action");
    footer.append(&back);
    footer.append(&spacer);
    footer.append(&skip);
    footer.append(&next);
    main.append(&footer);
    body.append(&main);

    let user = std::env::var("USER").unwrap_or_default();
    let catalog = Catalog::load().unwrap_or_else(|e| {
        eprintln!("nyx-welcome: the catalog: {e:#}");
        Catalog::builtin().expect("the built-in catalog is valid")
    });
    let saved = Saved::load();
    let ui = Rc::new(Ui {
        window: window.clone(),
        catalog,
        have: RefCell::new(Installed::probe(SystemSettings::default())),
        manager_error: RefCell::new(None),
        user,
        choices: RefCell::new(Choices::default()),
        saved: RefCell::new(saved),
        header,
        list: list.clone(),
        marks,
        page,
        scroller,
        current: Cell::new(0),
        completed: RefCell::new(BTreeSet::new()),
        footer,
        back: back.clone(),
        skip: skip.clone(),
        next: next.clone(),
        busy: Cell::new(false),
    });

    let u = ui.clone();
    list.connect_row_activated(move |_, row| u.show(row.index() as usize));
    let u = ui.clone();
    back.connect_clicked(move |_| u.show(u.current.get().saturating_sub(1)));
    let u = ui.clone();
    skip.connect_clicked(move |_| u.advance(true));
    let u = ui.clone();
    next.connect_clicked(move |_| u.advance(false));
    keys(&ui);

    let start = step.and_then(|id| STEPS.iter().position(|s| s.id == id)).unwrap_or(0);
    ui.show(start);
    // What's installed decides what's offered: the manager answers in the background.
    ui.refresh_installed(|ui| ui.rebuild());
    ui
}

fn keys(ui: &Rc<Ui>) {
    nyx_ui::keys::install(&ui.window, nyx_ui::keys::Common { close: true, ..Default::default() });
    let keys = gtk::EventControllerKey::new();
    let u = ui.clone();
    keys.connect_key_pressed(move |_, k, _, mods| {
        let alt = mods.contains(gdk::ModifierType::ALT_MASK);
        match k {
            gdk::Key::Left if alt => u.show(u.current.get().saturating_sub(1)),
            gdk::Key::Right if alt && u.kind() != Kind::Review => u.advance(false),
            _ => return glib::Propagation::Proceed,
        }
        glib::Propagation::Stop
    });
    ui.window.add_controller(keys);
}
