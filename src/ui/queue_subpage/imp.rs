use adw::{prelude::*, subclass::prelude::*};
use core::cell::{Cell, RefCell};
use gtk::CompositeTemplate;
use gtk::glib;
use std::sync::Arc;

use crate::excuses::{ACTION_ERR, EXP_RX};
use crate::library::SharedAlbum;
use crate::player::{PlayerRequest, QueueItem, player_tx};
use crate::ui::Rating;
use crate::ui::{UpdateUI, ui_tx};

#[derive(Default, CompositeTemplate)]
#[template(file = "queue_subpage.ui")]
pub struct QueueSubpage {
    pub index: Cell<usize>,
    pub stop_after: Cell<bool>,
    pub queue_item: RefCell<QueueItem>,
    pub album: RefCell<Option<SharedAlbum>>,

    #[template_child]
    pub song_title: TemplateChild<gtk::Label>,
    #[template_child]
    pub album_title: TemplateChild<gtk::Label>,
    #[template_child]
    pub artist_name: TemplateChild<gtk::Label>,

    #[template_child]
    pub rating: TemplateChild<Rating>,

    #[template_child]
    pub play_now_button: TemplateChild<adw::ActionRow>,
    #[template_child]
    pub stop_after_button: TemplateChild<adw::ActionRow>,
    #[template_child]
    pub stopper_closes_player: TemplateChild<adw::SwitchRow>,

    #[template_child]
    pub remove_song_button: TemplateChild<adw::ActionRow>,
    #[template_child]
    pub remove_stopper_button: TemplateChild<adw::ActionRow>,

    #[template_child]
    pub go_to_album_button: TemplateChild<adw::ActionRow>,
    #[template_child]
    pub go_to_artist_button: TemplateChild<adw::ActionRow>,
}

#[gtk::template_callbacks]
impl QueueSubpage {
    // TODO: Test if `index` is incorrect after processing a stopper

    #[template_callback]
    pub fn handle_play_now(&self) {
        (self.obj().activate_action("ui.close_sheet", None)).expect(ACTION_ERR);
        (self.obj().activate_action("ui.playing_nav_pop", None)).expect(ACTION_ERR);
        let player_tx = player_tx();
        (player_tx.send(PlayerRequest::SkipTo(self.index.get()))).expect(EXP_RX);
        (player_tx.send(PlayerRequest::TogglePlay(Some(true)))).expect(EXP_RX);
    }
    #[template_callback]
    pub fn handle_stop_after(&self) {
        (self.obj().activate_action("ui.playing_nav_pop", None)).expect(ACTION_ERR);
        let index = self.index.get() + 1;
        let stop_after = !self.stop_after.get();
        (player_tx().send(match stop_after {
            true => PlayerRequest::InsertAt(Box::new((index, QueueItem::new_stopper(false)))),
            false => PlayerRequest::RemoveItem(index),
        }))
        .expect(EXP_RX);
        // self.obj().set_stop_after(stop_after);
    }
    #[template_callback]
    pub fn handle_remove_item(&self) {
        (self.obj().activate_action("ui.playing_nav_pop", None)).expect(ACTION_ERR);

        let index = self.index.get();
        player_tx()
            .send(PlayerRequest::RemoveItem(index))
            .expect(EXP_RX);

        (ui_tx().send(UpdateUI::Notification(
            format!("Removed from the queue: \"{}\"", self.song_title.label()),
            Some(Box::new((
                "Undo",
                Box::new(|| player_tx().send(PlayerRequest::Undo).expect(EXP_RX)),
            ))),
        )))
        .expect(EXP_RX);
    }
    #[template_callback]
    pub fn handle_move_up(&self) {
        (self.obj().activate_action("ui.playing_nav_pop", None)).expect(ACTION_ERR);
        (player_tx().send(PlayerRequest::Shift {
            from: self.index.get(),
            by: -1,
        }))
        .expect(EXP_RX);
    }
    #[template_callback]
    pub fn handle_move_down(&self) {
        (self.obj().activate_action("ui.playing_nav_pop", None)).expect(ACTION_ERR);
        (player_tx().send(PlayerRequest::Shift {
            from: self.index.get(),
            by: 1,
        }))
        .expect(EXP_RX);
    }
    #[template_callback]
    pub fn handle_go_to_album(&self) {
        let Some(album) = self.album.borrow().as_ref().map(Arc::clone) else {
            // The button is greyed-out if the song is not from the library,
            // but handling the `None` variant anyway, just in case
            return;
        };
        let ui_tx = ui_tx();
        ui_tx.send(UpdateUI::FocusLibrary).expect(EXP_RX);
        let _ = ui_tx.send(UpdateUI::AlbumPage(album));
        (self.obj().activate_action("ui.playing_nav_pop", None)).expect(ACTION_ERR);
    }
    #[template_callback]
    pub fn handle_go_to_artist(&self) {
        let Some(artist) = (self.album.borrow())
            .as_ref()
            .map(|album| Arc::clone(album.lock().unwrap().artist()))
        else {
            // The button is greyed-out if the song is not from the library,
            // but handling the `None` variant anyway, just in case
            return;
        };
        let ui_tx = ui_tx();
        ui_tx.send(UpdateUI::FocusLibrary).expect(EXP_RX);
        let _ = ui_tx.send(UpdateUI::ArtistPage(artist));
        (self.obj().activate_action("ui.playing_nav_pop", None)).expect(ACTION_ERR);
    }
    #[template_callback]
    pub fn handle_stopper_closes_player(&self) {
        let stopper = self.queue_item.borrow().as_stopper().clone();
        stopper.set_close_player(self.stopper_closes_player.is_active());
        self.obj().show_stopper_info(self.index.get(), &stopper);
        let _ = ui_tx().send(UpdateUI::RedrawQueue);
    }
}

#[glib::object_subclass]
impl ObjectSubclass for QueueSubpage {
    const NAME: &str = "MellowQueueSubpage";
    type Type = super::QueueSubpage;
    type ParentType = adw::NavigationPage;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
        class.bind_template_callbacks();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for QueueSubpage {}
impl WidgetImpl for QueueSubpage {}
impl NavigationPageImpl for QueueSubpage {}
