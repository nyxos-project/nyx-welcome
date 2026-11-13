//! A box that lays its children out at their own width in rows, starting a new row
//! when the next one doesn't fit (the software page's sections, tags and chips).

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

pub fn wrap_box(spacing: i32, row_spacing: i32) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    b.set_layout_manager(Some(WrapLayout::new(spacing, row_spacing)));
    b
}

mod imp {
    use super::*;
    use std::cell::Cell;

    #[derive(Default)]
    pub struct WrapLayout {
        pub spacing: Cell<i32>,
        pub row_spacing: Cell<i32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for WrapLayout {
        const NAME: &'static str = "NyxWelcomeWrapLayout";
        type Type = super::WrapLayout;
        type ParentType = gtk::LayoutManager;
    }

    impl ObjectImpl for WrapLayout {}

    fn children(w: &gtk::Widget) -> Vec<gtk::Widget> {
        let mut out = Vec::new();
        let mut child = w.first_child();
        while let Some(c) = child {
            if c.should_layout() {
                out.push(c.clone());
            }
            child = c.next_sibling();
        }
        out
    }

    impl WrapLayout {
        /// Each child's place (x, y, width, height) in `width`, and the total height.
        fn place(&self, kids: &[gtk::Widget], width: i32) -> (Vec<(i32, i32, i32, i32)>, i32) {
            let (gap, row_gap) = (self.spacing.get(), self.row_spacing.get());
            let mut out = Vec::with_capacity(kids.len());
            let (mut x, mut y, mut row_h) = (0, 0, 0);
            for c in kids {
                let w = c.measure(gtk::Orientation::Horizontal, -1).1.min(width.max(1));
                let h = c.measure(gtk::Orientation::Vertical, w).1;
                if x > 0 && x + w > width {
                    x = 0;
                    y += row_h + row_gap;
                    row_h = 0;
                }
                out.push((x, y, w, h));
                x += w + gap;
                row_h = row_h.max(h);
            }
            (out, y + row_h)
        }
    }

    impl LayoutManagerImpl for WrapLayout {
        fn request_mode(&self, _widget: &gtk::Widget) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, widget: &gtk::Widget, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let kids = children(widget);
            if orientation == gtk::Orientation::Horizontal {
                let min = kids.iter().map(|c| c.measure(orientation, -1).0).max().unwrap_or(0);
                let gaps = self.spacing.get() * (kids.len() as i32 - 1).max(0);
                let nat = kids.iter().map(|c| c.measure(orientation, -1).1).sum::<i32>() + gaps;
                return (min, nat, -1, -1);
            }
            let width = if for_size < 0 { i32::MAX / 2 } else { for_size };
            let h = self.place(&kids, width).1;
            (h, h, -1, -1)
        }

        fn allocate(&self, widget: &gtk::Widget, width: i32, _height: i32, _baseline: i32) {
            let kids = children(widget);
            let (places, _) = self.place(&kids, width);
            for (c, (x, y, w, h)) in kids.iter().zip(places) {
                c.size_allocate(&gtk::Allocation::new(x, y, w, h), -1);
            }
        }
    }
}

glib::wrapper! {
    pub struct WrapLayout(ObjectSubclass<imp::WrapLayout>) @extends gtk::LayoutManager;
}

impl WrapLayout {
    pub fn new(spacing: i32, row_spacing: i32) -> WrapLayout {
        let l: WrapLayout = glib::Object::new();
        l.imp().spacing.set(spacing);
        l.imp().row_spacing.set(row_spacing);
        l
    }
}
