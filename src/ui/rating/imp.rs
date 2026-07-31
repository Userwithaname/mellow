use adw::{prelude::*, subclass::prelude::*};
use core::cell::{Cell, OnceCell, RefCell};
use gtk::CompositeTemplate;
use gtk::glib;

use crate::excuses::EXP_INIT;
use crate::library::{RatableAndTaggable, song_rating::SongRating, tag_list};

const NUM_STARS: u8 = 5;
const DEFAULT_STAR_SIZE: i32 = 16;
const SMALL_STAR_SIZE: i32 = 14;
const SMALL_STAR_MARGIN: i32 = (DEFAULT_STAR_SIZE - SMALL_STAR_SIZE) / 2;

#[derive(Default, CompositeTemplate)]
#[template(file = "rating.ui")]
pub struct Rating {
    #[template_child]
    stars: TemplateChild<gtk::Box>,
    #[template_child]
    favorite_button: TemplateChild<gtk::Button>,
    #[template_child]
    tags_list: TemplateChild<adw::WrapBox>,
    #[template_child]
    add_tag_entry: TemplateChild<gtk::SearchEntry>,
    #[template_child]
    add_tags_toggle: TemplateChild<gtk::ToggleButton>,
    #[template_child]
    available_tags: TemplateChild<gtk::Box>,

    star_icons: OnceCell<Box<[gtk::Image]>>,

    pub(super) rating: Cell<SongRating>,
    pub item: RefCell<Option<Box<dyn RatableAndTaggable>>>,
}

// TODO: Allow keyboard navigation for changing star ratings
// - Allow the stars widget to be focused using the tab key
// - Capture the left/right arrow keys to increase or decrease the stars

#[gtk::template_callbacks]
impl Rating {
    #[template_callback]
    pub fn handle_toggle_favorite(&self) {
        let mut rating = self.rating.get();
        rating.toggle_favorite();
        self.set_rating(rating);
    }

    #[template_callback]
    pub fn handle_confirm_tag(&self) {
        if !self.add_tag_entry.text().is_empty() {
            self.available_tags.first_child().unwrap().activate();
        }
    }
    #[template_callback]
    pub fn hide_tags_menu(&self) {
        self.add_tags_toggle.set_active(false);
    }

    /// Initializes the widget controllers
    #[inline]
    fn init_stars(&self) {
        let mut star_icons = Vec::new();
        for _ in 0..NUM_STARS {
            let star_icon = gtk::Image::builder()
                .icon_name("starred-symbolic")
                .css_classes(["dimmed"])
                .build();
            self.stars.append(&star_icon);
            star_icons.push(star_icon);
        }
        let _ = self.star_icons.set(star_icons.into());

        let motion = gtk::EventControllerMotion::builder()
            .propagation_phase(gtk::PropagationPhase::Capture)
            .build();
        motion.connect_motion(glib::clone!(
            #[weak(rename_to=rating)]
            self,
            move |_, pos_x, _| match rating.pixels_to_stars(pos_x) {
                Ok(new_rating) => rating.preview_stars(rating.rating.get().stars(), new_rating),
                Err(_) => rating.show_stars(rating.rating.get().stars()),
            }
        ));
        motion.connect_leave(glib::clone!(
            #[weak(rename_to=rating)]
            self,
            move |_| rating.show_stars(rating.rating.get().stars())
        ));
        self.stars.add_controller(motion);

        let click = gtk::GestureClick::builder()
            .propagation_phase(gtk::PropagationPhase::Capture)
            .build();
        click.connect_released(glib::clone!(
            #[weak(rename_to=rating)]
            self,
            move |_, _, pos_x, pos_y| if let Ok(new_rating) = rating.pixels_to_stars(pos_x) {
                if pos_y < 0.0 || pos_y > rating.stars.height() as f64 {
                    return;
                }
                rating.set_stars(match new_rating == rating.rating.get().stars() {
                    false => new_rating,
                    true => 0,
                });
            }
        ));
        self.stars.add_controller(click);
    }

    /// Sets the rating to the given value
    #[inline]
    pub fn set_rating(&self, rating: SongRating) {
        self.rating.set(rating);
        self.show_stars(rating.stars());
        self.update_favorite_button(rating.is_favorite());
        if let Some(item) = &*self.item.borrow() {
            item.set_rating(rating);
        }
    }

    /// Updates the appearance of the favorite button
    pub fn update_favorite_button(&self, is_favorite: bool) {
        match is_favorite {
            true => {
                self.favorite_button.remove_css_class("dimmed");
                self.favorite_button
                    .set_tooltip_text(Some("Remove From Favorites"));
            }
            false => {
                self.favorite_button.add_css_class("dimmed");
                self.favorite_button
                    .set_tooltip_text(Some("Add To Favorites"));
            }
        }
    }

    /// Sets the stars rating to the given value
    #[inline]
    pub fn set_stars(&self, stars: u8) {
        let mut rating = self.rating.get();
        rating.set_stars(stars);
        self.set_rating(rating);
    }

    /// Highlights the stars to match the `rating`
    #[inline]
    pub fn show_stars(&self, stars: u8) {
        let star_icons = self.star_icons.get().expect(EXP_INIT);
        for i in 0..stars {
            let star = &star_icons[i as usize];
            star.remove_css_class("dimmed");
            star.set_pixel_size(DEFAULT_STAR_SIZE);
            star.set_margin_start(0);
            star.set_margin_end(0);
        }
        for i in stars..NUM_STARS {
            let star = &star_icons[i as usize];
            star.add_css_class("dimmed");
            star.set_pixel_size(DEFAULT_STAR_SIZE);
            star.set_margin_start(0);
            star.set_margin_end(0);
        }
    }

    /// Highlights the stars to match the `preview` rating,
    /// and shows inactive stars as either smaller or regular
    /// sized, to show the previous `rating`
    #[inline]
    pub fn preview_stars(&self, stars: u8, preview: u8) {
        let star_icons = self.star_icons.get().expect(EXP_INIT);
        let stars = stars.max(preview);
        for i in 0..preview {
            let star = &star_icons[i as usize];
            star.remove_css_class("dimmed");
            star.set_pixel_size(DEFAULT_STAR_SIZE);
            star.set_margin_start(0);
            star.set_margin_end(0);
        }
        for i in preview..stars {
            let star = &star_icons[i as usize];
            star.add_css_class("dimmed");
            star.set_pixel_size(DEFAULT_STAR_SIZE);
            star.set_margin_start(0);
            star.set_margin_end(0);
        }
        for i in stars..NUM_STARS {
            let star = &star_icons[i as usize];
            star.add_css_class("dimmed");
            star.set_pixel_size(SMALL_STAR_SIZE);
            star.set_margin_start(SMALL_STAR_MARGIN);
            star.set_margin_end(SMALL_STAR_MARGIN);
        }
    }

    /// Takes the given `pos_x` pixel position and returns
    /// the number of stars at that position
    ///
    /// # Errors
    /// Returns an `Err` with the closest valid star count
    /// if `pos_x` is outside the widget boundaries
    pub fn pixels_to_stars(&self, pos_x: f64) -> Result<u8, u8> {
        if pos_x < 0.0 {
            return Err(0);
        }
        let spacing = self.stars.spacing() as f64;
        let star_width = DEFAULT_STAR_SIZE as f64 + spacing;
        match ((pos_x + spacing / 2.0) / star_width) as u8 + 1 {
            stars if stars > 5 => Err(5),
            stars => Ok(stars),
        }
    }

    #[inline]
    pub fn set_item(&self, item: Box<dyn RatableAndTaggable>) {
        self.item.replace(Some(item));

        if self.stars.is_mapped() {
            self.refresh_rating();
        }
    }

    fn refresh_rating(&self) {
        if let Some(item) = &*self.item.borrow() {
            let rating = item.get_rating();
            self.rating.set(rating);
            self.show_stars(rating.stars());
            self.update_favorite_button(rating.is_favorite());
        }
    }

    fn add_tag(&self, tag: String) {
        if let Some(item) = &*self.item.borrow() {
            if !tag.is_empty() {
                item.add_tag(tag);
                self.refresh_tags();
            }
        } else {
            #[cfg(debug_assertions)]
            panic!("Could not add tag - `item` is not assigned to the rating widget");
        }
    }
    fn refresh_tags(&self) {
        if let Some(item) = &*self.item.borrow() {
            self.tags_list.remove_all();
            for tag in item.get_tags() {
                let tag_box = gtk::Box::builder().build();
                tag_box.append(&gtk::Label::new(Some(&tag)));
                let remove_tag_button = gtk::Button::builder()
                    .icon_name("window-close-symbolic")
                    .css_classes(["flat", "circular"])
                    .build();
                tag_box.append(&remove_tag_button);

                self.tags_list.append(&tag_box);

                remove_tag_button.connect_clicked(glib::clone!(
                    #[weak(rename_to = this)]
                    self,
                    move |_| {
                        this.tags_list.remove(&tag_box);
                        (this.item.borrow().as_ref()).inspect(|item| item.remove_tag(&tag));
                        this.update_tag_buttons(); // In case the last instance was removed
                    }
                ));
            }
        }
    }
    #[template_callback]
    pub fn update_tag_buttons(&self) {
        // NOTE: Using a `GridView` would be more efficient (but this works for now)

        while let Some(button) = self.available_tags.last_child() {
            self.available_tags.remove(&button);
        }

        let entry = &*self.add_tag_entry.text();
        let mut exact_match = false;
        for tag in tag_list::read_global_tags().tag_names() {
            if !tag.starts_with(entry) {
                continue;
            }

            let tag_button = gtk::Button::builder()
                .label(tag)
                .css_classes(["pill"])
                .build();
            tag_button.connect_clicked(glib::clone!(
                #[weak(rename_to = rating)]
                self,
                move |tag_button| {
                    rating.add_tag(tag_button.label().expect(EXP_INIT).to_string());
                    rating.add_tag_entry.set_text("");
                }
            ));
            self.available_tags.append(&tag_button);
            exact_match |= tag == entry;
        }

        if !exact_match {
            // An extra button for creating new tags
            let tag_button = gtk::Button::builder()
                .label(format!("Create new tag: {entry}"))
                .css_classes(["pill"])
                .build();
            let new_tag = entry.to_string();
            tag_button.connect_clicked(glib::clone!(
                #[weak(rename_to = rating)]
                self,
                move |_| {
                    rating.add_tag(new_tag.clone());
                    rating.add_tag_entry.set_text("");
                }
            ));
            self.available_tags.append(&tag_button);
        }

        if !entry.is_empty() {
            self.available_tags
                .first_child()
                .unwrap()
                .add_css_class("suggested-action");
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Rating {
    const NAME: &str = "MellowRating";
    type Type = super::Rating;
    type ParentType = gtk::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
        class.bind_template_callbacks();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}
impl ObjectImpl for Rating {
    fn constructed(&self) {
        self.init_stars();
        self.stars.set_cursor_from_name(Some("pointer"));

        self.stars.connect_map(glib::clone!(
            #[weak(rename_to = this)]
            self,
            move |_| this.refresh_rating()
        ));

        self.tags_list.connect_map(glib::clone!(
            #[weak(rename_to = this)]
            self,
            move |_| this.refresh_tags()
        ));

        self.available_tags.connect_map(glib::clone!(
            #[weak(rename_to = rating)]
            self,
            move |_| rating.update_tag_buttons()
        ));

        self.add_tag_entry.connect_map(|entry| {
            entry.grab_focus();
        });
    }
}
impl WidgetImpl for Rating {}
impl BoxImpl for Rating {}
