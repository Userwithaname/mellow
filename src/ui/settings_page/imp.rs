use adw::{prelude::*, subclass::prelude::*};
use core::cell::{Cell, OnceCell, RefCell};
use gtk::gdk_pixbuf::Pixbuf;
use gtk::{CompositeTemplate, InterfaceColorScheme};
use gtk::{gdk, glib};

use crate::excuses::{EXP_INIT, EXP_RX};
use crate::library::{LibraryRequest, library_tx};
use crate::player::{PlayerRequest, player_tx};
use crate::ui::StartupQueueChoice;
use crate::util::{approx_eq, lerp};

#[derive(Default, CompositeTemplate)]
#[template(file = "settings_page.ui")]
pub struct SettingsPage {
    // Playback Settings
    #[template_child]
    pub volume: TemplateChild<gtk::Scale>,
    #[template_child]
    pub gapless: TemplateChild<adw::SwitchRow>,
    #[template_child]
    pub play_in_background: TemplateChild<adw::SwitchRow>,

    // Appearance settings
    #[template_child]
    pub adaptive_colors: TemplateChild<adw::SwitchRow>,
    #[template_child]
    pub color_scheme: TemplateChild<adw::ComboRow>,

    // Directory Settings
    #[template_child]
    pub directory_list: TemplateChild<gtk::ListBox>,
    #[template_child]
    pub refresh_library_button: TemplateChild<gtk::Button>,

    // Startup Settings
    pub startup_choice: RefCell<StartupQueueChoice>,
    #[template_child]
    pub remember_queue_row: TemplateChild<adw::ExpanderRow>,
    #[template_child]
    pub remember_queue: TemplateChild<gtk::CheckButton>,
    #[template_child]
    pub remember_time: TemplateChild<adw::SwitchRow>,
    #[template_child]
    pub new_queue_row: TemplateChild<adw::ExpanderRow>,
    #[template_child]
    pub new_queue: TemplateChild<gtk::CheckButton>,
    // TODO: Remember shuffle preference and queue source even when disabled?
    #[template_child]
    pub shuffle_queue: TemplateChild<adw::SwitchRow>,
    #[template_child]
    pub queue_source: TemplateChild<adw::ComboRow>,
    #[template_child]
    pub empty_queue: TemplateChild<gtk::CheckButton>,

    pub directories: RefCell<Vec<String>>,

    pub css: OnceCell<gtk::CssProvider>,
    pub style_manager: OnceCell<adw::StyleManager>,
    current_color: Cell<Option<(f64, f64, f64)>>,

    pub style_main: RefCell<Vec<gtk::Widget>>,
    pub style_menu: RefCell<Vec<gtk::Widget>>,
    pub style_soft: RefCell<Vec<gtk::Widget>>,
    pub has_style: Cell<bool>,
}

#[gtk::template_callbacks]
impl SettingsPage {
    #[template_callback]
    pub fn handle_set_volume(&self, _: gtk::ScrollType, value: f64) -> glib::Propagation {
        if approx_eq(value, self.volume.value()) {
            return glib::Propagation::Stop;
        }
        (player_tx().send(PlayerRequest::SetVolume(value * value))).expect(EXP_RX);
        glib::Propagation::Proceed
    }
    #[template_callback]
    pub fn handle_gapless_switch(&self) {
        (player_tx().send(PlayerRequest::SetGapless(self.gapless.is_active()))).expect(EXP_RX);
    }

    #[template_callback]
    pub fn handle_adaptive_colors_switch(&self) {
        self.update_color_state();
        if let Some((r, g, b)) = self.current_color.get() {
            self.set_background_color(r, g, b);
        }
    }

    #[template_callback]
    pub fn handle_refresh_library(&self) {
        self.refresh_library_button.set_sensitive(false);
        library_tx().send(LibraryRequest::Rebuild).expect(EXP_RX);
    }

    #[inline]
    fn update_color_state(&self) {
        match self.adaptive_colors.is_active() {
            true => self.enable_background_color(),
            false => self.disable_background_color(),
        }
    }

    #[template_callback]
    pub fn handle_theme_dropdown(&self) {
        self.set_theme(match self.color_scheme.selected() {
            0 => adw::ColorScheme::ForceDark,
            1 => adw::ColorScheme::ForceLight,
            2 => adw::ColorScheme::Default,
            _ => unimplemented!(),
        });
    }

    #[template_callback]
    pub fn handle_select_remember_queue(&self) {
        let expanded = self.remember_queue_row.is_expanded();
        if !expanded && !self.remember_queue.is_active() {
            return;
        }
        self.remember_queue.set_active(true);
        self.startup_choice
            .replace(StartupQueueChoice::RestoreQueue);
        self.remember_queue_row.set_expanded(expanded);
        self.new_queue_row.set_expanded(false);
    }
    #[template_callback]
    pub fn handle_select_new_queue(&self) {
        let expanded = self.new_queue_row.is_expanded();
        if !expanded && !self.new_queue.is_active() {
            return;
        }
        self.new_queue.set_active(true);
        self.remember_queue_row.set_expanded(false);
        self.new_queue_row.set_expanded(expanded);
    }
    #[template_callback]
    pub fn handle_select_empty_queue(&self) {
        if self.empty_queue.is_active() {
            return;
        }
        self.empty_queue.set_active(true);
        self.startup_choice.replace(StartupQueueChoice::EmptyQueue);
        self.remember_queue_row.set_expanded(false);
        self.new_queue_row.set_expanded(false);
    }
    #[template_callback]
    pub fn handle_collapse_queue_rows(&self) {
        self.remember_queue_row
            .set_expanded(self.remember_queue.is_active());
        self.new_queue_row.set_expanded(self.new_queue.is_active());
    }
    #[template_callback]
    pub fn handle_update_new_queue_choice(&self) {
        self.startup_choice.replace(
            (1 + self.queue_source.selected() as i32 + (self.shuffle_queue.is_active() as i32 * 3))
                .into(),
        );
    }

    #[inline]
    pub fn set_theme(&self, preference: adw::ColorScheme) {
        (self.css.get().expect(EXP_INIT)).set_prefers_color_scheme(match preference {
            adw::ColorScheme::ForceDark | adw::ColorScheme::PreferDark => {
                InterfaceColorScheme::Dark
            }
            adw::ColorScheme::ForceLight | adw::ColorScheme::PreferLight => {
                InterfaceColorScheme::Light
            }
            _ => InterfaceColorScheme::Default,
        });
        (self.style_manager.get().expect(EXP_INIT)).set_color_scheme(preference);
        let color = self.current_color.get();
        match color {
            Some((r, g, b)) => self.set_background_color(r, g, b),
            None => self.disable_background_color(),
        }
    }

    #[inline]
    pub fn enable_background_color(&self) {
        if self.current_color.get().is_none() || self.has_style.get() {
            return;
        }

        for widget in self.style_main.borrow().iter() {
            widget.add_css_class("color-main");
        }
        for widget in self.style_menu.borrow().iter() {
            widget.add_css_class("color-menu");
        }
        for widget in self.style_soft.borrow().iter() {
            widget.add_css_class("color-osd");
        }

        self.has_style.set(true);
    }
    pub fn disable_background_color(&self) {
        if !self.has_style.get() {
            return;
        }

        for widget in self.style_main.borrow().iter() {
            widget.remove_css_class("color-main");
        }
        for widget in self.style_menu.borrow().iter() {
            widget.remove_css_class("color-menu");
        }
        for widget in self.style_soft.borrow().iter() {
            widget.remove_css_class("color-osd");
        }

        self.has_style.set(false);
    }
    pub fn reset_background_color(&self) {
        if self.css.get().unwrap().prefers_color_scheme() == InterfaceColorScheme::Default {
            self.set_theme(adw::ColorScheme::Default);
        }
        self.current_color.set(None);
        self.disable_background_color();
    }

    /// Returns a linearized color channel
    ///
    /// The input value must be scaled to a 0 to 1 range
    /// (e.g. color as f64 / 255.0)
    #[inline]
    fn srgb_to_linear(c: f64) -> f64 {
        c.powf(2.2)
    }

    /// Sets the interface colors based on the input `r` `g` `b`
    /// values. The colors are processed differently based on the
    /// selected color scheme.
    ///
    /// The input values are expected to be linear rather than sRGB
    #[inline]
    pub fn set_background_color(&self, r: f64, g: f64, b: f64) {
        #[inline]
        fn process_highlight_color(mut r: f64, mut g: f64, mut b: f64) -> (u8, u8, u8) {
            /// Colors below this luminance value will be desaturated for accuracy
            const DESATURATION_THRESHOLD: f64 = 0.2;

            let luminance = lum(r, g, b);
            let target_lum = (luminance * luminance * luminance).mul_add(0.6, 0.4);

            if luminance < DESATURATION_THRESHOLD {
                if luminance == 0.0 {
                    return (255, 255, 255);
                }

                // Desaturate dark colors for more accurate results once normalized
                let saturation =
                    (1.0 - (1.0 - luminance / DESATURATION_THRESHOLD).powi(2)).mul_add(0.7, 0.3);
                r = lerp(luminance, r, saturation);
                g = lerp(luminance, g, saturation);
                b = lerp(luminance, b, saturation);
            }

            // Normalize the color and scale it to the target luminance
            // Accuracy is not as important here, this approximation should suffice
            approx_linear_to_srgb(
                r / luminance * target_lum,
                g / luminance * target_lum,
                b / luminance * target_lum,
            )
        }
        #[inline]
        fn process_color_dark(mut r: f64, mut g: f64, mut b: f64) -> (u8, u8, u8) {
            const SATURATION: f64 = 1.6;

            r = (1.0 - (1.0 - r).powi(3)) / 7.5;
            g = (1.0 - (1.0 - g).powi(3)) / 7.5;
            b = (1.0 - (1.0 - b).powi(3)) / 7.5;

            let luminance = lum(r, g, b);
            r = lerp(luminance, r, SATURATION);
            g = lerp(luminance, g, SATURATION);
            b = lerp(luminance, b, SATURATION);

            linear_to_srgb(r, g, b)
        }
        #[inline]
        fn process_color_light(mut r: f64, mut g: f64, mut b: f64) -> (u8, u8, u8) {
            /// Colors below this luminance value will be desaturated for accuracy
            const DESATURATION_THRESHOLD: f64 = 0.33;

            let luminance = lum(r, g, b);
            let target_lum = (luminance * luminance * luminance).mul_add(0.5, 0.5);

            if luminance < DESATURATION_THRESHOLD {
                if luminance == 0.0 {
                    return (255, 255, 255);
                }

                // Desaturate dark colors for more accurate results once normalized
                let saturation = 1.0 - (1.0 - luminance / DESATURATION_THRESHOLD).powi(3);
                r = lerp(luminance, r, saturation);
                g = lerp(luminance, g, saturation);
                b = lerp(luminance, b, saturation);
            }

            // Normalize the color and scale it to the target luminance
            linear_to_srgb(
                r / luminance * target_lum,
                g / luminance * target_lum,
                b / luminance * target_lum,
            )
        }
        #[inline]
        fn process_color_auto(mut r: f64, mut g: f64, mut b: f64) -> ((u8, u8, u8), f64) {
            const SATURATION: f64 = 1.07;

            r = lerp(r, r * r, 0.4);
            g = lerp(g, g * g, 0.4);
            b = lerp(b, b * b, 0.4);

            let luminance = lum(r, g, b);
            r = lerp(luminance, r, SATURATION);
            g = lerp(luminance, g, SATURATION);
            b = lerp(luminance, b, SATURATION);

            (linear_to_srgb(r, g, b), luminance.powf(1.0 / 2.2))
        }
        /// Color luminance function:
        /// <https://stackoverflow.com/questions/596216/formula-to-determine-perceived-brightness-of-rgb-color/596243#596243>
        #[inline]
        fn lum(r: f64, g: f64, b: f64) -> f64 {
            r.mul_add(0.2126, g.mul_add(0.7152, b * 0.0722))
            // r.mul_add(0.299, g.mul_add(0.587, b * 0.114))
        }
        /// Converts a linear color to sRGB (for use with the CSS)
        #[inline]
        fn linear_to_srgb(r: f64, g: f64, b: f64) -> (u8, u8, u8) {
            (
                (r.powf(1.0 / 2.2) * 255.0) as u8,
                (g.powf(1.0 / 2.2) * 255.0) as u8,
                (b.powf(1.0 / 2.2) * 255.0) as u8,
            )
        }
        /// Rough approximation of the `linear_to_srgb` function,
        /// which might be a bit faster to compute (hopefully).
        /// Note that pure white and pure black are not possible
        /// with this approximation.
        #[inline]
        fn approx_linear_to_srgb(r: f64, g: f64, b: f64) -> (u8, u8, u8) {
            (
                (((1.0 - r).mul_add(-(1.0 - r), 0.8) + 0.4) / 1.25 * 255.0) as u8,
                (((1.0 - g).mul_add(-(1.0 - g), 0.8) + 0.4) / 1.25 * 255.0) as u8,
                (((1.0 - b).mul_add(-(1.0 - b), 0.8) + 0.4) / 1.25 * 255.0) as u8,
            )
        }

        self.current_color.set(Some((r, g, b)));
        let css = self.css.get().expect(EXP_INIT);
        let (mut r_highlight, mut g_highlight, mut b_highlight) = match self.has_style.get() {
            true => process_highlight_color(r, g, b),
            false => (255, 255, 255),
        };
        let (r, g, b) = match css.prefers_color_scheme() {
            InterfaceColorScheme::Dark => process_color_dark(r, g, b),
            InterfaceColorScheme::Light => {
                r_highlight = r_highlight.saturating_add(50);
                g_highlight = g_highlight.saturating_add(50);
                b_highlight = b_highlight.saturating_add(50);
                process_color_light(r, g, b)
            }
            _ => match process_color_auto(r, g, b) {
                (color, luminance) if luminance < 0.5 => {
                    (self.style_manager.get().unwrap())
                        .set_color_scheme(adw::ColorScheme::ForceDark);
                    color
                }
                (color, _) => {
                    r_highlight = r_highlight.saturating_add(50);
                    g_highlight = g_highlight.saturating_add(50);
                    b_highlight = b_highlight.saturating_add(50);
                    (self.style_manager.get().unwrap())
                        .set_color_scheme(adw::ColorScheme::ForceLight);
                    color
                }
            },
        };
        let r_dark = (r / 2).saturating_sub(4);
        let g_dark = (g / 2).saturating_sub(4);
        let b_dark = (b / 2).saturating_sub(4);
        let r_soft = r_dark.saturating_sub(4);
        let g_soft = g_dark.saturating_sub(4);
        let b_soft = b_dark.saturating_sub(4);

        // NOTE: The `color-osd` style could be used for player controls if transparency
        // amount would change with the luminance value, but it currently has contrast
        // issues when using a light theme

        css.load_from_string(&format!(
            ".color-main {{
                 background-color: rgba({r}, {g}, {b}, 1);
                 border-bottom: 0px none;
                 border-right: 0px none;
                 border-left: 0px none;
                 border-top: 0px none;
             }}
             .color-menu {{
                 background-color: rgba({r_dark}, {g_dark}, {b_dark}, 1);
             }}
             .color-osd {{
                 background-color: rgba({r_soft}, {g_soft}, {b_soft}, 0.75);
             }}
             .highlight-top {{
                 border-color: rgba({r_highlight}, {g_highlight}, {b_highlight}, 1);
             }}
            ",
        ));

        self.update_color_state();
    }

    #[inline]
    pub fn set_background_from_artwork(&self, artwork: &gdk::Texture) {
        let mut tex_dl = gdk::TextureDownloader::new(artwork);
        tex_dl.set_format(gdk::MemoryFormat::R8g8b8a8Premultiplied);
        let (image_data, row_stride) = tex_dl.download_bytes();

        let color = Pixbuf::from_bytes(
            &image_data,
            gtk::gdk_pixbuf::Colorspace::Rgb,
            true,
            8,
            artwork.width(),
            artwork.height(),
            row_stride as i32,
        )
        .scale_simple(1, 1, gtk::gdk_pixbuf::InterpType::Bilinear)
        .unwrap()
        .read_pixel_bytes();

        self.set_background_color(
            Self::srgb_to_linear(color[0] as f64 / 255.0),
            Self::srgb_to_linear(color[1] as f64 / 255.0),
            Self::srgb_to_linear(color[2] as f64 / 255.0),
        );
    }

    #[inline]
    pub fn set_directories(&self, directories: &[String]) {
        self.directory_list.set_sensitive(true);
        self.directory_list.remove_all();
        for (i, directory) in directories.iter().enumerate() {
            let prefix_icon = gtk::Image::builder()
                .icon_name("folder-symbolic")
                .margin_top(8)
                .margin_bottom(8)
                .build();
            let directory_row = adw::ActionRow::builder()
                .title(directory)
                .selectable(true)
                .build();
            directory_row.add_prefix(&prefix_icon);
            let remove_button = gtk::Button::builder()
                .icon_name("window-close-symbolic")
                .margin_top(8)
                .margin_bottom(8)
                .has_tooltip(true)
                .tooltip_text("Remove") // TODO: Support translations
                .css_classes(["flat", "circular"])
                .build();
            remove_button.connect_clicked({
                let directory_row = directory_row.clone();
                let directory_list = self.directory_list.clone();
                move |_| {
                    // Prevent removing any other directories until the list updates,
                    // because the row index may no longer be consistent with the library
                    directory_list.set_sensitive(false);
                    directory_list.remove(&directory_row); // This may seem more responsive

                    (library_tx().send(LibraryRequest::RemoveLibrary(i))).expect(EXP_RX);
                }
            });
            directory_row.add_suffix(&remove_button);
            self.directory_list.append(&directory_row);
        }
        if directories.is_empty() {
            let add_directory_button = adw::ButtonRow::builder()
                .title("Add Directory")
                .start_icon_name("folder-new-symbolic")
                .action_name("win.add_library")
                .build();
            add_directory_button.add_css_class("suggested-action");
            self.directory_list.append(&add_directory_button);
        }
        self.directories.replace(directories.into());
    }
}

#[glib::object_subclass]
impl ObjectSubclass for SettingsPage {
    const NAME: &str = "MellowSettingsPage";
    type Type = super::SettingsPage;
    type ParentType = adw::PreferencesPage;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
        class.bind_template_callbacks();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for SettingsPage {}
impl WidgetImpl for SettingsPage {}
impl PreferencesPageImpl for SettingsPage {}
