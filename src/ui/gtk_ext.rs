use gtk::gdk;
use gtk::glib::object::IsA;

pub(super) trait GtkPictureExt {
    thread_local! {
        static BLANK_TEXTURE: gdk::Paintable = gdk::Paintable::new_empty(1, 1);
    }

    /// Sets the image to a blank placeholder image (grey square)
    fn set_blank(&self);
    /// Sets the image to `paintable` if `Some`, or a blank image otherwise
    fn set_paintable_or_blank(&self, paintable: Option<&impl IsA<gdk::Paintable>>);
}
impl GtkPictureExt for gtk::Picture {
    #[inline]
    fn set_blank(&self) {
        Self::BLANK_TEXTURE.with(|blank| self.set_paintable(Some(blank)));
    }
    #[inline]
    fn set_paintable_or_blank(&self, paintable: Option<&impl IsA<gdk::Paintable>>) {
        match paintable {
            None => Self::BLANK_TEXTURE.with(|blank| self.set_paintable(Some(blank))),
            paintable => self.set_paintable(paintable),
        }
    }
}
