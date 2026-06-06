use adw::subclass::prelude::*;
use core::cell::Cell;
use gtk::CompositeTemplate;
use gtk::glib;

#[derive(Default, CompositeTemplate)]
#[template(file = "library_page.ui")]
pub struct LibraryPage {
    #[template_child]
    pub view_stack: TemplateChild<adw::ViewStack>,
    #[template_child]
    pub ready_stack: TemplateChild<adw::ViewStack>,

    pub is_empty: Cell<bool>,
}

#[glib::object_subclass]
impl ObjectSubclass for LibraryPage {
    const NAME: &str = "MellowLibraryPage";
    type Type = super::LibraryPage;
    type ParentType = adw::NavigationPage;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for LibraryPage {}
impl WidgetImpl for LibraryPage {}
impl NavigationPageImpl for LibraryPage {}
