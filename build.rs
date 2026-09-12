use std::fs;

#[cfg(feature = "no-meson")]
fn main() {
    const APP_ID: &str = "io.github.userwithaname.Mellow";
    println!("cargo:rustc-env=APP_ID={APP_ID}");

    let app_name = APP_ID.rsplit_once('.').expect("Invalid APP_ID").1;
    println!("cargo:rustc-env=APP_NAME={app_name}");

    let cargo_toml = fs::read_to_string(env!("CARGO_MANIFEST_DIR").to_owned() + "/Cargo.toml")
        .expect("Failed to read Cargo.toml");
    for line in cargo_toml.lines() {
        if line.starts_with("version") {
            let mut version = line.split_once("=").unwrap().1.trim();
            version = &version[1..version.len() - 1];
            println!("cargo:rustc-env=APP_VERSION={version}");

            let release_notes = release_notes_from_metainfo(APP_ID, version);
            println!("cargo:rustc-env=RELEASE_NOTES={release_notes}");
        }
    }

    glib_build_tools::compile_resources(
        &["data/resources"],
        "data/resources/resources.gresource.xml",
        "mellow.gresource",
    );
}

#[cfg(not(feature = "no-meson"))]
fn main() {
    if let Some(app_id) = option_env!("APP_ID") {
        let app_name = app_id.rsplit_once('.').expect("Invalid APP_ID").1;
        println!("cargo:rustc-env=APP_NAME={app_name}");

        let app_version =
            option_env!("APP_VERSION").expect("APP_VERSION env var should be set in Meson");
        let release_notes = release_notes_from_metainfo(app_id, app_version);
        println!("cargo:rustc-env=RELEASE_NOTES={release_notes}");
    }
    // Everything else is done in Meson
}

fn release_notes_from_metainfo(app_id: &str, app_version: &str) -> String {
    let metainfo = fs::read_to_string(
        env!("CARGO_MANIFEST_DIR").to_owned() + "/data/" + app_id + ".metainfo.xml.in",
    )
    .expect("Failed to read MetaInfo file");

    let mut metainfo_lines = metainfo.lines();
    let release_entry = &format!("<release version=\"{app_version}\"");
    while let Some(line) = metainfo_lines.next()
        && !line.contains(release_entry)
    {
        // Skip until the `app_version` release notes
    }
    metainfo_lines.next();

    let mut out = String::new();
    while let Some(line) = metainfo_lines.next()
        && !line.contains("</description>")
    {
        out.push_str(line.trim());
    }
    out
}
