use adw::subclass::prelude::*;
use glib::{clone, variant::StaticVariantType};
use gtk::{gio, glib, prelude::WidgetExt};

use crate::ui::Window;

#[inline]
pub fn open_sheet(window: &Window) -> gio::ActionEntry<gio::SimpleActionGroup> {
    gio::ActionEntry::builder("open_sheet")
        .activate(clone!(
            #[weak(rename_to=ui)]
            window.imp(),
            move |_, _, _| ui.open_sheet(true)
        ))
        .build()
}
#[inline]
pub fn close_sheet(window: &Window) -> gio::ActionEntry<gio::SimpleActionGroup> {
    gio::ActionEntry::builder("close_sheet")
        .activate(clone!(
            #[weak(rename_to=ui)]
            window.imp(),
            move |_, _, _| ui.open_sheet(false)
        ))
        .build()
}
#[inline]
pub fn toggle_sheet(window: &Window) -> gio::ActionEntry<gio::SimpleActionGroup> {
    gio::ActionEntry::builder("toggle_sheet")
        .activate(clone!(
            #[weak(rename_to=ui)]
            window.imp(),
            move |_, _, _| ui.toggle_sheet()
        ))
        .build()
}
#[inline]
pub fn open_library(window: &Window) -> gio::ActionEntry<gio::SimpleActionGroup> {
    gio::ActionEntry::builder("open_library")
        .activate(clone!(
            #[weak(rename_to=ui)]
            window.imp(),
            move |_, _, _| {
                ui.focus_library();
                ui.open_sheet(true);
            }
        ))
        .build()
}
#[inline]
pub fn open_playing(window: &Window) -> gio::ActionEntry<gio::SimpleActionGroup> {
    gio::ActionEntry::builder("open_playing")
        .activate(clone!(
            #[weak(rename_to=ui)]
            window.imp(),
            move |_, _, _| {
                ui.focus_playing();
                ui.open_sheet(true);
            }
        ))
        .build()
}
#[inline]
pub fn open_settings(window: &Window) -> gio::ActionEntry<gio::SimpleActionGroup> {
    gio::ActionEntry::builder("open_settings")
        .activate(clone!(
            #[weak(rename_to=ui)]
            window.imp(),
            move |_, _, _| {
                ui.focus_settings();
                ui.open_sheet(true);
            }
        ))
        .build()
}
#[inline]
pub fn playing_nav_push(window: &Window) -> gio::ActionEntry<gio::SimpleActionGroup> {
    gio::ActionEntry::builder("playing_nav_push")
        .parameter_type(Some(&String::static_variant_type()))
        .activate(clone!(
            #[weak(rename_to=ui)]
            window.imp(),
            move |_, _, tag| {
                let tag = tag.unwrap().get::<String>().unwrap();
                ui.playing.push_by_tag(&tag);
            }
        ))
        .build()
}
#[inline]
pub fn playing_nav_pop(window: &Window) -> gio::ActionEntry<gio::SimpleActionGroup> {
    gio::ActionEntry::builder("playing_nav_pop")
        .activate(clone!(
            #[weak(rename_to=ui)]
            window.imp(),
            move |_, _, _| {
                ui.playing.pop();
            }
        ))
        .build()
}
#[inline]
pub fn library_nav_pop(window: &Window) -> gio::ActionEntry<gio::SimpleActionGroup> {
    gio::ActionEntry::builder("library_nav_pop")
        .activate(clone!(
            #[weak(rename_to=ui)]
            window.imp(),
            move |_, _, _| {
                ui.library.pop();
            }
        ))
        .build()
}

#[inline]
pub fn library_search(window: &Window) -> gio::ActionEntry<gio::SimpleActionGroup> {
    gio::ActionEntry::builder("library_search")
        .activate(clone!(
            #[weak(rename_to=ui)]
            window.imp(),
            move |_, _, _| if ui.artists_page.is_mapped() {
                ui.artists_page.focus_search()
            } else if ui.songs_page.is_mapped() {
                ui.songs_page.focus_search()
            } else if ui.albums_page.is_mapped() {
                ui.albums_page.focus_search()
            }
        ))
        .build()
}
