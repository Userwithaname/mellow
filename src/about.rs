use adw::prelude::AdwDialogExt;
use gtk::{License, glib::object::IsA};

const APP_ID: Option<&str> = option_env!("APP_ID");
const APP_NAME: Option<&str> = option_env!("APP_NAME");
const APP_VERSION: Option<&str> = option_env!("APP_VERSION");
const RELEASE_NOTES: Option<&str> = option_env!("RELEASE_NOTES");
const RESOURCES_FILE: Option<&str> = option_env!("RESOURCES_FILE");

const COPYRIGHT: &str = "© 2026 Iva Kotar";
const LICENSE_TYPE: License = License::Gpl30;
const DEVELOPERS: &[&str] = &["Iva Kotar"];
const DESIGNERS: &[&str] = &["Iva Kotar"];

/// Creates and opens a new 'About' window
pub fn show_about_dialog(parent: &impl IsA<gtk::Widget>) {
    let about = adw::AboutDialog::builder()
        .application_icon(app_id())
        .application_name(app_name())
        .version(app_version())
        .release_notes(app_version())
        .release_notes(release_notes())
        .issue_url("https://github.com/Userwithaname/mellow/issues/")
        .developers(DEVELOPERS)
        .designers(DESIGNERS)
        .copyright(COPYRIGHT)
        .license_type(LICENSE_TYPE)
        .build();
    about.present(Some(parent));
}

/// Returns the application ID, which is assigned from
/// the `APP_ID` environment variable during compilation
///
/// # Panics
/// Panics if the `APP_ID` environment variable
/// was not set before building
#[must_use]
pub const fn app_id() -> &'static str {
    APP_ID.expect("APP_ID env var not set at compile time")
}
/// Returns the application name, which is assigned from
/// the `APP_NAME` environment variable during compilation
///
/// # Panics
/// Panics if the `APP_NAME` environment variable
/// was not set before building
#[must_use]
pub const fn app_name() -> &'static str {
    APP_NAME.expect("APP_NAME env var not set at compile time")
}
/// Returns the application version, which is assigned from
/// the `APP_VERSION` environment variable during compilation
///
/// # Panics
/// Panics if the `APP_VERSION` environment variable
/// was not set before building
#[must_use]
pub const fn app_version() -> &'static str {
    APP_VERSION.expect("APP_VERSION env var not set at compile time")
}
/// Returns release notes for the current version, which are assigned
/// from the `RELEASE_NOTES` environment variable during compilation
///
/// # Panics
/// Panics if the `RELEASE_NOTES` environment variable was not set before building
#[must_use]
pub const fn release_notes() -> &'static str {
    RELEASE_NOTES.expect("RELEASE_NOTES env var not set at compile time")
}
/// Returns the resources file path, which is assigned from
/// the `RESOURCES_FILE` environment variable during compilation
///
/// # Panics
/// Panics if the `RESOURCES_FILE` environment variable
/// was not set before building
#[must_use]
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
