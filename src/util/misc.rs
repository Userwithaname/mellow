use std::path::PathBuf;
use std::{fs, io};

/// Splits the `input` at every occurence of the `split_by`
/// character, and returns the parts in-between as a `Vec`
///
/// - Supports `\` as the un-escape character in the `input`
/// - Whitespaces around each split are trimmed, and escape
///   characters are not included in the output result
/// - Note that splitting by `\` is not possible
///
/// # Example
/// ```rust
/// # use mellow::util::unescaped_split;
/// #
/// assert_eq!(
///     unescaped_split(r"Testing, testing\, one two, three", ',').as_ref(),
///     vec!["Testing", "testing, one two", "three"]
/// );
/// assert_eq!(
///     unescaped_split(r"Testing? testing\? one two? three", '?').as_ref(),
///     vec!["Testing", "testing? one two", "three"]
/// );
/// ```
#[inline]
#[must_use]
pub fn unescaped_split(input: &str, split_by: char) -> Vec<String> {
    let split_by_u8 = split_by as u8;
    let split_by_str = &[split_by_u8];
    // SAFETY: Cannot be invalid because `split_by` is of type `char`
    let split_by_str = unsafe { str::from_utf8_unchecked(split_by_str) };
    let chars: Vec<u8> = input.bytes().collect();

    let mut start = 0;
    let mut output = Vec::new();
    for i in 0..chars.len() {
        if chars[i] == split_by_u8 {
            if i > 0 && chars[i - 1] == b'\\' && (i < 2 || chars[i - 2] != b'\\') {
                continue;
            }
            output.push(
                input[start..i]
                    .replace(&format!("\\{split_by}"), split_by_str)
                    .trim()
                    .to_owned(),
            );
            start = i + 1;
        }
    }
    match input[start..].trim().to_owned() {
        last if !last.is_empty() => output.push(last),
        _ => (),
    }
    output
}

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
