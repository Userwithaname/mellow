use adw::{prelude::*, subclass::prelude::*};
use core::cell::{Cell, OnceCell, RefCell};
use gtk::gdk_pixbuf::Pixbuf;
use gtk::{CompositeTemplate, InterfaceColorScheme};
use gtk::{gdk, glib};

use crate::excuses::{ACTION_ERR, EXP_INIT};
use crate::library::{Library, LibraryRequest};
use crate::player::PlayerRequest;
use crate::ui::StartupQueueChoice;
use crate::util::color::{ColorLinear, ColorSRGB};
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
    #[template_child]
    add_directory_button: TemplateChild<gtk::Button>,

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
    current_color: Cell<Option<ColorLinear>>,

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
        PlayerRequest::SetVolume(value * value).send();
        glib::Propagation::Proceed
    }
    #[template_callback]
    pub fn handle_gapless_switch(&self) {
        PlayerRequest::SetGapless(self.gapless.is_active()).send();
    }

    #[template_callback]
    pub fn handle_adaptive_colors_switch(&self) {
        self.update_color_state();
        if let Some(color) = self.current_color.get() {
            self.set_background_color(color);
        }
    }

    #[template_callback]
    pub fn handle_refresh_library(&self) {
        self.refresh_library_button.set_sensitive(false);
        Library::rebuild();
    }
    #[template_callback]
    pub fn handle_add_directory(&self) {
        self.allow_library_changes(false); // Will be re-activated later
        self.add_directory_button
            .activate_action("app.add_library", None)
            .expect(ACTION_ERR);
    }

    #[inline]
    pub fn allow_library_changes(&self, allow: bool) {
        self.add_directory_button.set_sensitive(allow);
        self.directory_list.set_sensitive(allow);
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
            Some(color) => self.set_background_color(color),
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

    /// Adjusts the interface colors, using `color` as a base, which
    /// is processed according to the selected color scheme
    #[inline]
    pub fn set_background_color(&self, color: ColorLinear) {
        #[inline(always)]
        fn process_highlight_color(mut color: ColorLinear) -> ColorSRGB {
            /// Colors below this luminance value will be desaturated for accuracy
            const DESATURATION_THRESHOLD: f64 = 0.2;

            let luminance = color.lum();
            if luminance < DESATURATION_THRESHOLD {
                if luminance == 0.0 {
                    return ColorSRGB::white();
                }

                // Desaturate dark colors for more accurate results once normalized
                let saturation =
                    (1.0 - (1.0 - luminance / DESATURATION_THRESHOLD).powi(2)).mul_add(0.7, 0.3);
                color.per_channel(|c| *c = lerp(luminance, *c, saturation));
            }

            // Normalize the color and scale it to the target luminance
            let target_lum = luminance.powi(3).mul_add(0.4, 0.6);
            (color / luminance * target_lum).to_srgb_approx() // Accuracy is not as important here
        }
        fn process_color_dark(mut color: ColorLinear) -> ColorSRGB {
            const SATURATION: f64 = 1.6;

            color = (1.0 - (1.0 - color).powi(3)) / 7.5;

            let luminance = color.lum();
            color.per_channel(|c| *c = lerp(luminance, *c, SATURATION));

            color.to_srgb()
        }
        fn process_color_light(mut color: ColorLinear) -> ColorSRGB {
            /// Colors below this luminance value will be desaturated for accuracy
            const DESATURATION_THRESHOLD: f64 = 0.33;

            let luminance = color.lum();
            if luminance < DESATURATION_THRESHOLD {
                if luminance == 0.0 {
                    return ColorSRGB::white();
                }

                // Desaturate dark colors for more accurate results once normalized
                let saturation = 1.0 - (1.0 - luminance / DESATURATION_THRESHOLD).powi(3);
                color.per_channel(|c| *c = lerp(luminance, *c, saturation));
            }

            // Normalize the color and scale it to the target luminance
            let target_lum = luminance.powi(3).mul_add(0.5, 0.5);
            (color / luminance * target_lum).to_srgb()
        }
        fn process_color_auto(mut color: ColorLinear) -> (ColorSRGB, f64) {
            const SATURATION: f64 = 1.07;

            color.per_channel(|c| *c = lerp(*c, c.powi(2), 0.4));

            let luminance = color.lum();
            color.per_channel(|c| *c = lerp(luminance, *c, SATURATION));

            (color.to_srgb(), luminance.powf(1.0 / 2.2))
        }

        self.current_color.set(Some(color));
        let css = self.css.get().expect(EXP_INIT);
        let mut highlight_color = match self.has_style.get() {
            true => process_highlight_color(color),
            false => ColorSRGB::white(),
        };
        let final_color = match css.prefers_color_scheme() {
            InterfaceColorScheme::Dark => process_color_dark(color),
            InterfaceColorScheme::Light => {
                highlight_color.per_channel(|c| *c = c.saturating_add(50));
                process_color_light(color)
            }
            _ => match process_color_auto(color) {
                (color, luminance) if luminance < 0.5 => {
                    (self.style_manager.get().unwrap())
                        .set_color_scheme(adw::ColorScheme::ForceDark);
                    color
                }
                (color, _) => {
                    highlight_color.per_channel(|c| *c = c.saturating_add(50));
                    (self.style_manager.get().unwrap())
                        .set_color_scheme(adw::ColorScheme::ForceLight);
                    color
                }
            },
        };

        let (r, g, b) = final_color.as_tuple();
        let (r_highlight, g_highlight, b_highlight) = highlight_color.as_tuple();
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

        self.set_background_color(ColorSRGB::from([color[0], color[1], color[2]]).to_linear());
    }

    #[inline]
    pub fn set_directories(&self, directories: &[String]) {
        self.allow_library_changes(true);
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
                let settings_page = self.to_owned();
                let directory_row = directory_row.clone();
                let directory_list = self.directory_list.clone();
                move |_| {
                    // Prevent removing any other directories until the list updates,
                    // because the row index may no longer be consistent with the library
                    settings_page.allow_library_changes(false);
                    directory_list.remove(&directory_row); // This may seem more responsive

                    LibraryRequest::RemoveLibrary(i).send();
                }
            });
            directory_row.add_suffix(&remove_button);
            self.directory_list.append(&directory_row);
        }
        if directories.is_empty() {
            let add_directory_button = adw::ButtonRow::builder()
                .title("Add Directory")
                .start_icon_name("folder-new-symbolic")
                .build();
            add_directory_button.connect_activated({
                let settings_page = self.to_owned();
                move |_| settings_page.handle_add_directory()
            });
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
