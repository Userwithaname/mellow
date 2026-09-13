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
    /// - Only supports one timestamp per lyric line (non-repeating)
    /// - Lines with multiple timestamps (repeating lines) are not supported
    /// - Lines prefixed with unsupported tags will be skipped
    /// - Does not support extension features
    ///
    /// The _LRC (file format)_ page from Wikipedia was used as reference
    /// for implementation: <https://en.wikipedia.org/wiki/LRC_(file_format)>
    #[inline]
    #[must_use]
    pub fn try_parse_synced(contents: String) -> Lyrics {
        let synced_lyrics: Vec<SyncedLyric> = (contents.lines())
            .filter_map(|line| {
                let (time, lyric) = line.split_once(']')?;
                Some(SyncedLyric {
                    time_ms: timestamp_to_ms(time.get(1..)?)?,
                    lyric: lyric.to_owned(),
                })
            })
            .collect();
        match synced_lyrics.is_empty() {
            false => Lyrics::Synced(synced_lyrics),
            true => Lyrics::Unsynced(contents),
        }
    }
}
