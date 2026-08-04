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
///     "one".to_string(),
///     "two".to_string(),
///     "three, four".to_string(),
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
/// list: one, two, three\\, four, \n\
/// numbers: 1, 2, 3, 4, \n\
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
/// which can be used with the `serialize!()` macro
///
/// Warning: The `list` will not serialize correctly if an element contains
/// an unescaped comma (`\,`, or any odd number of backslashes before it)
///
/// # Example
/// ```rust
/// # use mellow::util::serialize_list;
/// #
/// assert_eq!(
///     serialize_list(&[
///         "one".to_string(),
///         "two".to_string(),
///         "three, four".to_string(),
///     ]),
///     r"one, two, three\, four, "
/// );
/// ```
#[inline]
#[must_use]
pub fn serialize_list(list: &[String]) -> String {
    list.iter().map(|s| s.replace(',', r"\,") + ", ").collect()
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
/// Deserializing lists requires `unescaped_split` to be available
/// in scope where this macro is invoked
///
/// # Panics
/// Panics when parsing invalid data for types `?` and `[?]`
///
/// # Example
/// ```rust
/// # use mellow::util::deserialize;
/// use mellow::util::unescaped_split;
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
/// list: one, two, three\\, four,
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
        unescaped_split($value, ',')
    };
    (@to_value [?String], $value:expr, $field:expr) => {
        unescaped_split($value, ',').into()
    };
}
pub use deserialize;
