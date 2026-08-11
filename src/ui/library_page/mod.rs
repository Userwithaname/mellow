use adw::{prelude::*, subclass::prelude::*};
use gtk::glib;

mod imp;

pub mod filter;
pub mod sort;

glib::wrapper! {
    pub struct LibraryPage(ObjectSubclass<imp::LibraryPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements
            gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::Orientable, gtk::ConstraintTarget;
}

impl LibraryPage {
    #[inline]
    pub fn switch_view(&self, name: &str) {
        let view_stack = &self.imp().view_stack;
        view_stack.set_visible_child_name(name);
        view_stack.set_cursor_from_name(match name == "loading" {
            true => Some("wait"),
            false => None,
        });
    }

    #[inline]
    pub fn set_empty(&self, empty: bool) {
        if self.imp().is_empty.replace(empty) == empty {
            return;
        }
        match empty {
            false => self.imp().ready_stack.set_visible_child_name("library"),
            true => self.imp().ready_stack.set_visible_child_name("empty"),
        }
    }
}

#[derive(Debug)]
pub enum SubpageType {
    Song,
    Album,
    Artist,
}

pub trait LibraryObject {
    fn play_count(&self) -> f64;
    fn stars(&self) -> f64;
    fn rating(&self) -> f64;
    fn year(&self) -> u32;
    fn added(&self) -> u64;
    fn modified(&self) -> u64;
    fn tags(&self) -> Vec<String>;
}
