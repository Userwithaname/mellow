use adw::subclass::prelude::*;
use core::cell::{Cell, RefCell};
use gtk::{CompositeTemplate, glib};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::excuses::EXP_RX;
use crate::library::{SharedAlbum, ToQueue};
use crate::player::{PlayerRequest, QueueItem, player_tx};
use crate::ui::{Rating, show_queue};
use crate::ui::{UpdateUI, ui_tx};

#[derive(Default, CompositeTemplate)]
#[template(file = "album_page.ui")]
pub struct AlbumPage {
    pub album: RefCell<Option<SharedAlbum>>,
    pub cancel_artowrk_loading: Arc<AtomicBool>,

    #[template_child]
    play_button: TemplateChild<adw::SplitButton>,
    #[template_child]
    pub album_cover: TemplateChild<gtk::Picture>,
    #[template_child]
    pub album_title: TemplateChild<gtk::Label>,
    #[template_child]
    pub artist_name: TemplateChild<gtk::Label>,
    #[template_child]
    pub year: TemplateChild<gtk::Label>,

    #[template_child]
    pub rating: TemplateChild<Rating>,

    #[template_child]
    pub details: TemplateChild<gtk::Label>,

    #[template_child]
    pub album_pref_page: TemplateChild<adw::PreferencesPage>,

    shuffle: Cell<bool>,
}

#[gtk::template_callbacks]
impl AlbumPage {
    #[inline]
    pub fn set_shuffle(&self, shuffle: bool) {
        self.shuffle.set(shuffle);
        self.play_button.set_icon_name(match shuffle {
            false => "media-playback-start-symbolic",
            true => "media-playlist-shuffle-symbolic",
        });
    }
    #[template_callback]
    pub fn handle_play_now(&self) {
        Self::play_now(self.all_songs(), self.shuffle.get());
    }
    #[inline]
    pub fn play_now(queue: Vec<QueueItem>, shuffle: bool) {
        let player_tx = player_tx();
        let _ = player_tx.send(PlayerRequest::LoadQueue {
            queue,
            shuffled: match shuffle {
                true => Some(vec![]),
                false => None,
            },
            track: 0,
        });
        let _ = player_tx.send(PlayerRequest::TogglePlay(Some(true)));
        let ui_tx = ui_tx();
        let _ = ui_tx.send(UpdateUI::OpenSheet(false));
        let _ = ui_tx.send(UpdateUI::FocusPlaying);
    }
    #[inline]
    pub fn play_disc(&self, disc_number: u32) {
        Self::play_now(self.songs_from_disc(disc_number), false);
    }
    #[inline]
    pub fn add_to_queue(queue: Vec<QueueItem>) {
        let _ = player_tx().send(PlayerRequest::AppendQueue(queue));
    }
    #[inline]
    pub fn add_disc_to_queue(&self, disc_number: u32) {
        Self::add_to_queue(self.songs_from_disc(disc_number));
        let _ = ui_tx().send(UpdateUI::Notification(
            format!(
                "Disc {disc_number} \"{}\" has been added to queue",
                self.album_title.label()
            ),
            Some(Box::new(("View", Box::new(show_queue)))),
        ));
    }
    #[inline]
    pub fn all_songs(&self) -> Vec<QueueItem> {
        self.album.borrow().as_ref().unwrap().to_queue()
    }
    #[inline]
    pub fn songs_from_disc(&self, disc_number: u32) -> Vec<QueueItem> {
        (self.album.borrow().as_ref().unwrap())
            .to_queue()
            .into_iter()
            .filter(|item| item.as_song().info().load_basic().as_ref().unwrap().disc == disc_number)
            .collect()
    }
    #[template_callback]
    pub fn handle_go_to_artist(&self) {
        (ui_tx().send(UpdateUI::ArtistPage(
            (self.album.borrow().as_ref().unwrap().lock().unwrap()).artist_cloned(),
        )))
        .expect(EXP_RX);
    }
}

#[glib::object_subclass]
impl ObjectSubclass for AlbumPage {
    const NAME: &str = "MellowAlbumPage";
    type Type = super::AlbumPage;
    type ParentType = adw::NavigationPage;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
        class.bind_template_callbacks();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for AlbumPage {}
impl WidgetImpl for AlbumPage {}
impl NavigationPageImpl for AlbumPage {}

impl Drop for AlbumPage {
    fn drop(&mut self) {
        self.cancel_artowrk_loading.store(true, Ordering::Relaxed);
        let Some(album) = self.album.take() else {
            return;
        };
        if let Ok(album) = album.try_lock() {
            album.first_song().info().try_unload_detailed();
        }
    }
}
