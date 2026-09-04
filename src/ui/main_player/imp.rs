use adw::{prelude::*, subclass::prelude::*};
use gtk::CompositeTemplate;
use gtk::glib;

use crate::excuses::EXP_RX;
use crate::player::{PlayerRequest, player_tx};
use crate::ui::gtk_ext::GtkPictureExt;
use crate::util::approx_eq;

#[derive(Default, CompositeTemplate)]
#[template(file = "main_player.ui")]
pub struct MainPlayer {
    #[template_child]
    pub album_cover: TemplateChild<gtk::Picture>,
    #[template_child]
    pub song_title: TemplateChild<gtk::Label>,
    #[template_child]
    pub album_title: TemplateChild<gtk::Label>,
    #[template_child]
    pub artist_name: TemplateChild<gtk::Label>,

    #[template_child]
    pub media_controls: TemplateChild<gtk::Box>,
    #[template_child]
    pub pause_button: TemplateChild<gtk::Button>,
    #[template_child]
    pub seek_bar: TemplateChild<gtk::Scale>,
    #[template_child]
    pub current_time: TemplateChild<gtk::Label>,
    #[template_child]
    pub duration: TemplateChild<gtk::Label>,
}

#[gtk::template_callbacks]
impl MainPlayer {
    #[template_callback]
    pub fn handle_skip_prev(&self) {
        player_tx().send(PlayerRequest::SkipPrevious).expect(EXP_RX);
    }
    #[template_callback]
    pub fn handle_play_pause(&self) {
        player_tx()
            .send(PlayerRequest::TogglePlay(None))
            .expect(EXP_RX);
    }
    #[template_callback]
    pub fn handle_skip_next(&self) {
        player_tx().send(PlayerRequest::SkipNext).expect(EXP_RX);
    }
    #[template_callback]
    pub fn handle_seek(&self, _: gtk::ScrollType, value: f64) -> glib::Propagation {
        if approx_eq(value, self.seek_bar.value()) {
            return glib::Propagation::Stop;
        }
        player_tx().send(PlayerRequest::Seek(value)).expect(EXP_RX);
        glib::Propagation::Proceed
    }
}

#[glib::object_subclass]
impl ObjectSubclass for MainPlayer {
    const NAME: &str = "MellowMainPlayer";
    type Type = super::MainPlayer;
    type ParentType = gtk::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
        class.bind_template_callbacks();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for MainPlayer {
    fn constructed(&self) {
        self.album_cover.set_blank();
        self.obj().set_state(false, false);
    }
}
impl WidgetImpl for MainPlayer {}
impl BoxImpl for MainPlayer {}
