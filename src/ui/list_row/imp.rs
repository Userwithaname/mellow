use adw::{prelude::*, subclass::prelude::*};
use core::cell::RefCell;
use gtk::CompositeTemplate;
use gtk::glib;

#[derive(Default, CompositeTemplate)]
#[template(file = "list_row.ui")]
pub struct ListRow {
    #[template_child]
    pub prefix_image: TemplateChild<gtk::Picture>,
    #[template_child]
    pub suffix_label: TemplateChild<gtk::Label>,

    #[template_child]
    pub open_subpage_icon: TemplateChild<gtk::Image>,
    #[template_child]
    pub selection_toggle: TemplateChild<gtk::CheckButton>,

    pub bindings: RefCell<Vec<glib::Binding>>,
}

impl ListRow {
    #[inline]
    pub fn set_selected(&self, selected: bool) {
        self.selection_toggle.set_active(selected);
    }
}

#[glib::object_subclass]
impl ObjectSubclass for ListRow {
    const NAME: &str = "MellowListRow";
    type Type = super::ListRow;
    type ParentType = adw::ActionRow;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for ListRow {}
impl WidgetImpl for ListRow {}
impl ActionRowImpl for ListRow {}
impl PreferencesRowImpl for ListRow {}
impl ListBoxRowImpl for ListRow {}
