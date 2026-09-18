use adw::prelude::AdwDialogExt;
use gtk::{License, glib::object::IsA};

pub const APP_URL: &str = "https://github.com/Userwithaname/mellow";
pub const APP_ID: &str = env!("APP_ID");
pub const APP_NAME: &str = env!("APP_NAME");
pub const APP_VERSION: &str = env!("APP_VERSION");
pub const RELEASE_NOTES: &str = env!("RELEASE_NOTES");

#[cfg(feature = "gresources")]
#[cfg(not(feature = "no-meson"))]
const RESOURCES_FILE: Option<&str> = option_env!("RESOURCES_FILE");

const COPYRIGHT: &str = "© 2026 Iva Kotar";
const LICENSE_TYPE: License = License::Gpl30;
const DEVELOPERS: &[&str] = &["Iva Kotar"];
const DESIGNERS: &[&str] = &["Iva Kotar"];

/// Creates and opens a new 'About' window
pub fn show_about_dialog(parent: &impl IsA<gtk::Widget>) {
    let about = adw::AboutDialog::builder()
        .application_icon(APP_ID)
        .application_name(APP_NAME)
        .copyright(COPYRIGHT)
        .designers(DESIGNERS)
        .developers(DEVELOPERS)
        .license_type(LICENSE_TYPE)
        .release_notes(RELEASE_NOTES)
        .version(APP_VERSION)
        .website(APP_URL)
        .issue_url([APP_URL, "/issues/"].concat())
        .build();
    about.present(Some(parent));
}

/// Returns the resources file path, which is assigned from
/// the `RESOURCES_FILE` environment variable during compilation
///
/// # Panics
/// Panics if the `RESOURCES_FILE` environment variable
/// was not set before building
#[inline]
#[must_use]
#[cfg(feature = "gresources")]
#[cfg(not(feature = "no-meson"))]
pub const fn resources_file() -> &'static str {
    RESOURCES_FILE.expect("RESOURCES_FILE env var not set at compile time")
}

#[cfg(test)]
mod tests {
    use core::error::Error;
    use gtk::License;
    use std::fs;

    use crate::about::LICENSE_TYPE;

    #[test]
    fn metadata_consistency() -> Result<(), Box<dyn Error>> {
        let project_dir = env!("CARGO_MANIFEST_DIR");

        let (mut name_meson, mut name_cargo) = ("(none)", "(none)");
        let (mut version_meson, mut version_cargo) = ("(none)", "(none)");
        let (mut app_id_meson, mut app_id_build_rs) = ("(none)", "(none)");
        let mut license = "(none)";

        let meson_build = fs::read_to_string([project_dir, "/meson.build"].concat())
            .inspect_err(|_| eprintln!("Could not read {project_dir}/meson.build"))?;
        for line in meson_build.lines() {
            if name_meson == "(none)"
                && let Some((_, name)) = line.split_once("project('")
            {
                name_meson = name.split_once('\'').unwrap().0;
            }
            if version_meson == "(none)"
                && let Some((_, version)) = line.split_once("version: '")
            {
                version_meson = version.split_once('\'').unwrap().0;
            }
            if line.starts_with("base_id") {
                app_id_meson = line.split_once("=").unwrap().1.trim();
                app_id_meson = &app_id_meson[1..app_id_meson.len() - 1];
                break; // This assumes `base_id` is below `project()`
            }
        }

        let cargo_toml = fs::read_to_string([project_dir, "/Cargo.toml"].concat())
            .inspect_err(|_| eprintln!("Could not read {project_dir}/Cargo.toml"))?;
        for line in cargo_toml.lines() {
            if line.starts_with("name") {
                name_cargo = line.split_once("=").unwrap().1.trim();
                name_cargo = &name_cargo[1..name_cargo.len() - 1];
            } else if line.starts_with("version") {
                version_cargo = line.split_once("=").unwrap().1.trim();
                version_cargo = &version_cargo[1..version_cargo.len() - 1];
            } else if line.starts_with("license") {
                license = line.split_once("=").unwrap().1.trim();
                license = &license[1..license.len() - 1];
            }
        }

        let build_rs = fs::read_to_string([project_dir, "/build.rs"].concat())
            .inspect_err(|_| eprintln!("Could not read {project_dir}/build.rs"))?;
        for line in build_rs.lines() {
            if line.contains("let app_id = \"") {
                app_id_build_rs = line.split_once("=").unwrap().1.trim();
                app_id_build_rs = &app_id_build_rs[1..app_id_build_rs.len() - 2];
            }
        }

        // Test if project info in Meson and Cargo matches
        assert!(
            name_meson == name_cargo,
            "Meson: {name_meson}\nCargo: {name_cargo}",
        );
        assert!(
            version_meson.to_lowercase() == version_cargo.to_lowercase(),
            "Meson: {version_meson}\nCargo: {version_cargo}",
        );
        assert!(
            app_id_meson == app_id_build_rs,
            "Meson: {app_id_meson}\nCargo: {app_id_build_rs}",
        );
        assert!(
            app_id_meson.to_lowercase().contains(name_meson),
            "APP_ID must contain the application name"
        );

        let app_id_path = format!("\"/{}/\"", app_id_meson.replace('.', "/"));

        // Test if resources are using the correct ID
        let gresources =
            fs::read_to_string([project_dir, "/data/resources/resources.gresource.xml"].concat())?;
        assert!(
            gresources.contains(&app_id_path),
            "Incorrect prefix in `resources.gresource.xml`\nExpected: {app_id_path}"
        );
        let gschema =
            fs::read_to_string(format!("{project_dir}/data/{app_id_meson}.gschema.xml.in"))?;
        assert!(
            gschema.contains(app_id_meson),
            "Incorrect ID in `{app_id_meson}.gschema.xml.in`\nExpected: {app_id_meson}"
        );
        assert!(
            gschema.contains(&app_id_path),
            "Incorrect path in `{app_id_meson}.gschema.xml.in`\nExpected: {app_id_path}"
        );
        let metainfo =
            fs::read_to_string(format!("{project_dir}/data/{app_id_meson}.metainfo.xml.in"))?;
        assert!(
            metainfo.contains(&format!("<release version=\"{version_meson}\"")),
            "MetaInfo does not contain release information for version {version_meson}"
        );

        // Test if licenses match
        let license_file = fs::read_to_string([project_dir, "/LICENSE"].concat())?;
        match LICENSE_TYPE {
            License::Gpl30 => {
                assert!(
                    license == "GPL-3.0",
                    "LICENSE_TYPE: GPL-3.0\nCargo: {license}"
                );
                assert!(
                    (license_file.lines().next())
                        .expect("LICENSE file is empty")
                        .contains("GNU GENERAL PUBLIC LICENSE"),
                    "LICENSE file does not contain the correct license"
                );
            }
            value => {
                panic!("License test must be updated\nLICENSE_TYPE: {value:?}\nCargo: {license}")
            }
        }

        Ok(())
    }
}
