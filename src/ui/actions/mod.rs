use adw::{prelude::*, subclass::prelude::*};
use glib::GString;
use gtk::{gio, glib};
use std::rc::Rc;

use crate::about::show_about_dialog;
use crate::shortcuts::show_shortcuts_dialog;
use crate::ui::{Application, Window, actions};

pub mod app;
pub mod menu;
pub mod player;
pub mod ui;

pub trait Actions {
    fn setup_actions(&self);
}
impl Actions for Application {
    #[inline]
    fn setup_actions(&self) {
        self.add_action_entries([
            actions::app::show_window(self),
            actions::app::quit(self, self.window()),
        ]);
    }
}

pub trait WindowActions {
    fn setup_actions(&self, songs_sort: &GString, albums_sort: &GString, artists_sort: &GString);
}
impl WindowActions for Window {
    #[inline]
    fn setup_actions(&self, songs_sort: &GString, albums_sort: &GString, artists_sort: &GString) {
        self.add_action_entries([
            gio::ActionEntry::builder("show_about_dialog")
                .activate(move |window: &Window, _, _| show_about_dialog(window))
                .build(),
            gio::ActionEntry::builder("show_shortcuts_dialog")
                .activate(move |window: &Window, _, _| show_shortcuts_dialog(window))
                .build(),
        ]);

        let player_actions = gio::SimpleActionGroup::new();
        let window_imp = self.imp();

        player_actions.add_action_entries({
            let player = window_imp.main_player.imp();
            [
                actions::player::skip_prev(player),
                actions::player::play_pause(player),
                actions::player::skip_next(player),
                actions::player::play_all_songs(&window_imp.songs_page),
                actions::player::play_all_albums(&window_imp.albums_page),
                actions::player::play_all_artists(&window_imp.artists_page),
                actions::player::queue_visible_album(Rc::clone(&window_imp.album_pages)),
                actions::player::queue_visible_artist(Rc::clone(&window_imp.artist_pages)),
            ]
        });
        self.insert_action_group("player", Some(&player_actions));

        let ui_actions = gio::SimpleActionGroup::new();
        ui_actions.add_action_entries([
            actions::ui::open_sheet(window_imp),
            actions::ui::close_sheet(window_imp),
            actions::ui::toggle_sheet(window_imp),
            actions::ui::open_library(window_imp),
            actions::ui::open_playing(window_imp),
            actions::ui::open_settings(window_imp),
            actions::ui::playing_nav_push(window_imp.playing.get()),
            actions::ui::playing_nav_pop(window_imp.playing.get()),
            actions::ui::library_nav_pop(window_imp.library.get()),
            actions::ui::library_search(window_imp),
        ]);
        self.insert_action_group("ui", Some(&ui_actions));

        let menu_actions = gio::SimpleActionGroup::new();
        menu_actions.add_action_entries([
            actions::menu::songs_sort_mode(window_imp.songs_page.get(), songs_sort),
            actions::menu::albums_sort_mode(window_imp.albums_page.get(), albums_sort),
            actions::menu::artists_sort_mode(window_imp.artists_page.get(), artists_sort),
            actions::menu::songs_play_mode(window_imp.songs_page.get()),
            actions::menu::albums_play_mode(window_imp.albums_page.get()),
            actions::menu::artists_play_mode(window_imp.artists_page.get()),
            actions::menu::album_page_play_mode(Rc::clone(&window_imp.album_pages)),
            actions::menu::artist_page_play_mode(Rc::clone(&window_imp.artist_pages)),
        ]);
        self.insert_action_group("menu", Some(&menu_actions));
    }
}
