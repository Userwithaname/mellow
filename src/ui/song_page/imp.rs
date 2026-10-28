use adw::{prelude::*, subclass::prelude::*};
use core::cell::{Cell, RefCell};
use gtk::CompositeTemplate;
use gtk::glib;
use std::sync::Arc;

use crate::excuses::{ACTION_ERR, EXP_INIT};
use crate::library::{SharedSong, ToQueue};
use crate::player::{PlayerRequest, QueueItem, player_tx};
use crate::ui::{Rating, show_queue};
use crate::ui::{UpdateUI, ui_tx};

#[derive(Default, CompositeTemplate)]
#[template(file = "song_page.ui")]
pub struct SongPage {
    #[template_child]
    pub song_title: TemplateChild<gtk::Label>,
    #[template_child]
    pub album_title: TemplateChild<gtk::Label>,
    #[template_child]
    pub artist_name: TemplateChild<gtk::Label>,

    #[template_child]
    pub rating: TemplateChild<Rating>,

    pub index: Cell<usize>,
    pub shared_song: RefCell<Option<SharedSong>>,
    pub context: RefCell<Option<Box<dyn ToQueue + Send>>>,
}

#[gtk::template_callbacks]
impl SongPage {
    #[template_callback]
    pub fn handle_play_now(&self) {
        (self.obj().activate_action("ui.library_nav_pop", None)).expect(ACTION_ERR);
        let player_tx = player_tx();
        PlayerRequest::LoadQueue {
            queue: self.context.borrow().as_ref().expect(EXP_INIT).to_queue(),
            shuffled: None,
            track: self.index.get(),
        }
        .send_with(player_tx);
        PlayerRequest::TogglePlay(Some(true)).send_with(player_tx);
        let ui_tx = ui_tx();
        UpdateUI::OpenSheet(false).send_with(ui_tx);
        UpdateUI::FocusPlaying.send_with(ui_tx);
    }
    #[template_callback]
    pub fn handle_play_next(&self) {
        (self.obj().activate_action("ui.library_nav_pop", None)).expect(ACTION_ERR);
        let song = self.shared_song.borrow();
        let song = song.as_ref().unwrap();
        PlayerRequest::InsertRelative(Box::new((1, QueueItem::Song(Arc::clone(song))))).send();
        UpdateUI::Notification(
            format!(
                "Song \"{}\" will play next in the queue",
                song.info().inspect_basic().as_ref().unwrap().title
            ),
            Some(Box::new(("View", Box::new(show_queue)))),
        )
        .send();
    }
    #[template_callback]
    pub fn handle_add_to_queue(&self) {
        (self.obj().activate_action("ui.library_nav_pop", None)).expect(ACTION_ERR);
        let song = self.shared_song.borrow();
        let song = song.as_ref().unwrap();
        PlayerRequest::Append(QueueItem::Song(Arc::clone(song))).send();
        UpdateUI::Notification(
            format!(
                "Song \"{}\" has been added to queue",
                song.info().inspect_basic().as_ref().unwrap().title
            ),
            Some(Box::new(("View", Box::new(show_queue)))),
        )
        .send();
    }
    #[template_callback]
    pub fn handle_go_to_album(&self) {
        // Do nothing if the song is not from the user's library
        // (button should be greyed out anyway)
        if let Some(album) = &*self.shared_song.borrow().as_ref().unwrap().get_album() {
            UpdateUI::AlbumPage(Arc::clone(album)).send();
        }
    }
    #[template_callback]
    pub fn handle_go_to_artist(&self) {
        // Do nothing if the song is not from the user's library
        // (button should be greyed out anyway)
        if let Some(album) = &*self.shared_song.borrow().as_ref().unwrap().get_album() {
            UpdateUI::ArtistPage(album.lock().unwrap().artist_cloned()).send();
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for SongPage {
    const NAME: &str = "MellowSongPage";
    type Type = super::SongPage;
    type ParentType = adw::NavigationPage;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
        class.bind_template_callbacks();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for SongPage {
    fn constructed(&self) {
        self.album_title.set_cursor_from_name(Some("pointer"));
        let click = gtk::GestureClick::builder()
            .propagation_phase(gtk::PropagationPhase::Capture)
            .build();
        click.connect_released(glib::clone!(
            #[weak(rename_to=subpage)]
            self,
            #[weak(rename_to=label)]
            self.album_title,
            move |_, _, pos_x, pos_y| if (0.0..label.width() as f64).contains(&pos_x)
                && (0.0..label.height() as f64).contains(&pos_y)
            {
                subpage.handle_go_to_album();
            }
        ));
        self.album_title.add_controller(click);

        self.artist_name.set_cursor_from_name(Some("pointer"));
        let click = gtk::GestureClick::builder()
            .propagation_phase(gtk::PropagationPhase::Capture)
            .build();
        click.connect_released(glib::clone!(
            #[weak(rename_to=subpage)]
            self,
            #[weak(rename_to=label)]
            self.artist_name,
            move |_, _, pos_x, pos_y| if (0.0..label.width() as f64).contains(&pos_x)
                && (0.0..label.height() as f64).contains(&pos_y)
            {
                subpage.handle_go_to_artist();
            }
        ));
        self.artist_name.add_controller(click);
    }
}
impl WidgetImpl for SongPage {}
impl NavigationPageImpl for SongPage {}
