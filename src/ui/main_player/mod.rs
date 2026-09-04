use adw::{prelude::*, subclass::prelude::*};
use gtk::Orientation;
use gtk::{gdk, glib};

use crate::excuses::EXP_RX;
use crate::player::{PlayerRequest, player_tx};
use crate::ui::gtk_ext::GtkPictureExt;
use crate::util::format_duration_ms;

pub mod imp;

glib::wrapper! {
    pub struct MainPlayer(ObjectSubclass<imp::MainPlayer>)
        @extends gtk::Box, gtk::Widget,
        @implements
            gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::Orientable, gtk::ConstraintTarget;
}

impl MainPlayer {
    /// Initializes the seek bar for the player UI
    ///
    /// # Panics
    /// Panics at runtime upon interaction with the seek
    /// bar if the player channel is closed
    #[inline]
    pub fn init_seek(&self) {
        // Locate the `gtk::GestureClick` controller already present on `GtkScale`
        // and connect a closure to resume playback when seek bar is released
        let mut i = 0;
        let controllers = self.imp().seek_bar.observe_controllers();
        while let Some(controller) = controllers.item(i) {
            if let Some(gesture_click) = controller.downcast_ref::<gtk::GestureClick>() {
                gesture_click.connect_released(|_, _, _, _| {
                    player_tx().send(PlayerRequest::SeekDone).expect(EXP_RX);
                });
                break;
            }
            i += 1;
        }

        debug_assert!(
            i < controllers.n_items(),
            "Could not find `GtkGestureClick` controller on the seek bar"
        );
    }

    #[inline]
    pub fn set_state(&self, playing: bool, interactive: bool) {
        let ui = self.imp();
        ui.pause_button.set_icon_name(match playing {
            true => "media-playback-pause-symbolic",
            false => "media-playback-start-symbolic",
        });
        ui.media_controls.set_sensitive(interactive);
    }

    #[inline]
    pub fn set_info(
        &self,
        song: &str,
        album: &str,
        artist: &str,
        artwork: Option<&gdk::Texture>,
        song_duration_ms: u64,
    ) {
        let ui = self.imp();

        ui.album_cover.set_paintable_or_blank(artwork);
        ui.song_title.set_label(song);
        ui.album_title.set_label(album);
        ui.artist_name.set_label(artist);

        match song_duration_ms {
            0 => ui.duration.set_label("-:--"),
            _ => ui.duration.set_label(&format_duration_ms(song_duration_ms)),
        }
    }
    #[inline]
    pub fn set_artwork(&self, artwork: Option<&gdk::Texture>) {
        self.imp().album_cover.set_paintable_or_blank(artwork);
    }
    pub fn reset_info(&self) {
        let ui = self.imp();

        ui.album_cover.set_blank();
        ui.song_title.set_label("");
        ui.album_title.set_label("");
        ui.artist_name.set_label("");

        ui.current_time.set_label("-:--");
        ui.duration.set_label("-:--");
    }

    #[inline]
    pub fn set_time(&self, time_ms: Option<u64>, duration_ms: f64) {
        let ui = self.imp();
        if let Some(time_ms) = time_ms {
            ui.current_time.set_label(&format_duration_ms(time_ms));
            ui.seek_bar.set_child_visible(true);
            if duration_ms > 0.0 {
                ui.seek_bar.set_sensitive(true);
                #[allow(clippy::cast_precision_loss)]
                ui.seek_bar.set_value(time_ms as f64 / duration_ms);
            } else {
                ui.seek_bar.set_sensitive(false);
                ui.seek_bar.set_value(0.0);
            }
        } else {
            ui.current_time.set_label("-:--");
            ui.seek_bar.set_child_visible(false);
            ui.seek_bar.set_sensitive(false);
            ui.seek_bar.set_value(0.0);
        }
    }

    /// Sets main player spacing based on available space
    #[inline]
    pub fn update_spacing(&self, height: i32) {
        const SPACERS: i32 = 2;
        const WITH_OUTER: i32 = SPACERS + 3;
        let headroom = height + self.spacing() * SPACERS
            - self.size(Orientation::Vertical)
            - self.margin_top()
            - self.margin_bottom();
        self.set_spacing((headroom / WITH_OUTER).max(6));
    }

    #[inline]
    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.imp().pause_button.icon_name() == Some("media-playback-pause-symbolic".into())
    }
}
