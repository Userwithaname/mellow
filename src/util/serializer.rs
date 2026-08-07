use std::ops::Deref;

/// Serializes the given value/field pairs into a `String`,
/// which can be used with `deserialize!()` to retreive the
/// values afterwards
///
/// # Example
/// ```rust
/// # use mellow::util::serialize;
/// use mellow::util::serialize_list;
///
/// let number = 5;
/// let text = "hello";
/// let list = &[
///     "one",
///     "two",
///     "three, four",
/// ];
/// let numbers = &[1, 2, 3, 4];
///
/// assert_eq!(
///     serialize! {
///         number => "number",
///         text => "text",
///         serialize_list(list) => "list",
///         serialize_list(&numbers.map(|n| n.to_string())) => "numbers",
///     },
///     "\
/// number: 5
/// text: hello
/// list: one, two, three\\, four
/// numbers: 1, 2, 3, 4
/// "
/// );
/// ```
#[macro_export]
macro_rules! serialize {
    {$($value:expr => $field:tt,)+} => {
        [$($field, ": ", &$value.to_string(), "\n",)+].concat()
    };
}
pub use serialize;

/// Combines `list` elements into a single `String` separated by commas,
/// which can be used with `deserialize_list` and the `serialize!()` macro
///
/// Warning: Unescaped backslash before an unescaped comma is currently not
/// handled in `deserialize_list`, and will result in duplicate backslashes
///
/// # Example
/// ```rust
/// # use mellow::util::serialize_list;
/// #
/// assert_eq!(
///     serialize_list(&[
///         "one",
///         "two",
///         "three, four",
///     ]),
///     r"one, two, three\, four"
/// );
/// assert_eq!(
///     serialize_list(&[
///         r"one\",
///         r"two\\",
///         r"three\, \four\",
///     ]),
///     r"one\\, two\\\\, three\\\, \four\"
/// );
/// ```
#[inline]
#[must_use]
pub fn serialize_list<S: Deref<Target = str>>(list: &[S]) -> String {
    let mut out = String::new();

    // `unescape` is used to count consecutive backslash characters
    let mut unescape = 0usize;
    let (mut start, mut end);
    for item in list {
        (start, end, unescape) = (0, 0, 0);
        for char in item.chars() {
            match char {
                ',' => {
                    out.push_str(&item[start..end]);
                    out.push_str(&r"\".repeat(unescape + 1)); // Unescape backslashes and comma
                    out.push(char);
                    start = end + 1; // Start next after the comma (',' is 1 byte)
                    unescape = 0;
                }
                '\\' => unescape += 1,
                _ => unescape = 0,
            }
            end += char.len_utf8();
        }
        out.push_str(&item[start..end]);
        out.push_str(&r"\".repeat(unescape)); // Unescape backslashes before comma
        out.push_str(", ");
    }
    // Remove last ", " and excess backslashes (unescaping is only needed before commas)
    if !out.is_empty() {
        out.truncate(out.len() - 2 - unescape);
    }

    out
}

/// Takes serialized `data` and assigns the parsed values of fields
/// into the specified variables using a `match`-like syntax
///
/// Pattern matching is non-exhaustive; fields missing from `data`
/// will be skipped silently without parsing or assigning
///
/// The following types are supported:
/// - `str` for assigning string slices
/// - `String` for assigning owned strings
/// - `?` for types implementing the `FromStr` trait
/// - All types (except `str`) can be wrapped in square brackets
///   (`[…]`) to parse them as lists (such as `Vec`s)
/// - A special case exists for `[?String]`, which is the same as
///   `[String]`, but converts the `Vec<String>` using `.into()`
///
/// Deserializing lists requires `deserialize_list` to be available
/// in scope where this macro is invoked
///
/// # Panics
/// Panics when parsing invalid data for types `?` and `[?]`
///
/// # Example
/// ```rust
/// # use mellow::util::deserialize;
/// use mellow::util::deserialize_list;
///
/// let mut number = 0;
/// let mut text = String::new();
/// let mut text_str = "";
/// let mut numbers: Vec<usize> = Vec::new();
/// let mut list = Vec::new();
///
/// let data = "\
/// number: 5
/// text: hello
/// text_str: hi
/// numbers: 1, 2, 3, 4
/// list: one, two, three\\, four
/// ";
///
/// deserialize! {
///     data => {
///         "number"<?> => number,
///         "text"<String> => text,
///         "text_str"<str> => text_str,
///         "numbers"<[?]> => numbers,
///         "list"<[String]> => list,
///     }
/// }
///
/// assert_eq!(number, 5);
/// assert_eq!(text, "hello".to_string());
/// assert_eq!(text_str, "hi");
/// assert_eq!(numbers, [1, 2, 3, 4]);
/// assert_eq!(list, ["one", "two", "three, four"]);
/// ```
#[macro_export]
macro_rules! deserialize {
    {$data:tt => {$($field:tt<$type:tt> => $target:expr,)+}} => {
        for line in $data.lines() {
            let Some((field, value)) = line.split_once(": ") else {
                continue;
            };

            match field {
                $($field => {
                    $target = deserialize!(@to_value $type, value, field);
                },)+
                _ => eprintln!("Unknown field: `{field}`"),
            }
        }
    };

    (@to_value ?, $value:expr, $field:expr) => {
        $value.parse().map_err(|e| format!("{} {e}", $field)).unwrap()
    };
    (@to_value [?], $value:expr, $field:expr) => {
        $value.split(',').into_iter().map(|value| value.trim().parse().unwrap()).collect()
    };
    (@to_value str, $value:expr, $field:expr) => {
        $value
    };
    (@to_value String, $value:expr, $field:expr) => {
        $value.to_owned()
    };
    (@to_value [String], $value:expr, $field:expr) => {
        deserialize_list($value)
    };
    (@to_value [?String], $value:expr, $field:expr) => {
        deserialize_list($value).into()
    };
}
pub use deserialize;

/// Returns a `Vec<String>` built from comma-separated values within `input`
///
/// - Commas can be unescaped using backslash (`\`)
/// - Unescape characters before commas will not be included
/// - Backslashes should only be unescaped when placed before a comma
/// - Note that unescaping a backslash before an unescaped comma will not
///   deserialize correctly (see examples)
///
/// # Examples
/// ```rust
/// # use mellow::util::deserialize_list;
/// #
/// assert_eq!(*deserialize_list("one, two, three"), ["one", "two", "three"]);
/// assert_eq!(
///     *deserialize_list(r"first, second\, with a comma, third"),
///     ["first", "second, with a comma", "third"]
/// );
/// assert_eq!(
///     *deserialize_list(r"element ending with a backslash\\, another \element"),
///     [r"element ending with a backslash\", r"another \element"]
/// );
/// ```
///
/// Unescaped backslashes before an unescaped comma in the input will currently
/// not remove the excess backslashes:
/// ```rust
/// # use mellow::util::deserialize_list;
/// #
/// let list = deserialize_list(r"one\, two\\, three\\\, four\\\\, test");
/// // Expected:
/// assert_ne!(*list, [r"one, two\,", r"three\, four\\", "test"]);
/// // Actual (notice how "three" has twice as many backslashes than expected):
/// assert_eq!(*list, [r"one, two\", r"three\\, four\\", "test"]);
/// ```
#[inline]
#[must_use]
pub fn deserialize_list(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut extend_output = |item: &str| out.push(item.replace(r"\,", ","));

    // `unescape` is used to count consecutive backslash characters
    let (mut start, mut end, mut unescape) = (0, 0, 0);
    for char in input.chars() {
        match char {
            ',' => {
                // Split at comma if not unescaped (if `unescape` is even)
                if unescape & 1 == 0 {
                    extend_output(input[start..end - (unescape / 2)].trim());
                    start = end + 1; // Start next after the comma (',' is 1 byte)
                } else {
                    // TODO: Remove excess backslash characters when `unescape` is odd
                    // One way would be to initialize the output with an empty element
                    // beforehand and build it in chunks, then push a new empty string
                    // in the above `if` branch, increment the target index, and repeat
                    // (remove the excess element if necessary). Be mindful of performance.
                }
                unescape = 0;
            }
            '\\' => unescape += 1,
            _ => unescape = 0,
        }
        end += char.len_utf8();
    }
    if start < input.len()
        // COMPAT: Lists from versions <=0.4.1 end with ", "
        && let last = input[start..].trim()
        && !last.is_empty()
    {
        extend_output(last);
    }

    out
}
