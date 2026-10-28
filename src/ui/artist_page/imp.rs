use adw::subclass::prelude::*;
use core::cell::{Cell, RefCell};
use gtk::{CompositeTemplate, glib};

use crate::library::unload_unused::UsedBy;
use crate::library::{SharedArtist, ToQueue, ToShuffledQueue};
use crate::player::{PlayerRequest, player_tx};
use crate::ui::{UpdateUI, show_queue, ui_tx};

#[derive(Default, CompositeTemplate)]
#[template(file = "artist_page.ui")]
pub struct ArtistPage {
    pub artist: RefCell<Option<SharedArtist>>,

    #[template_child]
    play_button: TemplateChild<adw::SplitButton>,
    #[template_child]
    pub artist_name: TemplateChild<gtk::Label>,
    #[template_child]
    pub album_count: TemplateChild<gtk::Label>,
    #[template_child]
    pub albums_list: TemplateChild<gtk::ListBox>,

    shuffle: Cell<bool>,
}

#[gtk::template_callbacks]
impl ArtistPage {
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
        match self.shuffle.get() {
            true => self.play_shuffled(),
            false => self.play_sequential(),
        }
    }
    #[template_callback]
    pub fn play_sequential(&self) {
        let player_tx = player_tx();
        PlayerRequest::LoadQueue {
            queue: self.artist.borrow().as_ref().unwrap().to_queue(),
            shuffled: None,
            track: 0,
        }
        .send_with(player_tx);
        PlayerRequest::TogglePlay(Some(true)).send_with(player_tx);
        let ui_tx = ui_tx();
        UpdateUI::OpenSheet(false).send_with(ui_tx);
        UpdateUI::FocusPlaying.send_with(ui_tx);
    }
    #[template_callback]
    pub fn play_shuffled(&self) {
        let player_tx = player_tx();
        PlayerRequest::LoadQueue {
            queue: self.artist.borrow().as_ref().unwrap().to_shuffled_queue(),
            shuffled: None,
            track: 0,
        }
        .send_with(player_tx);
        PlayerRequest::TogglePlay(Some(true)).send_with(player_tx);
        let ui_tx = ui_tx();
        UpdateUI::OpenSheet(false).send_with(ui_tx);
        UpdateUI::FocusPlaying.send_with(ui_tx);
    }
    #[inline]
    pub fn add_to_queue(&self) {
        let ui_tx = ui_tx();
        UpdateUI::RunAction("ui.library_nav_pop").send_with(ui_tx);
        PlayerRequest::AppendQueue(self.artist.borrow().as_ref().unwrap().to_queue()).send();
        UpdateUI::Notification(
            format!(
                "Artist \"{}\" has been added to queue",
                self.artist_name.label()
            ),
            Some(Box::new(("View", Box::new(show_queue)))),
        )
        .send_with(ui_tx);
    }
}

#[glib::object_subclass]
impl ObjectSubclass for ArtistPage {
    const NAME: &str = "MellowArtistPage";
    type Type = super::ArtistPage;
    type ParentType = adw::NavigationPage;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
        class.bind_template_callbacks();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for ArtistPage {}
impl WidgetImpl for ArtistPage {}
impl NavigationPageImpl for ArtistPage {}

impl Drop for ArtistPage {
    fn drop(&mut self) {
        let Some(artist) = self.artist.take() else {
            return;
        };
        if let Ok(artist) = artist.try_lock() {
            for album in artist.albums() {
                if let Ok(album) = album.try_lock() {
                    let first_song = album.first_song();
                    // Unload thumbnail if still unused
                    first_song.info().mark_thumbnail_unused_by(UsedBy::None);
                }
            }
        }
    }
}
