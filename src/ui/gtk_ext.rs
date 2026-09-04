use gtk::gdk;
use gtk::glib::object::IsA;

thread_local! {
    pub(crate) static BLANK_TEXTURE: gdk::Paintable = gdk::Paintable::new_empty(1, 1);
}

pub(crate) trait GtkPictureExt {
    fn set_blank(&self);
    fn set_paintable_or_blank(&self, paintable: Option<&impl IsA<gdk::Paintable>>);
}
impl GtkPictureExt for gtk::Picture {
    #[inline]
    fn set_blank(&self) {
        BLANK_TEXTURE.with(|blank| self.set_paintable(Some(&*blank)))
    }
    #[inline]
    fn set_paintable_or_blank(&self, paintable: Option<&impl IsA<gdk::Paintable>>) {
        match paintable {
            None => BLANK_TEXTURE.with(|blank| self.set_paintable(Some(&*blank))),
            paintable => self.set_paintable(paintable),
        }
    }
}
