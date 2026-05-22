use adw::{prelude::*, subclass::prelude::*};
use core::cell::{Cell, OnceCell, RefCell};
use core::time::Duration;
use gtk::CompositeTemplate;
use gtk::{gdk, gio, glib, graphene};
use std::rc::Rc;

use crate::excuses::{EXP_INIT, EXP_RX};
use crate::library::{Library, library_tx};
use crate::player::{PlayerRequest, QueueItem, player_tx};
use crate::ui::queue_page::QueueScrollAction;
use crate::ui::{ListRow, QueueItemObject, QueueSubpage};
use crate::ui::{UpdateUI, fallback_song_image, ui_tx};
use crate::util::wrap_index;

const NUM_ITEMS_AHEAD: usize = 45;
const NUM_ITEMS_BEHIND: usize = 45;
const ROW_HEIGHT: usize = 55;
const PAN_UP_BUTTON_HEIGHT: i32 = 44;
const PAN_REPEAT_INTERVAL: Duration = Duration::from_millis(150);

#[derive(Default, CompositeTemplate)]
#[template(file = "queue_page.ui")]
pub struct QueuePage {
    #[template_child]
    header_normal: TemplateChild<adw::HeaderBar>,
    #[template_child]
    pub shuffle_toggle: TemplateChild<gtk::ToggleButton>,
    #[template_child]
    pub repeat_toggle: TemplateChild<gtk::ToggleButton>,

    #[template_child]
    header_selection: TemplateChild<adw::HeaderBar>,
    #[template_child]
    pub remove_selection: TemplateChild<gtk::Button>,

    pub selection_mode: Rc<Cell<Option<usize>>>,

    #[template_child]
    list_box: TemplateChild<gtk::ListBox>,
    #[template_child]
    drag_widget: TemplateChild<gtk::Fixed>,
    #[template_child]
    scrolled_window: TemplateChild<gtk::ScrolledWindow>,

    #[template_child]
    view_stack: TemplateChild<adw::ViewStack>,
    #[template_child]
    view_further_up: TemplateChild<gtk::Button>,
    #[template_child]
    view_further_down: TemplateChild<gtk::Button>,
    #[template_child]
    pub to_playing: TemplateChild<gtk::Button>,

    pub subpage: OnceCell<QueueSubpage>,

    view_pan_offset: Cell<isize>,
    queue_item_objects: Rc<RefCell<Vec<QueueItemObject>>>,
    list_model: OnceCell<gio::ListStore>,
    pub drag_row: OnceCell<ListRow>,
    pub next_scroll_pos: Cell<QueueScrollAction>,
    pan_loop_direction: Rc<Cell<PanLoopDirection>>,

    pub song_queue: RefCell<Box<[QueueItem]>>,
    pub playing_index: Cell<usize>,
    queue_length: Cell<usize>,
    last_repeat_mode: Cell<bool>,
}

#[derive(Default, Copy, Clone)]
enum PanLoopDirection {
    Up,
    #[default]
    None,
    Down,
}

#[derive(Debug)]
struct ItemNotFoundError;

#[gtk::template_callbacks]
impl QueuePage {
    #[template_callback]
    pub fn handle_set_repeat(&self, toggle_button: &gtk::ToggleButton) {
        (player_tx().send(PlayerRequest::SetRepeat(toggle_button.is_active()))).expect(EXP_RX);
    }
    #[template_callback]
    pub fn handle_set_shuffle(&self, toggle_button: &gtk::ToggleButton) {
        (player_tx().send(PlayerRequest::SetShuffle(toggle_button.is_active()))).expect(EXP_RX);
    }
    #[template_callback]
    pub fn handle_open_library(&self) {
        ui_tx().send(UpdateUI::FocusLibrary).expect(EXP_RX);
    }
    #[template_callback]
    pub fn handle_exit_selection(&self) {
        self.set_selection_mode(None);
    }
    #[template_callback]
    pub fn handle_remove_selected(&self) {
        let mut selected_items = Vec::<usize>::new();
        let list_model = self.list_model.get().expect(EXP_INIT);
        for i in 0..list_model.n_items() {
            let item = (list_model.item(i).and_downcast::<QueueItemObject>()).unwrap();
            if item.selected() {
                let index = item.index() as usize;
                selected_items.insert(
                    selected_items
                        .binary_search_by(|item| index.cmp(item))
                        .unwrap_err( /* each index is unique */ ),
                    index,
                );
            }
        }
        if selected_items.is_empty() {
            return;
        }

        (ui_tx().send(UpdateUI::Notification(
            format!("Removed {} items from the queue", selected_items.len()),
            Some(Box::new((
                "Undo",
                Box::new(|| player_tx().send(PlayerRequest::Undo).expect(EXP_RX)),
            ))),
        )))
        .expect(EXP_RX);

        let _ = player_tx().send(PlayerRequest::RemoveItems(selected_items));
        self.set_selection_mode(None);
    }
    #[template_callback]
    pub fn handle_pan_up(&self) {
        self.next_scroll_pos.set(QueueScrollAction::Retain);
        self.view_pan_offset
            .set((self.view_pan_offset.get() - 1) % self.queue_length.get() as isize);
        self.draw_queue(&self.song_queue.borrow(), self.playing_index.get());
    }
    #[template_callback]
    pub fn handle_pan_down(&self) {
        self.next_scroll_pos.set(QueueScrollAction::Offset(1));
        self.view_pan_offset
            .set((self.view_pan_offset.get() + 1) % self.queue_length.get() as isize);
        self.draw_queue(&self.song_queue.borrow(), self.playing_index.get());
    }
    #[template_callback]
    pub fn handle_show_playing(&self) {
        if let Ok(model_index) = self.queue_index_to_model(self.playing_index.get())
            && (self.view_pan_offset.get() == 0 || self.selection_mode.get().is_some())
        {
            self.scroll_to_model_item(model_index);
        } else {
            self.next_scroll_pos.set(QueueScrollAction::ToPlaying);
            self.draw_queue(&self.song_queue.borrow(), self.playing_index.get());
        }
    }

    #[inline]
    fn start_pan_loop(&self, direction: PanLoopDirection) {
        self.pan_loop_direction.set(direction);
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to=queue_page)]
            self,
            async move {
                loop {
                    match queue_page.pan_loop_direction.get() {
                        PanLoopDirection::Down => queue_page.handle_pan_down(),
                        PanLoopDirection::None => return,
                        PanLoopDirection::Up => queue_page.handle_pan_up(),
                    }
                    glib::timeout_future(PAN_REPEAT_INTERVAL).await;
                }
            }
        ));
    }
    #[inline]
    fn stop_pan_loop(&self) {
        self.pan_loop_direction.set(PanLoopDirection::None);
    }

    #[inline]
    pub fn scroll_to_pos(&self, scroll_target: f64) {
        let scrolled_window = self.scrolled_window.get();
        // WORKAROUND: Setting the scroll position in an idle task because it
        // doesn't update otherwise
        glib::idle_add_local(move || {
            scrolled_window.vadjustment().set_value(scroll_target);
            glib::ControlFlow::Break
        });
    }

    #[inline]
    pub fn scroll_to_item(&self, index: usize) {
        if let Ok(model_index) = self.queue_index_to_model(index) {
            self.scroll_to_model_item(model_index);

            #[cfg(debug_assertions)]
            self.model_index_to_queue_discrepancy_check(model_index, index);
        }
    }
    #[inline]
    pub fn scroll_to_model_item(&self, model_index: usize) {
        self.scroll_to_pos(
            (model_index * ROW_HEIGHT) as f64
                + (self.view_further_up.is_visible() as i32 * PAN_UP_BUTTON_HEIGHT
                    // NOTE: For some reason, `margin_top` only has
                    // to be accounted for when building with Meson
                    - self.list_box.margin_top()) as f64,
        );
    }

    #[inline]
    pub fn set_queue_items(&self, queue: Box<[QueueItem]>) {
        self.song_queue.replace(queue);
    }

    pub fn draw_queue(&self, queue: &[QueueItem], playing: usize) {
        let queue_length = queue.len();
        let old_queue_length = self.queue_length.replace(queue_length);
        self.view_stack
            .set_visible_child_name(match queue.is_empty() {
                true => "queue_empty",
                false => "song_queue",
            });

        // Exit selection mode before resetting the model
        self.set_selection_mode(None);

        // Panning offset has to be updated first to avoid having to draw twice
        if let QueueScrollAction::ToPlaying = self.next_scroll_pos.get() {
            self.view_pan_offset.set(0);
        }

        let center = wrap_index(playing as isize + self.view_pan_offset.get(), queue_length);
        let start = center.saturating_sub(NUM_ITEMS_BEHIND);
        let end = (center + NUM_ITEMS_AHEAD).min(queue.len());

        let mut items: Vec<QueueItemObject> = Self::items_to_objects(
            queue.iter().take(end).skip(start).enumerate(),
            playing,
            start,
        )
        .collect();

        let repeat_mode = self.repeat_toggle.is_active();

        let last_repeat_mode = self.last_repeat_mode.replace(repeat_mode);
        // FIX: Incorrect offset when toggling repeat mode on a short queue
        if repeat_mode && queue_length != 0 {
            let n_items_before = (NUM_ITEMS_BEHIND - (center - start)).min(queue_length - 1);
            if n_items_before > 0 {
                if repeat_mode != last_repeat_mode {
                    self.next_scroll_pos.set(QueueScrollAction::Offset(
                        n_items_before as i32, //
                    ));
                }
                let start = (queue.len() - n_items_before).max(center + 1);
                items.splice(
                    0..0,
                    Self::items_to_objects(queue.iter().skip(start).enumerate(), playing, start),
                );
            }
            let n_items_after = NUM_ITEMS_AHEAD - (end - center);
            if n_items_after > 0 {
                let end = n_items_after.min(center);
                items.extend(Self::items_to_objects(
                    queue.iter().take(end).enumerate(),
                    playing,
                    0,
                ));
            }
        } else if repeat_mode != last_repeat_mode {
            self.next_scroll_pos.set(QueueScrollAction::Offset(
                -(NUM_ITEMS_BEHIND.saturating_sub(center) as i32),
            ));
        }

        let list_model = self.list_model.get().unwrap();
        list_model.splice(0, list_model.n_items(), &items);
        self.queue_item_objects.replace(items);
        self.playing_index.set(playing);

        let last_up_button_visible = self.view_further_up.is_visible();
        let up_button_visible = repeat_mode || center > NUM_ITEMS_BEHIND;
        self.view_further_up.set_visible(up_button_visible);
        self.view_further_down.set_visible(
            repeat_mode || queue_length.saturating_sub(center) > NUM_ITEMS_AHEAD, //
        );

        match self.next_scroll_pos.take() {
            // Re-apply the scroll position, because it resets on every change
            QueueScrollAction::Retain => self.scroll_to_pos({
                self.scrolled_window.vadjustment().value()
                    - ((last_up_button_visible ^ up_button_visible) as i32
                        * ((-1_i32).pow(up_button_visible as u32) * PAN_UP_BUTTON_HEIGHT))
                        as f64
            }),
            // Keep the same relative scroll position when repeat mode changes
            QueueScrollAction::Offset(offset) => self.scroll_to_pos(
                self.scrolled_window.vadjustment().value()
                    + ((last_up_button_visible ^ up_button_visible) as i32
                        * (-(-1_i32).pow(up_button_visible as u32) * PAN_UP_BUTTON_HEIGHT)
                        + offset * ROW_HEIGHT as i32) as f64,
            ),
            // Scroll to the currently playing item
            QueueScrollAction::ToPlaying => self.scroll_to_item(playing),
        }

        // Garbage collection
        if old_queue_length > 0 {
            // NOTE: If there are issues with queue artworks not appearing, try
            // disabling garbage collection to verify that it is working properly
            Library::run_task(library_tx(), {
                let queue = queue.to_vec();
                move || {
                    let Some(len) = queue.len().checked_sub(1) else {
                        return;
                    };
                    let short_start = playing.saturating_sub(2);
                    let short_end = (playing + 2).min(queue.len());
                    for (index, song) in queue.into_iter().enumerate() {
                        let QueueItem::Song(song) = song else {
                            return;
                        };

                        // Unload detailed artworks, but keep a few items ahead and behind loaded
                        if !(short_start..=short_end).contains(&index)
                            && (!repeat_mode
                                || !(index > len - 2usize.saturating_sub(playing)
                                    || index < 2usize.saturating_sub(len - playing)))
                        {
                            song.info().try_unload_detailed();
                        } else {
                            drop(song.info().load_detailed());
                            continue;
                        }

                        // Unload thumbnails that are no longer needed
                        if !(start..=end).contains(&index)
                            && (!repeat_mode
                                || !(index > len - NUM_ITEMS_BEHIND.saturating_sub(playing)
                                    || index < NUM_ITEMS_AHEAD.saturating_sub(len - playing)))
                        {
                            song.info().try_unload_thumbnail();
                        }
                    }
                }
            });
        }
    }

    #[inline]
    fn items_to_objects<I, 'i>(
        items_iter: I,
        playing_index: usize,
        start_index: usize,
    ) -> impl Iterator<Item = QueueItemObject>
    where
        I: Iterator<Item = (usize, &'i QueueItem)>,
    {
        items_iter.map(move |index_item| {
            let q_index = index_item.0 + start_index;
            QueueItemObject::new(
                q_index as u32,
                q_index == playing_index,
                index_item.1.clone(),
            )
        })
    }

    /// Takes a queue item index and returns the index to access its object
    ///
    /// # Panics
    /// Panics if `self.queue_item_objects` `RefCell` is mutably borrowed
    #[inline]
    fn queue_index_to_model(&self, queue_index: usize) -> Result<usize, ItemNotFoundError> {
        let queue_objects_length = self.queue_item_objects.borrow().len();
        let center = wrap_index(
            self.playing_index.get() as isize + self.view_pan_offset.get(),
            self.queue_length.get(),
        );
        match self.repeat_toggle.is_active() {
            false => {
                let start = center.saturating_sub(NUM_ITEMS_BEHIND);
                if queue_index < start {
                    return Err(ItemNotFoundError);
                }
                match queue_index + NUM_ITEMS_BEHIND.min(center) - center {
                    value if value >= queue_objects_length => Err(ItemNotFoundError),
                    value => Ok(value),
                }
            }
            true => {
                let queue_length = self.queue_length.get();
                let model_length = self.queue_item_objects.borrow().len();
                if queue_length == 0 {
                    return Err(ItemNotFoundError);
                }

                let start = center.saturating_sub(NUM_ITEMS_BEHIND);
                let n_items_before = NUM_ITEMS_BEHIND
                    .min(queue_length - 1 - start)
                    .saturating_sub(center - start);

                // Wrapping over the start of the queue
                if n_items_before > 0 && queue_index > center + NUM_ITEMS_AHEAD {
                    let from = queue_length - n_items_before;
                    match queue_index - from {
                        value if value >= model_length => return Err(ItemNotFoundError),
                        value => return Ok(value),
                    };
                }

                // Non-wrapped items
                if let Some(value) = (queue_index + NUM_ITEMS_BEHIND.min(center))
                    .checked_sub(center)
                    .map(|i| i + n_items_before)
                    && value < model_length
                {
                    return Ok(value);
                }

                // Wrapping over the end of the queue
                let n_items_after = queue_length - center.saturating_sub(NUM_ITEMS_AHEAD);
                if queue_index <= n_items_after {
                    match queue_index + n_items_after {
                        value if value >= model_length => return Err(ItemNotFoundError),
                        value => return Ok(value),
                    }
                }

                Err(ItemNotFoundError)
            }
        }
    }
    /// Takes a model index and returns the index to access its queue item
    #[inline]
    #[must_use]
    fn model_index_to_queue(&self, model_index: usize) -> usize {
        let center_index = wrap_index(
            self.playing_index.get() as isize + self.view_pan_offset.get(),
            self.queue_length.get(),
        );
        match self.repeat_toggle.is_active() {
            false => model_index + center_index - NUM_ITEMS_BEHIND.min(center_index),
            true => {
                let queue_length = self.queue_length.get();
                debug_assert!(
                    queue_length != 0,
                    "`model_index_to_queue` used on an empty queue"
                );

                // Wrapping over the start of the queue
                let n_items_behind = NUM_ITEMS_BEHIND
                    .min(queue_length - 1)
                    .saturating_sub(center_index);
                if n_items_behind > 0 {
                    // println!("Wrapping over the start of the queue");
                    if model_index < n_items_behind {
                        return queue_length - n_items_behind + model_index;
                    }
                    if model_index == n_items_behind {
                        return 0;
                    }
                }

                // Non-wrapped items
                let offset_index = model_index + center_index
                    - NUM_ITEMS_BEHIND.min(center_index)
                    - n_items_behind;
                if offset_index < queue_length {
                    // println!("Non-wrapped item");
                    return offset_index;
                }

                // Wrapping over the end of the queue
                // println!("Wrapping over the end of the queue");
                offset_index - queue_length
            }
        }
    }

    #[inline]
    fn for_each_row<F: Fn(&gtk::ListBoxRow, i32)>(&self, f: F) {
        let mut i = 0;
        while let Some(row) = &self.list_box.row_at_index(i) {
            f(row, i);
            i += 1;
        }
    }

    // #[inline]
    // fn for_each_object<F: Fn(&QueueItemObject, u32)>(&self, f: F) {
    //     let mut i = 0;
    //     let list_model = self.list_model.get().expect(EXP_INIT);
    //     while let Some(item) = list_model.item(i).and_downcast_ref::<QueueItemObject>() {
    //         f(item, i);
    //         i += 1;
    //     }
    // }

    #[inline]
    fn update_selection_count(&self, selections: usize) {
        if self.selection_mode.get().is_some() {
            self.selection_mode.set(Some(selections));
            self.remove_selection.set_sensitive(selections != 0);
        }
    }

    #[inline]
    fn set_selection_mode(&self, selection_mode: Option<usize>) {
        self.selection_mode.set(selection_mode);
        self.remove_selection
            .set_sensitive(selection_mode.is_some_and(|count| count > 0));
        let selection_mode = selection_mode.is_some();

        self.header_selection.set_visible(selection_mode);
        self.header_normal.set_visible(!selection_mode);

        // Disabling these buttons because selection mode resets when queue is redrawn
        self.view_further_up.set_sensitive(!selection_mode);
        self.view_further_down.set_sensitive(!selection_mode);

        let model = self.list_model.get().expect(EXP_INIT);
        self.for_each_row(|list_row, index| {
            let list_row = list_row.downcast_ref::<ListRow>().unwrap().imp();
            list_row.selection_toggle.set_visible(selection_mode);
            list_row.open_subpage_icon.set_visible(!selection_mode);
            if !selection_mode {
                list_row.set_selected(false);
                (model.item(index as u32).unwrap())
                    .downcast::<QueueItemObject>()
                    .unwrap()
                    .set_selected(false);
            }
        });
    }

    #[inline]
    pub fn assign_artwork(&self, index: usize, artwork: Option<&gdk::Texture>) {
        if let Ok(model_index) = self.queue_index_to_model(index) {
            self.queue_item_objects.borrow()[model_index].set_property("artwork", artwork);

            #[cfg(debug_assertions)]
            self.model_index_to_queue_discrepancy_check(model_index, index);
        }
    }

    #[inline]
    fn setup_model(&self) {
        let model = gio::ListStore::new::<QueueItemObject>();
        let selection_mode = Rc::clone(&self.selection_mode);
        let fallback_image = fallback_song_image();
        let queue_page = self.obj().clone();
        self.list_box.bind_model(Some(&model), move |object| {
            // FIX: Optimize: The UI momentarily hangs whenever the queue is updated
            //
            // Commenting out `list_model.splice` in `update_song_queue` resolves this,
            // which means it has to be related to the `bind_model` step in some way.
            //
            // It seems to be related to the rows trying to determine their heights,
            // and I am assuming that giving the rows a fixed height would solve the
            // performance issues as well.
            //
            // This is especially noticable when skipping songs using keyboard shortcuts

            let queue_item_object = object.downcast_ref::<QueueItemObject>().unwrap();
            let queue_row = ListRow::default();
            let row_imp = queue_row.imp();

            queue_row.set_title(&queue_item_object.title());
            queue_row.set_subtitle(&queue_item_object.subtitle());

            match queue_item_object.queue_item() {
                QueueItem::Song(_) => {
                    if queue_item_object.playing() {
                        queue_row.add_css_class("heading");
                        queue_row.add_css_class("card");
                    }

                    queue_row.add_bindings(&[
                        queue_item_object
                            .bind_property("artwork", &row_imp.prefix_image.get(), "paintable")
                            .sync_create()
                            .build(),
                        queue_item_object
                            .bind_property("selected", &row_imp.selection_toggle.get(), "active")
                            .sync_create()
                            .build(),
                    ]);

                    let artwork = queue_item_object.artwork();
                    if artwork.is_some() {
                        queue_row.set_prefix_image(artwork.as_ref());
                    } else {
                        queue_item_object.load_artwork();
                        queue_row.set_prefix_image(Some(&fallback_image));
                    }

                    queue_row.set_suffix_label(&queue_item_object.suffix());
                }
                QueueItem::Stopper(_) => {
                    queue_row.add_css_class("heading");
                    queue_row.add_css_class("dimmed");

                    // IDEA: Draw a pause icon in place of the album cover

                    queue_row.add_binding(
                        queue_item_object
                            .bind_property("selected", &row_imp.selection_toggle.get(), "active")
                            .sync_create()
                            .build(),
                    );
                }
            }

            let index = queue_item_object.index() as usize;
            let selection_mode = Rc::clone(&selection_mode);
            queue_row.connect_activated(glib::clone!(
                #[weak(rename_to=queue_page)]
                queue_page.imp(),
                #[weak]
                queue_item_object,
                move |_| match selection_mode.get() {
                    None => (ui_tx().send(UpdateUI::OpenQueueSubpage(index))).expect(EXP_RX),
                    Some(mut selections) => {
                        let selected = !queue_item_object.selected();
                        queue_item_object.set_selected(selected);
                        match selected {
                            true => selections += 1,
                            false => selections -= 1,
                        }
                        queue_page.update_selection_count(selections);
                    }
                }
            ));

            queue_row.upcast::<gtk::Widget>()
        });

        let _ = self.list_model.set(model);
    }
    /// Returns `true` if interaction at a given `start_pos_x`
    /// should drag the queue row, or `false` if not
    #[inline]
    const fn should_drag(start_pos_x: f64) -> bool {
        start_pos_x < 65.0
    }
    #[inline]
    fn setup_drag_and_drop(&self) {
        let drag = gtk::GestureDrag::new();
        let drag_row = ListRow::default();
        drag_row.add_css_class("osd");
        let drag_container = self.drag_widget.parent().unwrap();
        let dragged_item = Rc::new(Cell::new(None));
        let dragged_item_index = Rc::new(Cell::new(0));
        self.drag_widget.set_cursor_from_name(Some("grabbing"));
        self.drag_widget.put(&drag_row, 0.0, 0.0);
        let drag_offset = Rc::new(Cell::new((0.0, 0.0)));

        type DragState = Rc<Cell<bool>>;
        trait SetDragState {
            fn set_drag_state(&self, dragging: bool);
        }
        impl SetDragState for DragState {
            fn set_drag_state(&self, dragging: bool) {
                self.set(dragging);
                ui_tx()
                    .send(UpdateUI::CanCloseSheet(!dragging))
                    .expect(EXP_RX);
            }
        }
        let dragging: DragState = Rc::new(Cell::new(false));

        drag.connect_drag_begin(glib::clone!(
            #[weak(rename_to=queue_page)]
            self,
            #[weak(rename_to=list_box)]
            self.list_box,
            #[strong(rename_to=selection_mode)]
            self.selection_mode,
            #[weak(rename_to=scrolled_window)]
            self.scrolled_window,
            #[weak(rename_to=drag_widget)]
            self.drag_widget,
            #[weak]
            drag_row,
            #[weak]
            drag_offset,
            #[weak]
            drag_container,
            #[weak]
            dragged_item,
            #[weak]
            dragged_item_index,
            #[weak]
            dragging,
            move |_, start_x, start_y| if selection_mode.get().is_none()
                && Self::should_drag(start_x)
            {
                dragging.set_drag_state(true);

                // FIX: The cursor does not update until the mouse button is released
                // list_box.set_cursor_from_name(Some("grabbing"));

                #[cold]
                fn set_fallback_offsets(
                    drag_row: &ListRow,
                    drag_offset: &Rc<Cell<(f64, f64)>>,
                    pan_up_button_visible: bool,
                    list_box: &gtk::ListBox,
                    start_y: f64,
                ) {
                    drag_row.to_default();
                    drag_offset.set((
                        0.0,
                        (pan_up_button_visible as i32 * PAN_UP_BUTTON_HEIGHT
                            + list_box.parent().unwrap().margin_top()
                            + list_box.margin_top()) as f64
                            // NOTE: The below line has issues when built with Meson, where
                            // rows further down the list become more and more inaccurate
                            // (but `cargo build --features no-meson` works correctly)
                            // This code will likely never run, so fixing this may not be
                            // worth the effort
                            - start_y % ROW_HEIGHT as f64
                            - 4.0,
                    ));
                }

                if let Some(row) = list_box.row_at_y(start_y as i32) {
                    let row = row.downcast_ref::<ListRow>().unwrap();
                    let row_index = row.index();
                    dragged_item.set(Some(
                        (queue_page.list_model.get().unwrap().item(row_index as u32))
                            .and_downcast::<QueueItemObject>()
                            .unwrap()
                            .queue_item()
                            .clone(),
                    ));
                    dragged_item_index.set(queue_page.model_index_to_queue(row_index as usize));
                    drag_row.copy_from(row);
                    drag_row.set_width_request(row.width() + 2);
                    drag_row.set_height_request(row.height() + 2);
                    if row.has_css_class("heading") {
                        drag_row.add_css_class("heading");
                    } else {
                        drag_row.remove_css_class("heading");
                    }

                    if let Some(point) = drag_container.compute_point(
                        row,
                        &graphene::Point::new(
                            start_x as f32,
                            (start_y - scrolled_window.vadjustment().value()) as f32,
                        ),
                    ) {
                        drag_offset.set((-point.x() as f64 - 1.0, -point.y() as f64 - 1.0));
                    } else {
                        set_fallback_offsets(
                            &drag_row,
                            &drag_offset,
                            queue_page.view_further_up.is_visible(),
                            &list_box,
                            start_y,
                        );
                    }
                } else {
                    set_fallback_offsets(
                        &drag_row,
                        &drag_offset,
                        queue_page.view_further_up.is_visible(),
                        &list_box,
                        start_y,
                    );
                }

                let (drag_offset_x, drag_offset_y) = drag_offset.get();
                drag_widget.move_(
                    &drag_row,
                    start_x + drag_offset_x,
                    start_y + drag_offset_y - scrolled_window.vadjustment().value(),
                );

                drag_container.set_visible(true);
            }
        ));
        drag.connect_update(glib::clone!(
            #[weak(rename_to=queue_page)]
            self,
            #[weak]
            drag_row,
            #[strong]
            dragging,
            #[strong]
            drag_offset,
            move |gesture_drag, _| if dragging.get() {
                // TODO: Stop dragging when escape is pressed (`dragging.set_drag_state(false)`)

                let (Some((start_x, start_y)), Some((_, offset_y))) =
                    (gesture_drag.start_point(), gesture_drag.offset())
                else {
                    return;
                };

                // IDEA: Offset `start_y` so it points to the center of the dragged row

                if let Some(to_row_index) = (queue_page.list_box)
                    .row_at_y((start_y + offset_y) as i32)
                    .map(|row| row.index())
                {
                    let from_row_index = (queue_page.list_box)
                        .row_at_y(start_y as i32)
                        .map(|row| row.index())
                        .unwrap_or_default();
                    queue_page.for_each_row(|list_row, index| {
                        if to_row_index - 1 == index && to_row_index < from_row_index
                            || to_row_index == index && to_row_index > from_row_index
                        {
                            list_row.add_css_class("highlight-top");
                        } else {
                            list_row.remove_css_class("highlight-top");
                        }
                    });
                } else {
                    queue_page.for_each_row(|row, _| row.remove_css_class("highlight-top"));
                }

                let (drag_offset_x, drag_offset_y) = drag_offset.get();
                queue_page.drag_widget.move_(
                    &drag_row,
                    start_x + drag_offset_x,
                    start_y + drag_offset_y + offset_y
                        - queue_page.scrolled_window.vadjustment().value(),
                );
            }
        ));
        drag.connect_end(glib::clone!(
            #[weak(rename_to=queue_page)]
            self,
            #[weak]
            drag_row,
            #[weak]
            drag_container,
            #[strong]
            dragged_item,
            #[strong]
            dragged_item_index,
            #[strong]
            dragging,
            move |gesture_drag, _| if dragging.get() {
                queue_page.for_each_row(|row, _| row.remove_css_class("highlight-top"));
                drag_container.set_visible(false);
                dragging.set_drag_state(false);
                drag_row.to_default();

                let list_box = &queue_page.list_box;
                list_box.set_cursor(None);

                let start_y = match gesture_drag.start_point() {
                    Some((_, start_y)) => start_y + queue_page.list_box.margin_top() as f64,
                    _ => return,
                };
                let end_y = match gesture_drag.offset() {
                    Some((_, offset_y)) => start_y + offset_y,
                    None => return,
                };
                let mut from_index = dragged_item_index.get();
                let Ok(mut from) = queue_page
                    .queue_index_to_model(from_index)
                    .map(|index| index as i32)
                else {
                    return;
                };

                let playing_index = queue_page.playing_index.get();
                let mut index_updated = false;
                // `dragged_item` was set in `drag.connect_begin`
                let expected_item = dragged_item.take().unwrap();

                // If the queue item changed while dragging (such as when encountering a stopper),
                // find it by looping backwards. (There is currently no way to add items while
                // dragging, so looping backwards should suffice.)
                while let Some(target_item) =
                    (queue_page.list_model.get().unwrap().item(from as u32))
                        .and_downcast::<QueueItemObject>()
                    && *target_item.queue_item() != expected_item
                {
                    from -= 1;
                    index_updated = true;
                }
                if index_updated {
                    from_index = queue_page.model_index_to_queue(from as usize);
                }

                let Some(to) = list_box.row_at_y(end_y as i32).map(|row| row.index()) else {
                    return;
                };
                let to_index = queue_page.model_index_to_queue(to as usize);

                (player_tx().send(PlayerRequest::Reorder {
                    from: from_index,
                    to: to_index,
                }))
                .expect(EXP_RX);

                // Short queues don't need to be offset, even if wrapped items are shown
                if queue_page.queue_length.get() <= NUM_ITEMS_BEHIND + NUM_ITEMS_AHEAD {
                    return;
                }

                // If the item could not be found in the model, the value is -1
                // (`!0` (bitwise inverted 0) becomes -1 when cast to `i32`)
                let playing = (queue_page.queue_index_to_model(playing_index)).unwrap_or(!0) as i32;
                (queue_page.next_scroll_pos).set(QueueScrollAction::Offset(
                    match playing_index > NUM_ITEMS_BEHIND || queue_page.repeat_toggle.is_active() {
                        _ if playing == -1 => 0, // -1 means `playing` is out of view
                        false if from < playing && to > playing => 1,
                        true if from > playing && to <= playing => -1,
                        true if from < playing && to >= playing => 1,
                        true if from == playing => from_index as i32 - to_index as i32,
                        _ => 0,
                    },
                ));
            }
        ));
        self.list_box.add_controller(drag);
        let _ = self.drag_row.set(drag_row);
    }
    #[inline]
    fn setup_selection_mode(&self) {
        // IDEA: Rating dropdown button for rating multiple songs at once
        // TODO: Exit selection mode by pressing escape
        // FIX: Item selections will likely be lost with each queue update

        let selection_mode = Rc::clone(&self.selection_mode);
        let hold = gtk::GestureLongPress::new();
        hold.connect_pressed(glib::clone!(
            #[weak(rename_to=queue_page)]
            self,
            move |_, x, y| if selection_mode.get().is_none() && !Self::should_drag(x) {
                queue_page.set_selection_mode(Some(1));
                let object_index = queue_page.list_box.row_at_y(y as i32).unwrap().index();
                queue_page.queue_item_objects.borrow()[object_index as usize].set_selected(true);
            }
        ));
        self.list_box.add_controller(hold);
    }
    #[inline]
    fn setup_pan_repeat_on_hold(&self) {
        let hold_to_pan_up = gtk::GestureLongPress::new();
        hold_to_pan_up.connect_pressed(glib::clone!(
            #[weak(rename_to = queue_page)]
            self,
            move |_, _, _| queue_page.start_pan_loop(PanLoopDirection::Up)
        ));
        hold_to_pan_up.connect_end(glib::clone!(
            #[weak(rename_to = queue_page)]
            self,
            move |_, _| queue_page.stop_pan_loop()
        ));
        self.view_further_up.add_controller(hold_to_pan_up);

        let hold_to_pan_down = gtk::GestureLongPress::new();
        hold_to_pan_down.connect_pressed(glib::clone!(
            #[weak(rename_to = queue_page)]
            self,
            move |_, _, _| queue_page.start_pan_loop(PanLoopDirection::Down)
        ));
        hold_to_pan_down.connect_end(glib::clone!(
            #[weak(rename_to = queue_page)]
            self,
            move |_, _| queue_page.stop_pan_loop()
        ));
        self.view_further_down.add_controller(hold_to_pan_down);
    }
    #[inline]
    fn setup_reset_scroll_button_visibility(&self) {
        self.scrolled_window
            .vadjustment()
            .connect_value_changed(glib::clone!(
                #[weak(rename_to = queue_page)]
                self,
                move |vadjustment| {
                    queue_page.to_playing.set_visible(
                        // Only show the 'Scroll To Playing' button when the item is out of view
                        match queue_page.queue_index_to_model(queue_page.playing_index.get()) {
                            Err(_) => false,
                            Ok(index) => {
                                let scroll_pos = vadjustment.value() as usize;
                                let view_height = queue_page.scrolled_window.height() as usize;
                                let playing_item_pos = index * ROW_HEIGHT
                                    + queue_page.view_further_up.is_visible() as usize
                                        * PAN_UP_BUTTON_HEIGHT as usize;
                                !(scroll_pos..scroll_pos + view_height - ROW_HEIGHT)
                                    .contains(&playing_item_pos)
                            }
                        },
                    );
                }
            ));
    }

    /// Empties the list model, cancelling any pending background tasks during drop
    #[inline]
    pub fn uninit(&self) {
        self.list_model.get().expect(EXP_INIT).remove_all();
    }

    /// Used to verify that `model_index_to_queue` is working correctly
    #[inline]
    #[allow(unused)]
    #[cfg(debug_assertions)]
    fn model_index_to_queue_discrepancy_check(&self, model_index: usize, expected_index: usize) {
        match self.model_index_to_queue(model_index) {
            to_queue_index if to_queue_index != expected_index => {
                eprintln!("Discrepancy between `queue_index_to_model` and `model_index_to_queue`:");
                eprintln!("	`queue_index_to_model({expected_index})`:	{model_index}");
                eprintln!("	`model_index_to_queue({model_index})`:	{to_queue_index}");
            }
            _ => (),
        }
    }
    /// Used to verify that `queue_index_to_model` is working correctly
    #[inline]
    #[allow(unused)]
    #[cfg(debug_assertions)]
    fn queue_index_to_model_discrepancy_check(&self, queue_index: usize, expected_index: usize) {
        match self.queue_index_to_model(queue_index) {
            Ok(to_model_index) if to_model_index != expected_index => {
                eprintln!("Discrepancy between `queue_index_to_model` and `model_index_to_queue`:");
                eprintln!("	`model_index_to_queue({expected_index})`:	{queue_index}");
                eprintln!("	`queue_index_to_model({queue_index})`:	{to_model_index}");
            }
            Err(_) => eprintln!("`queue_index_to_model({queue_index})` returned an error"),
            _ => (),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for QueuePage {
    const NAME: &str = "MellowQueuePage";
    type Type = super::QueuePage;
    type ParentType = adw::NavigationPage;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
        class.bind_template_callbacks();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}
impl ObjectImpl for QueuePage {
    fn constructed(&self) {
        // Shuffle and repeat could be set manually here to ensure
        // the correct icons are used if they differ in the UI file
        // self.obj().update_shuffle(false);
        // self.obj().update_repeat(false);

        // FIX: Doesn't work when the queue is first loaded
        self.next_scroll_pos.set(QueueScrollAction::ToPlaying);

        self.setup_model();
        self.setup_drag_and_drop();
        self.setup_selection_mode();
        self.setup_pan_repeat_on_hold();
        self.setup_reset_scroll_button_visibility();
    }
}

impl WidgetImpl for QueuePage {}
impl NavigationPageImpl for QueuePage {}
