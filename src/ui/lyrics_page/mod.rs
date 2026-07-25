use adw::subclass::prelude::*;
use gtk::glib;
use gtk::prelude::*;
use std::time::Duration;

mod imp;

glib::wrapper! {
    pub struct LyricsPage(ObjectSubclass<imp::LyricsPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements
            gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::Orientable, gtk::ConstraintTarget;
}

impl LyricsPage {
    pub fn set_content(&self, song_title: &str, lyrics: &str) {
        self.set_lyrics(song_title, lyrics);
    }

    pub fn set_lyrics(&self, song_title: &str, lrc_raw: &str) {
        let imp = self.imp();
        imp.lyrics_box.add_css_class("lyrics-list");
        imp.song_title.set_label(song_title);
        while let Some(child) = imp.lyrics_box.first_child() {
            imp.lyrics_box.remove(&child);
        }

        let parsed_lines = parse_lrc(lrc_raw);
        for line in &parsed_lines {
            let label_text = if line.text.is_empty() { " " } else { &line.text };
            let label = gtk::Label::builder()
                .label(label_text)
                .justify(gtk::Justification::Center)
                .wrap(true)
                .margin_top(8)
                .margin_bottom(8)
                .build();
            label.add_css_class("dim-label");
            imp.lyrics_box.append(&label);
        }
        *imp.lines.borrow_mut() = parsed_lines;
        imp.current_index.set(None);
    }

    pub fn update_position(&self, position: Duration) {
        let imp = self.imp();
        imp.lyrics_box.queue_allocate();
        
        let lines = imp.lines.borrow();
        if lines.is_empty() {
            return;
        }
        let mut target_index = lines
            .iter()
            .rposition(|line| line.timestamp <= position);
        if let Some(idx) = target_index {
            if idx > 0 && lines[idx].timestamp == lines[idx - 1].timestamp {
                target_index = Some(idx - 1);
            }
        }
        if target_index != imp.current_index.get() {
            if let Some(old_idx) = imp.current_index.get() {
                if let Some(row) = imp.lyrics_box.row_at_index(old_idx as i32) {
                    if let Some(label) = row.child().and_downcast::<gtk::Label>() {
                        label.remove_css_class("title-2");
                        label.add_css_class("dim-label");
                    }
                }
            }
            if let Some(new_idx) = target_index {
                if let Some(row) = imp.lyrics_box.row_at_index(new_idx as i32) {
                    if let Some(label) = row.child().and_downcast::<gtk::Label>() {
                        label.remove_css_class("dim-label");
                        label.add_css_class("title-2");
                    }
                    let vadjustment = imp.scrolled_window.vadjustment();
                    let row_alloc = row.allocation();
                    let row_center = row_alloc.y() as f64 + (row_alloc.height() as f64 / 2.0);
                    let visible_height = imp.scrolled_window.height() as f64;
                    let target_scroll = row_center - (visible_height / 2.0);
                    let max_scroll = vadjustment.upper() - vadjustment.page_size();
                    let clamped_scroll = target_scroll.clamp(0.0, max_scroll.max(0.0));
                    let adj = vadjustment.clone();
                    glib::idle_add_local_once(move || {
                        adj.set_value(clamped_scroll);
                    });
                }
            }
            imp.current_index.set(target_index);
        }
    }
}

fn parse_lrc(lrc_raw: &str) -> Vec<imp::LyricLine> {
    let mut lines = Vec::new();
    for line in lrc_raw.lines() {
        if line.starts_with('[') && line.contains(']') {
            let parts: Vec<&str> = line.splitn(2, ']').collect();
            if parts.len() == 2 {
                let time_str = &parts[0][1..];
                if let Some(duration) = parse_time(time_str) {
                    lines.push(imp::LyricLine {
                        timestamp: duration,
                        text: parts[1].trim().to_string(),
                    });
                }
            }
        }
    }
    lines.sort_by_key(|l| l.timestamp);
    lines
}

fn parse_time(time_str: &str) -> Option<Duration> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() == 2 {
        let mins: u64 = parts[0].parse().ok()?;
        let secs: f64 = parts[1].parse().ok()?;
        let total_ms = (mins * 60 * 1000) + (secs * 1000.0) as u64;
        Some(Duration::from_millis(total_ms))
    } else {
        None
    }
}
