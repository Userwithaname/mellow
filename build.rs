use std::fs;

fn main() {
    // Rerun if any files within the `data` directory have changed
    println!("cargo::rerun-if-changed=data");

    let app_id = option_env!("APP_ID").unwrap_or_else(|| {
        let app_id = "io.github.userwithaname.Mellow";
        println!("cargo:rustc-env=APP_ID={app_id}");
        app_id
    });

    let app_name = app_id.rsplit_once('.').expect("Invalid APP_ID").1;
    println!("cargo:rustc-env=APP_NAME={app_name}");

    let app_version = option_env!("APP_VERSION").unwrap_or_else(|| {
        let cargo_toml = fs::read_to_string(env!("CARGO_MANIFEST_DIR").to_owned() + "/Cargo.toml")
            .expect("Failed to read Cargo.toml");
        let cargo_toml = Box::leak(Box::new(cargo_toml));
        for line in cargo_toml.lines() {
            if line.starts_with("version") {
                let version = line.split_once("=").unwrap().1.trim();
                return &version[1..version.len() - 1];
            }
        }
        panic!("No version found in Cargo.toml");
    });
    println!("cargo:rustc-env=APP_VERSION={app_version}");

    let release_notes = release_notes_from_metainfo(app_id, app_version);
    println!("cargo:rustc-env=RELEASE_NOTES={release_notes}");

    #[cfg(feature = "gresources")]
    #[cfg(feature = "no-meson")]
    glib_build_tools::compile_resources(
        &["data/resources"],
        "data/resources/resources.gresource.xml",
        "mellow.gresource",
    );
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
