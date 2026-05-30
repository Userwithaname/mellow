use adw::subclass::prelude::*;
use core::cell::RefCell;
use glib::clone;
use gtk::{gio, glib};
use std::rc::Rc;

use crate::ui::main_player::imp::MainPlayer;
use crate::ui::{AlbumPage, AlbumsPage, ArtistPage, ArtistsPage, SongsPage};

#[inline]
pub fn skip_prev(player: &MainPlayer) -> gio::ActionEntry<gio::SimpleActionGroup> {
    gio::ActionEntry::builder("skip_prev")
        .activate(clone!(
            #[weak]
            player,
            move |_, _, _| player.handle_skip_prev()
        ))
        .build()
}
#[inline]
pub fn play_pause(player: &MainPlayer) -> gio::ActionEntry<gio::SimpleActionGroup> {
    gio::ActionEntry::builder("play_pause")
        .activate(clone!(
            #[weak]
            player,
            move |_, _, _| player.handle_play_pause()
        ))
        .build()
}
#[inline]
pub fn skip_next(player: &MainPlayer) -> gio::ActionEntry<gio::SimpleActionGroup> {
    gio::ActionEntry::builder("skip_next")
        .activate(clone!(
            #[weak]
            player,
            move |_, _, _| player.handle_skip_next()
        ))
        .build()
}
#[inline]
pub fn play_all_songs(songs_page: &SongsPage) -> gio::ActionEntry<gio::SimpleActionGroup> {
    gio::ActionEntry::builder("play_all_songs")
        .activate(clone!(
            #[weak(rename_to=songs_page)]
            songs_page.imp(),
            move |_, _, _| songs_page.handle_play_now()
        ))
        .build()
}
#[inline]
pub fn play_all_albums(albums_page: &AlbumsPage) -> gio::ActionEntry<gio::SimpleActionGroup> {
    gio::ActionEntry::builder("play_all_albums")
        .activate(clone!(
            #[weak(rename_to=albums_page)]
            albums_page.imp(),
            move |_, _, _| albums_page.handle_play_now()
        ))
        .build()
}
#[inline]
pub fn play_all_artists(artists_page: &ArtistsPage) -> gio::ActionEntry<gio::SimpleActionGroup> {
    gio::ActionEntry::builder("play_all_artists")
        .activate(clone!(
            #[weak(rename_to=artists_page)]
            artists_page.imp(),
            move |_, _, _| artists_page.handle_play_now()
        ))
        .build()
}
#[inline]
pub fn queue_visible_album(
    album_pages: Rc<RefCell<Vec<AlbumPage>>>,
) -> gio::ActionEntry<gio::SimpleActionGroup> {
    gio::ActionEntry::builder("queue_visible_album")
        .activate(move |_, _, _| {
            if let Some(album_page) = album_pages.borrow().last() {
                album_page.add_to_queue();
            }
        })
        .build()
}
#[inline]
pub fn queue_visible_artist(
    artist_pages: Rc<RefCell<Vec<ArtistPage>>>,
) -> gio::ActionEntry<gio::SimpleActionGroup> {
    gio::ActionEntry::builder("queue_visible_artist")
        .activate(move |_, _, _| {
            if let Some(artist_page) = artist_pages.borrow().last() {
                artist_page.imp().add_to_queue();
            }
        })
        .build()
}
