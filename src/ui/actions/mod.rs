use adw::{prelude::*, subclass::prelude::*};
use glib::GString;
use gtk::{gio, glib};

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

        let window = self.imp();

        let player_actions = gio::SimpleActionGroup::new();
        player_actions.add_action_entries({
            let player = window.main_player.imp();
            [
                actions::player::skip_prev(player),
                actions::player::play_pause(player),
                actions::player::skip_next(player),
                actions::player::play_all_songs(&window.songs_page),
                actions::player::play_all_albums(&window.albums_page),
                actions::player::play_all_artists(&window.artists_page),
                actions::player::queue_visible_album(window.album_pages.static_ref()),
                actions::player::queue_visible_artist(window.artist_pages.static_ref()),
                actions::player::refresh_library(),
            ]
        });
        self.insert_action_group("player", Some(&player_actions));

        let ui_actions = gio::SimpleActionGroup::new();
        ui_actions.add_action_entries([
            actions::ui::open_sheet(window),
            actions::ui::close_sheet(window),
            actions::ui::toggle_sheet(window),
            actions::ui::open_library(window),
            actions::ui::open_playing(window),
            actions::ui::open_settings(window),
            actions::ui::playing_nav_push(window.playing.get()),
            actions::ui::playing_nav_pop(window.playing.get()),
            actions::ui::library_nav_pop(window.library.get()),
            actions::ui::library_search(window),
        ]);
        self.insert_action_group("ui", Some(&ui_actions));

        let menu_actions = gio::SimpleActionGroup::new();
        menu_actions.add_action_entries([
            actions::menu::songs_sort_mode(window.songs_page.get(), songs_sort),
            actions::menu::albums_sort_mode(window.albums_page.get(), albums_sort),
            actions::menu::artists_sort_mode(window.artists_page.get(), artists_sort),
            actions::menu::songs_play_mode(window.songs_page.get()),
            actions::menu::albums_play_mode(window.albums_page.get()),
            actions::menu::artists_play_mode(window.artists_page.get()),
            actions::menu::album_page_play_mode(window.album_pages.static_ref()),
            actions::menu::artist_page_play_mode(window.artist_pages.static_ref()),
        ]);
        self.insert_action_group("menu", Some(&menu_actions));
    }
}
