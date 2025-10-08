use core::error::Error;
use gst::ClockTime;
use gtk::gio::{self, prelude::FileExt};
use lofty::picture::Picture;

pub struct Song {
    pub file: gio::File,
    pub album: Option<usize>,
    pub info: Option<SongInfo>,
}

#[derive(Default)]
pub struct SongInfo {
    pub title: String,
    pub album: String,
    pub artist: String,
    pub album_artist: String,
    pub track: String,
    pub year: String,
    pub lyrics: String,
    pub duration: ClockTime,
    // TODO: Move memory-heavy fields elsewhere?
    pub artwork: Option<Picture>,
}

impl Song {
    pub fn new(file: &str, album: Option<usize>) -> Result<Song, Box<dyn Error>> {
        Ok(Song {
            file: gio::File::for_path(file),
            album,
            info: None,
        })
    }

    pub fn file_uri(&self) -> String {
        self.file.uri().to_string()
    }

    pub fn get_info_or_assign(&mut self) -> &SongInfo {
        if self.info.is_none() {
            self.assign_info_with_fallback();
        }
        self.info.as_ref().unwrap()
    }

    pub fn assign_info_with_fallback(&mut self) {
        self.assign_info()
            .inspect_err(|e| eprintln!("Could not read song properties:\n{e}"))
            .unwrap_or_else(|_| {
                self.info = Some(SongInfo {
                    title: self.file.parse_name().to_string(),
                    album: String::new(),
                    artist: String::new(),
                    album_artist: String::new(),
                    track: String::new(),
                    year: String::new(),
                    lyrics: String::new(),
                    duration: ClockTime::from_seconds(0),
                    artwork: None,
                })
            })
    }

    pub fn assign_info(&mut self) -> Result<(), Box<dyn Error>> {
        // See: https://github.com/Serial-ATA/lofty-rs/blob/main/examples/tag_reader.rs

        use lofty::prelude::*;
        use lofty::probe::Probe;

        let tagged_file = Probe::open(self.file.path().unwrap())?.read()?;

        let tag = tagged_file
            .primary_tag()
            .or(tagged_file.first_tag())
            .ok_or("No tags found")?;

        let properties = tagged_file.properties();

        self.info = Some(SongInfo {
            title: tag.title().unwrap_or_default().to_string(),
            album: tag.album().unwrap_or_default().to_string(),
            artist: tag.artist().unwrap_or_default().to_string(),
            album_artist: tag
                .get_string(&ItemKey::AlbumArtist)
                .unwrap_or_default()
                .to_string(),
            track: tag.track().unwrap_or_default().to_string(),
            year: tag.year().unwrap_or_default().to_string(),
            lyrics: tag
                .get_string(&ItemKey::Lyrics)
                .unwrap_or_default()
                .to_string(),
            duration: ClockTime::from_mseconds(properties.duration().as_millis() as u64),
            artwork: if tag.picture_count() > 0 {
                Some(tag.pictures()[0].clone())
            } else {
                None
            },
        });

        Ok(())
    }
}
