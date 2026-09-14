use crate::util::timestamp_to_ms;

pub enum Lyrics {
    Unsynced(String),
    Synced(Vec<SyncedLyric>),
}

pub struct SyncedLyric {
    pub time_ms: usize,
    pub lyric: String,
}

impl Default for Lyrics {
    fn default() -> Self {
        Lyrics::Unsynced(String::new())
    }
}
impl Lyrics {
    /// Attempts to parse `contents` as synced lyrics in LRC format,
    /// and returns either `Lyrics::Synced` if parsing was successful,
    /// or `Lyrics::Unsynced` with the original `contents` upon failure.
    ///
    /// # Limitations
    /// - Lines must begin with a timestamp, or they will be skipped
    /// - Only timestamp tags are supported, others will be skipped
    /// - Unsupported tags appearing after the timestamp may be included
    ///   in the actual lyrics text
    /// - Does not support extension features
    ///
    /// The _LRC (file format)_ page from Wikipedia was used as reference
    /// for implementation: <https://en.wikipedia.org/wiki/LRC_(file_format)>
    #[inline]
    #[must_use]
    pub fn try_parse_synced(contents: String) -> Lyrics {
        let mut synced_lyrics = Vec::<SyncedLyric>::new();
        for line in contents.lines() {
            let mut split = line.split(']');
            let Some(lyric) = split.next_back() else {
                continue;
            };

            let mut times = Vec::new();
            let mut cur_split = split.next();
            while let Some(Some(Some(time))) = cur_split.map(|s| s.get(1..).map(timestamp_to_ms)) {
                cur_split = split.next();
                times.push(time);
            }

            // Handling for the ']' character in the actual lyrics
            let mut lyric = lyric.to_owned();
            let mut reinsert = String::new();
            while let Some(part) = cur_split {
                cur_split = split.next();
                reinsert.push_str(part);
                reinsert.push(']');
            }
            if !reinsert.is_empty() {
                lyric = [reinsert, lyric].concat();
            }

            for time_ms in times {
                match synced_lyrics.binary_search_by(|existing| existing.time_ms.cmp(&time_ms)) {
                    Err(index) | Ok(index) => {
                        let lyric = lyric.to_owned();
                        synced_lyrics.insert(index, SyncedLyric { time_ms, lyric });
                    }
                }
            }
        }

        match synced_lyrics.is_empty() {
            false => Lyrics::Synced(synced_lyrics),
            true => Lyrics::Unsynced(contents),
        }
    }
}
