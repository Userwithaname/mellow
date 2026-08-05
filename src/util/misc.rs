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
/// - If `split_by` is a large UTF-8 character
/// - If `split_by` is `'\'` when running in debug mode
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
    // NOTE: It might make sense to remove support for `split_by` entirely,
    // but the function should then be renamed (and moved to `serializer`?)
    debug_assert!(split_by != '\\', r"Cannot split by '\'");
    debug_assert!(
        split_by.len_utf8() == 1,
        "UTF-8 length must be 1 (was {} for '{split_by}')",
        split_by.len_utf8(),
    );

    let mut output = Vec::new();
    let mut extend_output = |item: &str| {
        // NOTE: Surprisingly, it is actually faster to allocate new strings
        // using `format!` for every split, rather than defining it outside
        // (presumably this may vary based on number of splits?)
        let split_by_unescaped = format!(r"\{split_by}");
        output.push(
            // SAFETY: The unescape character (\) has a UTF-8 length of 1 byte
            item.replace(&split_by_unescaped, unsafe {
                split_by_unescaped.get_unchecked(1..)
            })
            .to_owned(),
        );
    };

    let mut unescape = 0u8; // Counter for consecutive backslash characters
    let (mut start, mut end) = (0, 0);
    for char in input.chars() {
        if char == '\\' {
            unescape += 1;
        } else {
            // Split at `split_by`, unless `unescape` is odd
            if char == split_by && unescape & 1 == 0 {
                extend_output(input[start..end].trim());
                start = end + 1; // `len_utf8` for `split_by` is expected to be 1
            }
            unescape = 0;
        }
        end += char.len_utf8();
    }
    match input[start..].trim() {
        last if !last.is_empty() => extend_output(last),
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
