use adw::{prelude::*, subclass::prelude::*};
use gtk::CompositeTemplate;
use gtk::{gio, glib, pango};

use core::cell::{Cell, RefCell};

use crate::library::lyrics::{Lyrics, SyncedLyric};
use crate::ui::lyric_object::LyricObject;

#[derive(Default, CompositeTemplate)]
#[template(file = "lyrics_page.ui")]
pub struct LyricsPage {
    #[template_child]
    pub song_title: TemplateChild<gtk::Label>,
    #[template_child]
    pub lyrics: TemplateChild<gtk::ListView>,

    lyric_index: Cell<usize>,
    lyric_objects: RefCell<Vec<LyricObject>>,
}

impl LyricsPage {
    #[inline]
    pub fn load_lyrics(&self, lyrics: &Lyrics) {
        let lyric_objects = match lyrics {
            Lyrics::Unsynced(lyrics) if !lyrics.is_empty() => {
                vec![LyricObject::new(0, 0, lyrics.to_owned())]
            }
            Lyrics::Synced(synced_lyrics) => (synced_lyrics.iter().enumerate())
                .map(|(index, SyncedLyric { time_ms, lyric })| {
                    LyricObject::new(index as u32, *time_ms as u64, lyric.to_owned())
                })
                .collect(),
            Lyrics::Unsynced(_) => vec![LyricObject::new(0, 0, "Lyrics not available".to_owned())],
        };

        let model = gio::ListStore::new::<LyricObject>();
        model.extend_from_slice(&lyric_objects);
        self.lyric_objects.replace(lyric_objects);

        let selection_model = gtk::NoSelection::new(Some(model));
        self.lyrics.set_model(Some(&selection_model));
    }

    #[inline]
    pub fn update_synced_lyrics(&self, time_ms: u64) {
        let old_index = self.lyric_index.get();
        let mut new_index = old_index;
        let lyric_objects = self.lyric_objects.borrow();
        if lyric_objects.len() <= 1 {
            return;
        }
        if time_ms < lyric_objects[0].time() {
            lyric_objects[old_index].set_styles(vec!["body".to_owned()]);
            lyric_objects[new_index].set_styles(vec!["body".to_owned()]);
            self.lyric_index.set(0);
            return;
        }

        // Starting at `old_index` means fewer lyric times are checked; the first loop
        // is necessary to find the correct lyric after seeking
        for (index, object) in lyric_objects.iter().enumerate().take(old_index).rev() {
            let lyric_time = object.time();
            match lyric_time >= time_ms {
                true => new_index = index,
                false => break,
            }
        }
        for (index, object) in lyric_objects.iter().enumerate().skip(old_index) {
            let lyric_time = object.time();
            match time_ms >= lyric_time {
                true => new_index = index,
                false => break,
            }
        }

        if new_index != old_index {
            lyric_objects[old_index].set_styles(vec!["body".to_owned()]);
            lyric_objects[new_index].set_styles(vec!["heading".to_owned()]);
            self.lyric_index.set(new_index);
            // IDEA: Use the scroll position to determine whether `new_index` is above
            // or below, and offset it so a few extra items are kept in view as well
            self.lyrics
                .scroll_to(new_index as u32, gtk::ListScrollFlags::FOCUS, None);
        }
    }

    #[inline]
    fn setup_factory(&self) {
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(move |_, list_item| {
            let lyric_label = gtk::Label::builder()
                .justify(gtk::Justification::Center)
                .halign(gtk::Align::Center)
                .hexpand(true)
                .wrap(true)
                .wrap_mode(pango::WrapMode::Word)
                .margin_start(12)
                .margin_end(12)
                .build();
            list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem")
                .set_child(Some(&lyric_label));
        });
        factory.connect_bind(move |_, list_item| {
            let list_item = list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem");
            let lyric_object = list_item
                .item()
                .and_downcast::<LyricObject>()
                .expect("Needs to be LyricObject");
            let lyric_label = list_item
                .child()
                .and_downcast::<gtk::Label>()
                .expect("Needs to be gtk::Label");
            lyric_label.set_label(&lyric_object.lyric());
            lyric_object
                .bind_property("styles", &lyric_label, "css-classes")
                .sync_create()
                .build();
        });

        self.lyrics.set_factory(Some(&factory));
    }
}

#[glib::object_subclass]
impl ObjectSubclass for LyricsPage {
    const NAME: &str = "MellowLyricsPage";
    type Type = super::LyricsPage;
    type ParentType = adw::NavigationPage;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}
impl ObjectImpl for LyricsPage {
    fn constructed(&self) {
        self.setup_factory();
    }
}
impl WidgetImpl for LyricsPage {}
impl NavigationPageImpl for LyricsPage {}
