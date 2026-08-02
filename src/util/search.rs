pub const SCORE_THRESHOLD: f64 = 0.75;

/// Returns a number between 0 and 1 depending on
/// how well the `query` matches the `item`
///
/// Comparison is case-sensitive, and is based on
/// individual words
///
/// - If words in the `query` are ordered differently
///   from, or a word is not a substring of `item`, the
///   result is always 0
/// - Partially matching words will lower the overall
///   score, depending on how long the word from `query`
///   is compared to the word in `item`
/// - Skipped words result in a lower overall score,
///   depending on how long the skipped words are
///
/// # Example
/// ```rust
/// # use mellow::util::search::query_score;
/// #
/// assert_eq!(query_score("happy day", "happy tuesday"), 0.7142857142857143);
/// assert_eq!(query_score("test", "test"), 1.0);
/// assert_eq!(query_score("test", "TEST"), 0.0);
/// assert_eq!(query_score("test", "testing"), 0.5714285714285714);
/// assert_eq!(query_score("testing", "test"), 0.0);
/// assert_eq!(query_score("apple", "pineapple"), 0.5555555555555556);
/// assert_eq!(query_score("apples", "oranges"), 0.0);
/// assert_eq!(query_score("", "something"), 1.0);
/// assert_eq!(query_score("nothing", ""), 0.0);
/// ```
#[inline]
#[must_use]
pub fn query_score(query: &str, item: &str) -> f64 {
    if query.is_empty() {
        return 1.0;
    }
    if item.is_empty() {
        return 0.0;
    }

    let query_words = query.split(' ').collect::<Vec<&str>>();
    let item_words = item.split(' ').collect::<Vec<&str>>();
    let mut first_word = true;
    let mut last_word_index = 0;
    let mut score = 0.0;
    for word in &query_words {
        let mut max_match_score = 0.0;
        let mut max_match_index = 0;
        for (i, item_word) in item_words.iter().enumerate() {
            if item_word.contains(word) && (i > last_word_index || first_word) {
                let word_score = word.len() as f64 / item_word.len() as f64;
                if word_score > max_match_score {
                    max_match_score = match first_word {
                        false => word_score / (i - last_word_index) as f64,
                        true => word_score,
                    };
                    max_match_index = i;
                }
            }
        }
        if max_match_score < 0.01 && !word.is_empty() {
            return 0.0;
        }
        score += max_match_score;

        last_word_index = max_match_index;
        first_word = false;
    }
    score / query_words.len() as f64
}

#[inline]
#[must_use]
pub fn query_score_simple(query: &str, item: &str) -> f64 {
    let words = query.split(' ').collect::<Vec<&str>>();
    let mut missed_words = 0.0;
    for word in &words {
        if !item.contains(word) {
            missed_words += 1.0;
        }
    }
    1.0 - missed_words * (1.0 / words.len() as f64)
}
