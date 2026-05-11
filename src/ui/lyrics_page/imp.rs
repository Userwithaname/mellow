use adw::subclass::prelude::*;
use gtk::CompositeTemplate;
use gtk::glib;

#[derive(Default, CompositeTemplate)]
#[template(file = "lyrics_page.ui")]
pub struct LyricsPage {
    #[template_child]
    pub song_title: TemplateChild<gtk::Label>,
    #[template_child]
    pub lyrics: TemplateChild<gtk::Label>,
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
        self.obj().set_content("", "");
    }
}
impl WidgetImpl for LyricsPage {}
impl NavigationPageImpl for LyricsPage {}
