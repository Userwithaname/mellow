use adw::{self, prelude::*};
use gtk::{self, Align, Orientation, gdk::Paintable, glib, pango::EllipsizeMode};
use relm4::prelude::*;
use std::sync::mpsc;

use crate::{PlayerRequest, PlayerResponse};

pub const APP_NAME: &str = "Mellow";
pub const APP_ID: &str = "com.github.userwithaname.Mellow";

pub fn run(
    args: (
        mpsc::SyncSender<PlayerRequest>,
        mpsc::Receiver<PlayerResponse>,
    ),
) {
    glib::set_application_name(APP_NAME);
    glib::set_program_name(Some(APP_NAME));
    RelmApp::new(APP_ID).with_args(vec![]).run::<App>(args);
}

struct App {
    // TODO: Current player state
    // TODO: Current track info
    player_tx: mpsc::SyncSender<PlayerRequest>,
    ui_rx: mpsc::Receiver<PlayerResponse>,
}

// TODO: When queue is empty, display a landing page

#[relm4::component]
impl SimpleComponent for App {
    type Init = (
        mpsc::SyncSender<PlayerRequest>,
        mpsc::Receiver<PlayerResponse>,
    );
    type Input = PlayerRequest;
    type Output = ();

    view! {
        gtk::ApplicationWindow {
            set_title: Some(APP_NAME),
            set_icon_name: Some(APP_ID),
            set_default_size: (250, 430),

            #[wrap(Some)]
            set_titlebar = &adw::HeaderBar {
                set_css_classes: &["flat"],
                set_show_title: false,
            },

            gtk::WindowHandle { gtk::Box {
                set_margin_top: 0,
                set_margin_bottom: 12,
                set_margin_horizontal: 26,
                set_hexpand: true,
                set_vexpand: true,
                set_valign: Align::Center,
                set_orientation: Orientation::Vertical,
                set_spacing: 6,

                // TODO: Display the currently playing song album cover
                gtk::Picture { // Album cover
                    set_paintable: Some(&Paintable::new_empty(1, 1)),
                    set_content_fit: gtk::ContentFit::Contain,
                    set_halign: Align::Center,
                    set_size_request: (162, 162),
                    set_css_classes: &["card"],
                },

                // TODO: Marquee long titles
                gtk::Label { // Song title
                    set_label: "Song Title",
                    set_css_classes: &["heading"],
                    set_ellipsize: EllipsizeMode::End,
                    set_margin_top: 6,
                },
                gtk::Label { // Album title
                    set_label: "Album Title",
                    set_css_classes: &["caption-heading"],
                    set_ellipsize: EllipsizeMode::End,
                },
                gtk::Label { // Artist name
                    set_label: "Artist Name",
                    set_css_classes: &["caption-heading"],
                    set_ellipsize: EllipsizeMode::End,
                    set_margin_bottom: 6,
                },

                // TODO: Overlay media controls & auto-hide
                gtk::Box { // Media controls toolbar
                    set_orientation: Orientation::Vertical,
                    set_hexpand: true,
                    set_css_classes: &["toolbar", "osd"],

                    gtk::Box { // Main media buttons
                        set_orientation: Orientation::Horizontal,
                        set_halign: Align::Center,
                        set_hexpand: true,
                        set_margin_horizontal: 6,
                        set_spacing: 12,

                        // TODO: Change icons based on state

                        gtk::Button { // Skip backward
                            set_icon_name: "media-skip-backward-symbolic",
                            set_css_classes: &["circular"],

                            connect_clicked => PlayerRequest::SkipPrevious,
                        },
                        gtk::Button { // Pause/play
                            set_icon_name: "media-playback-start-symbolic",
                            set_css_classes: &["circular"],

                            connect_clicked => PlayerRequest::PlayOrPause,
                        },
                        gtk::Button { // Skip forward
                            set_icon_name: "media-skip-forward-symbolic",
                            set_css_classes: &["circular"],

                            connect_clicked => PlayerRequest::SkipNext,
                            // connect_clicked: // TODO
                        },
                    },

                    // TODO: Disable seek bar when no song is active
                    // TODO: Update the seek bar and labels to show the correct time
                    gtk::Box { // Seek controls
                        set_hexpand: true,

                        gtk::Label {
                            // set_label: "0:00",
                            set_label: "-:--",
                            set_css_classes: &["numeric"],
                            set_halign: Align::Start,
                        },
                        gtk::Scale {
                            set_range: (0.0, 1.0),
                            set_orientation: Orientation::Horizontal,
                            set_hexpand: true,
                            set_margin_horizontal: 6,

                            // TODO: Support seeking
                        },
                        gtk::Label {
                            // set_label: "1:23",
                            set_label: "-:--",
                            set_css_classes: &["numeric"],
                            set_halign: Align::End,
                        }
                    }
                }
            }},
        }
    }

    fn init(
        args: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = App {
            player_tx: args.0,
            ui_rx: args.1,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, request: Self::Input, _sender: ComponentSender<Self>) {
        self.player_tx.send(request).unwrap();
    }
}
