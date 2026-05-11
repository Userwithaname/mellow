use adw::subclass::prelude::*;
use core::cell::RefCell;
use gtk::CompositeTemplate;
use gtk::glib;

#[derive(Default, CompositeTemplate)]
#[template(file = "item_tile.ui")]
pub struct ItemTile {
    #[template_child]
    pub image: TemplateChild<gtk::Picture>,
    #[template_child]
    pub title: TemplateChild<gtk::Label>,
    #[template_child]
    pub subtitle: TemplateChild<gtk::Label>,

    pub bindings: RefCell<Vec<glib::Binding>>,
}

#[glib::object_subclass]
impl ObjectSubclass for ItemTile {
    const NAME: &str = "MellowItemTile";
    type Type = super::ItemTile;
    type ParentType = gtk::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}
impl ObjectImpl for ItemTile {}
impl WidgetImpl for ItemTile {}
impl BoxImpl for ItemTile {}
