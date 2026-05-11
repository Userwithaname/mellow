use adw::subclass::prelude::*;
use core::cell::RefCell;
use gtk::CompositeTemplate;
use gtk::glib;

#[derive(Default, CompositeTemplate)]
#[template(file = "item_row.ui")]
pub struct ItemRow {
    #[template_child]
    pub image: TemplateChild<gtk::Picture>,
    #[template_child]
    pub title: TemplateChild<gtk::Label>,
    #[template_child]
    pub subtitle: TemplateChild<gtk::Label>,

    pub bindings: RefCell<Vec<glib::Binding>>,
}

#[glib::object_subclass]
impl ObjectSubclass for ItemRow {
    const NAME: &str = "MellowItemRow";
    type Type = super::ItemRow;
    type ParentType = gtk::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for ItemRow {}
impl WidgetImpl for ItemRow {}
impl BoxImpl for ItemRow {}
