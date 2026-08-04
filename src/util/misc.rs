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
/// # Panics
/// - If `split_by` is `'\'` when running in debug mode
/// - If `split_by` is a large UTF-8 character
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
    let split_by_str = &[split_by as u8];
    // SAFETY: Cannot be invalid because `split_by` is of type `char`
    let split_by_str = unsafe { str::from_utf8_unchecked(split_by_str) };

    debug_assert!(split_by_str != r"\", r"Cannot split by '\'");
    debug_assert!(
        split_by.len_utf8() == 1,
        "UTF-8 length must be 1 (was {} for '{split_by}')",
        split_by.len_utf8(),
    );

    let mut start = 0;
    let mut end = 0;
    let mut unescape = 0u8; // Counter for consecutive backslash characters
    let mut output = Vec::new();
    for char in input.chars() {
        if char == '\\' {
            unescape += 1
        } else {
            // If the `unescape` count is even, do not unescape
            if char == split_by && unescape & 1 == 0 {
                output.push(
                    input[start..end]
                        .replace(&format!("\\{split_by}"), split_by_str)
                        .trim()
                        .to_owned(),
                );
                start = end + 1; // `len_utf8` for `split_by` is expected to be 1
            }
            unescape = 0
        }
        end += char.len_utf8();
    }
    match input[start..].trim() {
        last if !last.is_empty() => output.push(last.to_owned()),
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
