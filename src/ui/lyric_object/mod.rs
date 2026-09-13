use glib::Object;
use gtk::glib;

mod imp;

glib::wrapper! {
    pub struct LyricObject(ObjectSubclass<imp::LyricObject>);
}

impl LyricObject {
    #[inline]
    #[must_use]
    pub fn new(index: u32, time: u64, lyric: String) -> Self {
        Object::builder()
            .property("index", index)
            .property("time", time)
            .property("lyric", lyric)
            .build()
    }
}

#[derive(Default)]
pub struct LyricData {
    index: u32,
    time: u64,
    lyric: String,
    styles: Vec<String>,
}
