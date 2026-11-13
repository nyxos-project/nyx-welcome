//! The window: the steps down the side in four groups, how far along in the header,
//! the step in the middle with Back, Skip and Next under it, and on the software page
//! what's picked so far beside it. Pages are built each time they're shown, from the
//! choices so far.

use crate::state::Saved;
use crate::steps::{self, GROUPS, Kind, STEPS};
use crate::system::{self, Event, Job};
use gtk::{gdk, glib, prelude::*};
use nyx_catalog::{Catalog, Installed};
use nyx_system_proto::SystemSettings;
use nyx_ui::sidebar::{self, NavItem, Sidebar};
use nyx_ui::{chrome, widgets as w};
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

/// Catalog items Welcome chooses for a new person to begin with.
const PRESELECTED: &[&str] = &["nyx-notepad", "nyx-calculator", "nyx-archive", "nyx-photos"];

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

/// A step in the sidebar, with its done and skipped marks.
struct Nav {
    item: NavItem,
    done: gtk::Image,
    skipped: gtk::Label,
}

/// The header's middle: the step, a bar, and "2 of 9".
struct Progress {
    step: gtk::Label,
    bar: gtk::ProgressBar,
    count: gtk::Label,
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
    /// The software page's tab: a catalog section.
    pub section: RefCell<String>,
    /// Each software tab's count of picks, while the page is shown.
    pub tab_counts: RefCell<Vec<(String, gtk::Label)>>,
    header: chrome::AppHeader,
    sidebar: Rc<Sidebar>,
    nav: Vec<Nav>,
    /// The icon rail's bar where each group starts.
    bars: Vec<gtk::Separator>,
    progress: Progress,
    page: gtk::Box,
    scroller: gtk::ScrolledWindow,
    current: Cell<usize>,
    /// Steps left with Next.
    completed: RefCell<BTreeSet<usize>>,
    footer: gtk::Box,
    back: gtk::Button,
    skip: gtk::Button,
    next: gtk::Button,
    note: gtk::Box,
    note_icon: gtk::Image,
    note_text: gtk::Label,
    /// Beside the software page: what's picked so far.
    picks_pane: gtk::Box,
    picks: gtk::Box,
    width: Cell<i32>,
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
        for (n, nav) in self.nav.iter().enumerate() {
            nav.item.set_active(n == i);
        }
        let step = &STEPS[i];
        self.progress.step.set_text(step.title);
        self.progress.bar.set_fraction((i + 1) as f64 / STEPS.len() as f64);
        self.progress.count.set_text(&format!("{} of {}", i + 1, STEPS.len()));
        chrome::set_title(&self.window, "Welcome", step.title);
        self.footer.set_visible(true);
        self.rebuild();
        self.back.set_visible(i > 0);
        self.skip.set_visible(step.optional);
        self.next.set_visible(step.kind != Kind::Review);
        self.next.set_label(if step.kind == Kind::Intro { "Get started" } else { "Next" });
        self.layout();
    }

    /// Build the current page again (after a change that shows elsewhere on it).
    pub fn rebuild(self: &Rc<Self>) {
        self.fill_page();
        self.scroller.vadjustment().set_value(0.0);
    }

    /// Build the current page again where it's scrolled to, once the click that asked
    /// for it is over (it may come from a widget the rebuild replaces).
    pub fn rebuild_keeping_place(self: &Rc<Self>) {
        let ui = self.clone();
        glib::idle_add_local_once(move || {
            let value = ui.scroller.vadjustment().value();
            ui.fill_page();
            let adj = ui.scroller.vadjustment();
            glib::idle_add_local_once(move || adj.set_value(value));
        });
    }

    fn fill_page(self: &Rc<Self>) {
        while let Some(c) = self.page.first_child() {
            self.page.remove(&c);
        }
        self.tab_counts.borrow_mut().clear();
        self.page.append(&steps::build(self, self.kind()));
        self.picks_changed();
    }

    /// Replace the page with `widget` (Review's progress and results) and hide the footer.
    pub fn take_over(&self, widget: &impl IsA<gtk::Widget>) {
        while let Some(c) = self.page.first_child() {
            self.page.remove(&c);
        }
        self.page.append(widget);
        self.footer.set_visible(false);
        self.layout();
    }

    /// After software was checked or unchecked: the counts, the picks beside the page,
    /// and the footer's note.
    pub fn picks_changed(self: &Rc<Self>) {
        {
            let picked = steps::picked(self);
            for (id, label) in self.tab_counts.borrow().iter() {
                let n = picked.iter().filter(|i| self.catalog.section_of(i).is_some_and(|s| s.id == *id)).count();
                label.set_text(&n.to_string());
                label.set_visible(n > 0);
            }
        }
        while let Some(c) = self.picks.first_child() {
            self.picks.remove(&c);
        }
        if self.kind() == Kind::Software {
            self.picks.append(&steps::basket(self));
        }
        self.update_marks();
        self.update_note();
    }

    pub fn update_marks(&self) {
        let compact = self.sidebar.is_compact();
        let picked = steps::picked(self).len() as u64;
        let saved = self.saved.borrow();
        for (i, (nav, step)) in self.nav.iter().zip(STEPS).enumerate() {
            let skipped = saved.skipped.contains(step.id);
            let done = !skipped && (self.completed.borrow().contains(&i) || (saved.done && step.kind != Kind::Review));
            nav.done.set_visible(done && !compact);
            nav.skipped.set_visible(skipped && !compact);
            nav.item.set_count((step.kind == Kind::Software && !done && !skipped).then_some(picked));
        }
    }

    fn wide(&self) -> bool {
        let width = self.width.get();
        let width = if width > 0 { width } else { self.window.default_width() };
        width >= sidebar::NARROW_WIDTH
    }

    /// Fit the window's width: the picks beside the software page, the header's bar
    /// and name, and the footer's note.
    fn layout(&self) {
        let wide = self.wide();
        self.picks_pane.set_visible(wide && self.kind() == Kind::Software && self.footer.is_visible());
        self.progress.bar.set_size_request(if wide { 160 } else { 100 }, -1);
        if let Some(title) = find_class(self.header.root.upcast_ref(), "app-title") {
            title.set_visible(wide);
        }
        self.update_note();
    }

    fn update_note(&self) {
        let kind = self.kind();
        let Some((icon, text)) = steps::note(kind) else {
            self.note.set_visible(false);
            return;
        };
        let text = if kind == Kind::Software && !self.wide() {
            match steps::picked(self).len() {
                0 => "Installed at Review".to_string(),
                n => format!("{n} picked, installed at Review"),
            }
        } else {
            text.to_string()
        };
        let names: Vec<&str> = icon.split('|').collect();
        self.note_icon.set_from_gicon(&gtk::gio::ThemedIcon::from_names(&names));
        self.note_text.set_text(&text);
        self.note.set_visible(true);
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

/// The first widget under `root` with CSS class `class`.
fn find_class(root: &gtk::Widget, class: &str) -> Option<gtk::Widget> {
    if root.has_css_class(class) {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(c) = child {
        if let Some(found) = find_class(&c, class) {
            return Some(found);
        }
        child = c.next_sibling();
    }
    None
}

/// The steps under their groups' headings, and the rail's bar for each group.
fn steps_sidebar(sidebar: &Sidebar) -> (Vec<Nav>, Vec<gtk::Separator>) {
    let (mut nav, mut bars) = (Vec::new(), Vec::new());
    for (i, step) in STEPS.iter().enumerate() {
        if i == 0 || STEPS[i - 1].group != step.group {
            let (title, section) = GROUPS[step.group];
            let bar = gtk::Separator::new(gtk::Orientation::Horizontal);
            bar.add_css_class("nav-divider");
            bar.add_css_class("compact-show");
            bar.add_css_class(&format!("section-{section}"));
            bar.set_visible(false);
            sidebar.append(&bar);
            bars.push(bar);
            let heading = sidebar::group_label(title, i == 0);
            sidebar::set_section(&heading, section);
            sidebar.append(&heading);
        }
        let item = sidebar::nav_item("", step.title, step.title);
        let content = item.button.child().and_downcast::<gtk::Box>().expect("a nav item holds a box");
        if let Some(image) = content.first_child().and_downcast::<gtk::Image>() {
            let names: Vec<&str> = step.icon.split('|').collect();
            image.set_from_gicon(&gtk::gio::ThemedIcon::from_names(&names));
        }
        let done = gtk::Image::from_icon_name("object-select-symbolic");
        done.add_css_class("welcome-done");
        done.set_tooltip_text(Some("Done"));
        done.set_visible(false);
        content.append(&done);
        let skipped = w::label("Skipped", "nav-sub");
        skipped.set_visible(false);
        content.append(&skipped);
        sidebar.append(&item.button);
        nav.push(Nav { item, done, skipped });
    }
    let foot = w::hbox(8);
    foot.add_css_class("welcome-side-foot");
    foot.add_css_class("compact-hide");
    let icon = gtk::Image::from_icon_name("dialog-information-symbolic");
    icon.set_valign(gtk::Align::Start);
    foot.append(&icon);
    let text = w::label("Nothing for everyone changes until Review", "");
    text.set_wrap(true);
    text.set_hexpand(true);
    foot.append(&text);
    sidebar.append_foot(&foot);
    (nav, bars)
}

pub fn build(app: &gtk::Application, step: Option<&str>) -> Rc<Ui> {
    let window =
        gtk::ApplicationWindow::builder().application(app).title("Welcome").default_width(1000).default_height(700).build();
    window.add_css_class("welcome-window");
    window.set_size_request(480, 480);
    let header = chrome::app_header(&window, crate::APP_ID, "Welcome");
    let toggle = w::bar_button("sidebar-show-symbolic", "Collapse the sidebar (Ctrl+B)");
    header.start.append(&toggle);
    header.center.set_spacing(10);
    let progress = Progress {
        step: w::label("", "welcome-progress-step"),
        bar: gtk::ProgressBar::new(),
        count: w::label("", "welcome-progress-count"),
    };
    progress.bar.add_css_class("welcome-progress");
    progress.bar.set_valign(gtk::Align::Center);
    header.center.append(&progress.step);
    header.center.append(&progress.bar);
    header.center.append(&progress.count);
    let frame = nyx_ui::frame::AppFrame::new(&header.root);
    frame.install(&window);
    frame.set_flush(true);

    let sidebar = Sidebar::new();
    sidebar.root.add_css_class("welcome-side");
    let (nav, bars) = steps_sidebar(&sidebar);
    sidebar.follow_window(&window);
    frame.set_sidebar(&sidebar.root);

    let main = w::vbox(0);
    main.set_hexpand(true);
    let page = w::vbox(0);
    page.add_css_class("welcome-page");
    let clamp = nyx_ui::clamp::Clamp::new(&page, 760);
    let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&clamp).build();
    main.append(&scroller);

    let footer = w::hbox(8);
    footer.add_css_class("welcome-footer");
    let back = gtk::Button::new();
    back.add_css_class("flat");
    back.add_css_class("welcome-back");
    let back_inner = w::hbox(6);
    back_inner.append(&gtk::Image::from_icon_name("go-previous-symbolic"));
    back_inner.append(&gtk::Label::new(Some("Back")));
    back.set_child(Some(&back_inner));
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    let note = w::hbox(6);
    note.add_css_class("welcome-note");
    let note_icon = gtk::Image::new();
    let note_text = w::label("", "");
    note_text.set_ellipsize(gtk::pango::EllipsizeMode::End);
    note.append(&note_icon);
    note.append(&note_text);
    let skip = gtk::Button::with_label("Skip");
    skip.set_tooltip_text(Some("Leave this step out: nothing from it is applied"));
    let next = gtk::Button::with_label("Next");
    next.add_css_class("suggested-action");
    for b in [back.upcast_ref::<gtk::Widget>(), spacer.upcast_ref(), note.upcast_ref(), skip.upcast_ref(), next.upcast_ref()] {
        b.set_valign(gtk::Align::Center);
        footer.append(b);
    }
    main.append(&footer);

    let picks = w::vbox(0);
    let picks_scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&picks).build();
    picks_scroller.set_size_request(260, -1);
    let picks_pane = w::hbox(0);
    picks_pane.add_css_class("welcome-picks-pane");
    picks_pane.append(&nyx_ui::frame::pane_line(gtk::Orientation::Vertical));
    picks_pane.append(&picks_scroller);
    picks_pane.set_visible(false);
    let cols = w::hbox(0);
    cols.append(&main);
    cols.append(&picks_pane);
    frame.set_page(&cols);

    let user = std::env::var("USER").unwrap_or_default();
    let catalog = Catalog::load().unwrap_or_else(|e| {
        eprintln!("nyx-welcome: the catalog: {e:#}");
        Catalog::builtin().expect("the built-in catalog is valid")
    });
    let saved = Saved::load();
    let have = Installed::probe(SystemSettings::default());
    // The standalone Nyx apps NyxOS used to preinstall are each person's now: already
    // chosen for whoever doesn't have them.
    let selected = PRESELECTED.iter().filter_map(|id| catalog.item(id)).filter(|i| !have.has(i)).map(|i| i.id.clone()).collect();
    let start = step.and_then(|id| steps::find(&catalog, id));
    let section = start
        .as_ref()
        .and_then(|(_, s)| s.clone())
        .or_else(|| catalog.sections.first().map(|s| s.id.clone()))
        .unwrap_or_default();
    let ui = Rc::new(Ui {
        window: window.clone(),
        catalog,
        have: RefCell::new(have),
        manager_error: RefCell::new(None),
        user,
        choices: RefCell::new(Choices { selected, ..Choices::default() }),
        saved: RefCell::new(saved),
        section: RefCell::new(section),
        tab_counts: RefCell::default(),
        header,
        sidebar: sidebar.clone(),
        nav,
        bars,
        progress,
        page,
        scroller,
        current: Cell::new(0),
        completed: RefCell::new(BTreeSet::new()),
        footer,
        back: back.clone(),
        skip: skip.clone(),
        next: next.clone(),
        note,
        note_icon,
        note_text,
        picks_pane,
        picks,
        width: Cell::new(0),
        busy: Cell::new(false),
    });

    for (i, nav) in ui.nav.iter().enumerate() {
        let u = ui.clone();
        nav.item.button.connect_clicked(move |_| u.show(i));
    }
    let weak = Rc::downgrade(&ui);
    sidebar.connect_compact(move |compact| {
        if let Some(ui) = weak.upgrade() {
            for bar in &ui.bars {
                bar.set_visible(compact);
            }
            ui.update_marks();
        }
    });
    let s = sidebar.clone();
    toggle.connect_clicked(move |_| s.toggle());
    let weak = Rc::downgrade(&ui);
    window.add_tick_callback(move |w, _| {
        if let Some(ui) = weak.upgrade()
            && w.width() != ui.width.get()
        {
            ui.width.set(w.width());
            ui.layout();
        }
        glib::ControlFlow::Continue
    });
    let u = ui.clone();
    back.connect_clicked(move |_| u.show(u.current.get().saturating_sub(1)));
    let u = ui.clone();
    skip.connect_clicked(move |_| u.advance(true));
    let u = ui.clone();
    next.connect_clicked(move |_| u.advance(false));
    keys(&ui);

    ui.show(start.map(|(i, _)| i).unwrap_or(0));
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
        let ctrl = mods.contains(gdk::ModifierType::CONTROL_MASK);
        match k {
            gdk::Key::Left if alt => u.show(u.current.get().saturating_sub(1)),
            gdk::Key::Right if alt && u.kind() != Kind::Review => u.advance(false),
            gdk::Key::b | gdk::Key::B if ctrl => u.sidebar.toggle(),
            _ => return glib::Propagation::Proceed,
        }
        glib::Propagation::Stop
    });
    ui.window.add_controller(keys);
}
