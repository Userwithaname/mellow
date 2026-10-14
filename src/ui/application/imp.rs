use adw::subclass::prelude::*;
use gtk::glib;

use core::cell::{Cell, OnceCell};
use std::thread::JoinHandle;

use crate::ui::Window;

#[derive(Default)]
pub struct Application {
    /// Only one appication window may be open at a time
    pub window: OnceCell<Window>,
    pub component_handles: Cell<Option<[JoinHandle<()>; 2]>>,
}

#[glib::object_subclass]
impl ObjectSubclass for Application {
    const NAME: &str = "MellowApplication";
    type Type = super::Application;
    type ParentType = adw::Application;
}
impl ObjectImpl for Application {}
impl ApplicationImpl for Application {}
impl AdwApplicationImpl for Application {}
impl GtkApplicationImpl for Application {}
