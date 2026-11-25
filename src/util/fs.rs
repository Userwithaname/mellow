use core::hint::cold_path;
use glib::{UserDirectory, home_dir, user_cache_dir, user_config_dir, user_special_dir};
use gtk::glib;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::{fs, io};

static CACHE_DIR: LazyLock<String> =
    LazyLock::new(|| [user_cache_dir().to_str().unwrap(), "/mellow/"].concat());
static CONFIG_DIR: LazyLock<String> =
    LazyLock::new(|| [user_config_dir().to_str().unwrap(), "/mellow/"].concat());
static MUSIC_DIR: LazyLock<String> = LazyLock::new(|| {
    user_special_dir(UserDirectory::Music).map_or_else(
        || [home_dir().to_str().unwrap(), "/Music/"].concat(),
        |dir| dir.to_str().unwrap().to_owned(),
    )
});

/// Runs a closure for every file found within `dir` (recursive)
///
/// Adapted from the official Rust documentation:
/// <https://doc.rust-lang.org/std/fs/fn.read_dir.html#examples>
///
/// # Errors
/// - If `dir` is not a directory
/// - If a contained file or directory could not be read
#[inline]
pub fn visit_dirs<F: FnMut(PathBuf)>(dir: PathBuf, f: &mut F) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        match path.is_dir() {
            false => f(path),
            true => visit_dirs(path, f)?,
        }
    }
    Ok(())
}

/// Attempts to write the file to disk, and retries after creating the
/// directory path in case of an error
///
/// # Errors
/// If creating the directory or writing the file fails the second time,
/// the error is propagated
///
/// # Panics
/// Panics if `path` has no parent directory
#[inline]
pub fn write_file_create_dir_all<P: AsRef<Path>, C: AsRef<[u8]>>(
    path: P,
    contents: C,
) -> io::Result<()> {
    if fs::write(&path, &contents).is_err() {
        cold_path();
        fs::create_dir_all(path.as_ref().parent().unwrap())?;
        fs::write(path, contents)?;
    }
    Ok(())
}

/// Writes the `contents` to the file at `file_path` if the contents have
/// changed. If a file at `file_path` suffixed with a tilde (`~`) existts,
/// the old file is moved to that location as a backup.
///
/// Returns `Ok(true)` if the file was written, or `Ok(false)` if not
///
/// # Errors
/// Propagates write errors (see `write_file_create_dir_all`)
///
/// # Panics
/// May panic if `path` has no parent directory
pub fn write_file_with_opt_backup<P: AsRef<Path>, C: AsRef<[u8]>>(
    file_path: P,
    contents: C,
) -> io::Result<bool> {
    if let Ok(old_contents) = fs::read(&file_path) {
        if *contents.as_ref() == *old_contents {
            return Ok(false);
        }
        let backup_file = file_path.as_ref().join("~");
        if fs::exists(&backup_file).unwrap_or_default() {
            let _ = fs::rename(&file_path, backup_file);
        }
    }
    write_file_create_dir_all(&file_path, contents)?;
    Ok(true)
}

/// Returns the music directory path
#[inline]
#[must_use]
pub fn music_dir() -> &'static str {
    &MUSIC_DIR
}
/// Returns the cache directory path
#[inline]
#[must_use]
pub fn cache_dir() -> &'static str {
    &CACHE_DIR
}
/// Returns the thumbnails cache directory path
#[inline]
#[must_use]
pub fn thumbnails_dir() -> String {
    [&CACHE_DIR, "thumbnails/"].concat()
}
/// Returns the configuration directory path
#[inline]
#[must_use]
pub fn config_dir() -> &'static str {
    &CONFIG_DIR
}
/// Returns the `songs` file path, which contains the serialized info of library songs
#[inline]
#[must_use]
pub fn songs_file() -> String {
    [config_dir(), "songs"].concat()
}
/// Returns the `queue` file path, used to restore the previous queue
#[inline]
#[must_use]
pub fn queue_file() -> String {
    [config_dir(), "queue"].concat()
}
/// Returns the `shuffled_queue` file path, used to remember
/// the previous shuffled songs order in the restored queue
#[inline]
#[must_use]
pub fn shuffled_queue_file() -> String {
    [config_dir(), "shuffled_queue"].concat()
}
