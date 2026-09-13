use adw::{prelude::*, subclass::prelude::*};
use glib::Properties;
use gtk::glib;

use core::cell::RefCell;

use super::LyricData;

#[derive(Properties, Default)]
#[properties(wrapper_type = super::LyricObject)]
pub struct LyricObject {
    #[property(name = "index", get, set, type = u32, member = index)]
    #[property(name = "time", get, set, type = u64, member = time)]
    #[property(name = "lyric", get, set, type = String, member = lyric)]
    #[property(name = "styles", get, set, type = Vec<String>, member = styles)]
    pub data: RefCell<LyricData>,
}

#[glib::object_subclass]
impl ObjectSubclass for LyricObject {
    const NAME: &str = "MellowLyricObject";
    type Type = super::LyricObject;
}
#[glib::derived_properties]
impl ObjectImpl for LyricObject {}
