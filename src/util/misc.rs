use core::hint::cold_path;
use std::path::{Path, PathBuf};
use std::{fs, io};

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
