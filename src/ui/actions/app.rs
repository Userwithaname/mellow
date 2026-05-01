use adw::prelude::*;
use gtk::{gio, glib};

use crate::ui::{Application, Window};

#[inline]
pub fn show_window(app: &Application) -> gio::ActionEntry<Application> {
    gio::ActionEntry::builder("show_window")
        .activate(glib::clone!(
            #[weak]
            app,
            move |_, _, _| app.show_window()
        ))
        .build()
}

#[inline]
pub fn quit(app: &Application, window: &Window) -> gio::ActionEntry<Application> {
    gio::ActionEntry::builder("quit")
        .activate(glib::clone!(
            #[weak]
            window,
            #[weak]
            app,
            move |_, _, _| {
                window.close();
                app.quit();
            }
        ))
        .build()
}
