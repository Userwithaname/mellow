use core::cmp::Ordering;

use crate::library::tag_list::Tags;
use crate::ui::LibraryObject;
use crate::util::CmpIsEqOr;

#[derive(Default)]
pub enum FilterMode {
    #[default]
    Exclusive,
    Inclusive,
}

#[derive(Default)]
pub struct LibraryFilters {
    pub filter_mode: FilterMode,
    pub rating: Option<(Ordering, u8)>,
    pub play_count: Option<(Ordering, u64)>,
    pub year: Option<(Ordering, u32)>,
    pub tag_filter_mode: FilterMode,
    pub tags: Tags,
}

impl LibraryFilters {
    #[inline]
    pub fn filter<F: LibraryObject>(&self, item: &F) -> bool {
        match self.filter_mode {
            FilterMode::Exclusive => self.filter_exclusive(item),
            FilterMode::Inclusive => self.filter_inclusive(item),
        }
    }
    pub fn filter_exclusive<F: LibraryObject>(&self, item: &F) -> bool {
        self.rating.is_none_or(
            |rating| (item.stars().total_cmp(&(rating.1 as f64))).is_eq_or(rating.0), //
        ) && self.play_count.is_none_or(|play_count| {
            (item.play_count().total_cmp(&(play_count.1 as f64))).is_eq_or(play_count.0)
        }) && (self.year).is_none_or(|year| item.year().cmp(&year.1).is_eq_or(year.0))
            && (self.tags.is_empty() || self.filter_tags(item))
    }
    pub fn filter_inclusive<F: LibraryObject>(&self, item: &F) -> bool {
        ((self.rating.is_none() && self.play_count.is_none() && self.year.is_none())
            || self.rating.is_some_and(|rating| {
                (item.stars().total_cmp(&(rating.1 as f64))).is_eq_or(rating.0)
            })
            || self.play_count.is_some_and(|play_count| {
                (item.play_count().total_cmp(&(play_count.1 as f64))).is_eq_or(play_count.0)
            })
            || (self.year).is_some_and(|year| item.year().cmp(&year.1).is_eq_or(year.0)))
            && (self.tags.is_empty() || self.filter_tags(item))
    }
    pub fn filter_tags<F: LibraryObject>(&self, item: &F) -> bool {
        let mut item_tags = Tags::from(item.tags());
        match self.tag_filter_mode {
            FilterMode::Exclusive => {
                if item_tags.is_empty() && *self.tags == ["untagged"] {
                    return true;
                }
                for tag in &*self.tags {
                    if !item_tags.contains(tag) {
                        item_tags.remove(tag);
                        return false;
                    }
                }
                true
            }
            FilterMode::Inclusive => self.tags.iter().any(|tag| {
                item_tags.contains(tag) || item_tags.is_empty() && tag == "untagged" //
            }),
        }
    }
}
