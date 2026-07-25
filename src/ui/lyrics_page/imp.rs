use adw::subclass::prelude::*;
use gtk::prelude::*;
use gtk::CompositeTemplate;
use gtk::glib;
use std::cell::{Cell, RefCell};
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct LyricLine {
    pub timestamp: Duration,
    pub text: String,
}

#[derive(Default, CompositeTemplate)]
#[template(file = "lyrics_page.ui")]
pub struct LyricsPage {
    #[template_child]
    pub song_title: TemplateChild<gtk::Label>,
    #[template_child]
    pub lyrics: TemplateChild<gtk::Label>,
    #[template_child]
    pub scrolled_window: TemplateChild<gtk::ScrolledWindow>,
    #[template_child]
    pub lyrics_box: TemplateChild<gtk::ListBox>,
    pub lines: RefCell<Vec<LyricLine>>,
    pub current_index: Cell<Option<usize>>,
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
        self.parent_constructed();        
        self.lyrics_box.add_css_class("lyrics-list");        
        self.obj().set_content("", "");
    }
}
impl WidgetImpl for LyricsPage {}
impl NavigationPageImpl for LyricsPage {}
