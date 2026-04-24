use adw::{prelude::*, subclass::prelude::*};
use core::cell::{Cell, OnceCell, RefCell};
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

#[derive(Default, CompositeTemplate)]
#[template(resource = "/io/github/userwithaname/Mellow/queue_page.ui")]
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
    pub selection_toggle: TemplateChild<gtk::ToggleButton>,
    #[template_child]
    pub remove_selection: TemplateChild<gtk::Button>,

    pub selection_mode: Rc<Cell<bool>>,

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

    pub subpage: OnceCell<QueueSubpage>,

    view_pan_offset: Cell<isize>,
    queue_item_objects: Rc<RefCell<Vec<QueueItemObject>>>,
    list_model: OnceCell<gio::ListStore>,
    pub drag_row: OnceCell<ListRow>,
    pub next_scroll_pos: Cell<QueueScrollAction>,

    pub song_queue: RefCell<Box<[QueueItem]>>,
    pub playing_index: Cell<usize>,
    queue_length: Cell<usize>,
    last_repeat_mode: Cell<bool>,
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
        self.set_selection_mode(false);
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
                Box::new(move || {
                    let _ = player_tx().send(PlayerRequest::Undo);
                }),
            ))),
        )))
        .expect(EXP_RX);

        let _ = player_tx().send(PlayerRequest::RemoveItems(selected_items));
        self.set_selection_mode(false);
    }
    #[template_callback]
    pub fn handle_pan_up(&self) {
        // TODO: Handle infinite panning in repeat mode
        self.view_pan_offset.set(self.view_pan_offset.get() - 1);
    }
    #[template_callback]
    pub fn handle_pan_down(&self) {
        // TODO: Handle infinite panning in repeat mode
        self.view_pan_offset.set(self.view_pan_offset.get() + 1);
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
            self.scroll_to_pos(
                (model_index * ROW_HEIGHT) as f64
                    + (self.view_further_up.is_visible() as i32 * PAN_UP_BUTTON_HEIGHT
                        // NOTE: For some reason, `margin_top` only has
                        // to be accounted for when building with Meson
                        - self.list_box.margin_top()) as f64,
            );

            #[cfg(debug_assertions)]
            self.model_index_to_queue_discrepancy_check(model_index, index);
        }
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
        self.set_selection_mode(false);

        // TODO: Use `self.view_pan_offset` to offset the visible item range
        // TODO: Add a button to jump back to the playing item
        // TODO: Decide when/whether to reset the pan offset

        let start = playing.saturating_sub(NUM_ITEMS_BEHIND);
        let end = (playing + NUM_ITEMS_AHEAD).min(queue.len());

        let mut items = Self::items_to_objects(
            queue.iter().take(end).skip(start).enumerate(),
            playing,
            start,
        );

        let repeat_mode = self.repeat_toggle.is_active();

        let last_repeat_mode = self.last_repeat_mode.replace(repeat_mode);
        if repeat_mode && queue_length != 0 {
            let n_items_before = (NUM_ITEMS_BEHIND - (playing - start)).min(queue_length - 1);
            if n_items_before > 0 {
                if repeat_mode != last_repeat_mode {
                    self.next_scroll_pos.set(QueueScrollAction::Offset(
                        n_items_before as i32, //
                    ));
                }
                let start = (queue.len() - n_items_before).max(playing + 1);
                let items_before = Self::items_to_objects(
                    queue.iter().skip(start).enumerate(),
                    playing, //
                    start,
                );
                items = [items_before, items].concat();
            }
            let n_items_after = NUM_ITEMS_AHEAD - (end - playing);
            if n_items_after > 0 {
                let items_after = Self::items_to_objects(
                    queue.iter().take(n_items_after.min(playing)).enumerate(),
                    playing,
                    0,
                );
                items.extend(items_after);
            }
        } else if repeat_mode != last_repeat_mode {
            self.next_scroll_pos.set(QueueScrollAction::Offset(
                -(NUM_ITEMS_BEHIND.saturating_sub(playing) as i32),
            ));
        }

        let list_model = self.list_model.get().unwrap();
        list_model.splice(0, list_model.n_items(), &items);
        self.queue_item_objects.replace(items);
        self.playing_index.set(playing);

        let last_up_button_visible = self.view_further_up.is_visible();
        let up_button_visible = repeat_mode || playing > NUM_ITEMS_BEHIND;
        self.view_further_up.set_visible(up_button_visible);
        self.view_further_down.set_visible(
            repeat_mode || queue_length - playing > NUM_ITEMS_AHEAD, //
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
    #[must_use]
    fn items_to_objects<I, 'i>(
        items_iter: I,
        playing_index: usize,
        start_index: usize,
    ) -> Vec<QueueItemObject>
    where
        I: Iterator<Item = (usize, &'i QueueItem)>,
    {
        items_iter
            .map(|index_item| {
                let q_index = index_item.0 + start_index;
                QueueItemObject::new(
                    q_index as u32,
                    q_index == playing_index,
                    index_item.1.clone(),
                )
            })
            .collect()
    }

    /// Takes a queue item index and returns the index to access its object
    ///
    /// # Panics
    /// Panics if `self.queue_item_objects` `RefCell` is mutably borrowed
    #[inline]
    fn queue_index_to_model(&self, queue_index: usize) -> Result<usize, ItemNotFoundError> {
        let queue_items_len = self.queue_item_objects.borrow().len();
        let playing_index = self.playing_index.get();
        match self.repeat_toggle.is_active() {
            false => {
                let start = playing_index.saturating_sub(NUM_ITEMS_BEHIND);
                if queue_index < start {
                    return Err(ItemNotFoundError);
                }
                match queue_index + NUM_ITEMS_BEHIND.min(playing_index) - playing_index {
                    value if value >= queue_items_len => Err(ItemNotFoundError),
                    value => Ok(value),
                }
            }
            true => {
                let queue_length = self.queue_length.get();
                if queue_length == 0 {
                    return Err(ItemNotFoundError);
                }

                let start = playing_index.saturating_sub(NUM_ITEMS_BEHIND);
                let n_items_before = NUM_ITEMS_BEHIND
                    .min(queue_length - 1 - start)
                    .saturating_sub(playing_index - start);

                // Wrapping over the start of the queue
                if n_items_before > 0 && queue_index > playing_index + NUM_ITEMS_AHEAD {
                    let from = queue_length - n_items_before;
                    match queue_index - from {
                        value if value >= queue_items_len => return Err(ItemNotFoundError),
                        value => return Ok(value),
                    };
                }

                // Non-wrapped items
                if let Some(value) = (queue_index + NUM_ITEMS_BEHIND.min(playing_index))
                    .checked_sub(playing_index)
                    .map(|i| i + n_items_before)
                    && value < queue_items_len
                {
                    return Ok(value);
                }

                // Wrapping over the end of the queue
                let n_items_after = queue_length - playing_index.saturating_sub(NUM_ITEMS_AHEAD);
                if queue_index <= n_items_after {
                    match queue_index + n_items_after {
                        value if value >= queue_items_len => return Err(ItemNotFoundError),
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
        let playing_index = self.playing_index.get();
        match self.repeat_toggle.is_active() {
            false => model_index + playing_index - NUM_ITEMS_BEHIND.min(playing_index),
            true => {
                let queue_length = self.queue_length.get();
                debug_assert!(
                    queue_length != 0,
                    "`model_index_to_queue` used on an empty queue"
                );

                // Wrapping over the start of the queue
                let n_items_behind = NUM_ITEMS_BEHIND
                    .min(queue_length - 1)
                    .saturating_sub(playing_index);
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
                let offset_index = model_index + playing_index
                    - NUM_ITEMS_BEHIND.min(playing_index)
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
    fn set_selection_mode(&self, selection_mode: bool) {
        self.selection_toggle.set_active(selection_mode);
        self.header_selection.set_visible(selection_mode);
        self.header_normal.set_visible(!selection_mode);
        self.selection_mode.set(selection_mode);
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
                        queue_row.set_image_margins(5);
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
                #[weak]
                queue_item_object,
                move |_| match selection_mode.get() {
                    false => (ui_tx().send(UpdateUI::OpenQueueSubpage(index))).expect(EXP_RX),
                    true => queue_item_object.set_selected(!queue_item_object.selected()),
                }
            ));

            queue_row.upcast::<gtk::Widget>()
        });

        let _ = self.list_model.set(model);
    }
    /// Returns `true` if interaction at a given `start_pos_x`
    /// should drag the queue row, or `false` if not
    #[inline]
    fn should_drag(start_pos_x: f64) -> bool {
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
        let drag_offset_x = Rc::new(Cell::new(0.0));
        let drag_offset_y = Rc::new(Cell::new(0.0));

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
            drag_offset_x,
            #[weak]
            drag_offset_y,
            #[weak]
            drag_container,
            #[weak]
            dragged_item,
            #[weak]
            dragged_item_index,
            #[weak]
            dragging,
            move |_, start_x, start_y| if !selection_mode.get() {
                if !Self::should_drag(start_x) {
                    return;
                }

                dragging.set_drag_state(true);

                // FIX: The cursor does not update until the mouse button is released
                list_box.set_cursor_from_name(Some("grabbing"));

                fn set_fallback_offsets(
                    drag_row: &ListRow,
                    drag_offset_x: &Rc<Cell<f64>>,
                    drag_offset_y: &Rc<Cell<f64>>,
                    pan_up_button_visible: bool,
                    list_box: &gtk::ListBox,
                    start_y: f64,
                ) {
                    drag_row.to_default();
                    drag_offset_x.set(0.0);
                    drag_offset_y.set(
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
                    );
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
                    if let Some(point) = drag_container.compute_point(
                        row,
                        &graphene::Point::new(
                            start_x as f32,
                            (start_y - scrolled_window.vadjustment().value()) as f32,
                        ),
                    ) {
                        drag_offset_x.set(-point.x() as f64 - 1.0);
                        drag_offset_y.set(-point.y() as f64 - 1.0);
                    } else {
                        set_fallback_offsets(
                            &drag_row,
                            &drag_offset_x,
                            &drag_offset_y,
                            queue_page.view_further_up.is_visible(),
                            &list_box,
                            start_y,
                        );
                    }
                } else {
                    set_fallback_offsets(
                        &drag_row,
                        &drag_offset_x,
                        &drag_offset_y,
                        queue_page.view_further_up.is_visible(),
                        &list_box,
                        start_y,
                    );
                }

                drag_widget.move_(
                    &drag_row,
                    start_x + drag_offset_x.get(),
                    start_y + drag_offset_y.get() - scrolled_window.vadjustment().value(),
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
            drag_offset_x,
            #[strong]
            drag_offset_y,
            move |gesture_drag, _| if dragging.get() {
                // TODO: Cancel drag when pressing escape or right mouse button
                // (using `dragging.set_drag_state(false)`)

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

                queue_page.drag_widget.move_(
                    &drag_row,
                    start_x + drag_offset_x.get(),
                    start_y + drag_offset_y.get() + offset_y
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
                let expected_item =
                    dragged_item.take().unwrap( /* `dragged_item` was set in `drag.connect_begin` */ );

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

                let playing = (queue_page.queue_index_to_model(playing_index)).unwrap() as i32;
                (queue_page.next_scroll_pos).set(QueueScrollAction::Offset(
                    match playing_index > NUM_ITEMS_BEHIND || queue_page.repeat_toggle.is_active() {
                        false if from < playing && to > playing => 1,
                        true if from > playing && to <= playing => -1,
                        true if from < playing && to >= playing => 1,
                        true if from == playing => from - to,
                        _ => 0,
                    },
                ));
                (player_tx().send(PlayerRequest::Shift(from_index, (to - from) as isize)))
                    .expect(EXP_RX);
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
            move |_, x, y| if !selection_mode.get() && !Self::should_drag(x) {
                queue_page.set_selection_mode(true);
                let object_index = queue_page.list_box.row_at_y(y as i32).unwrap().index();
                queue_page.queue_item_objects.borrow()[object_index as usize].set_selected(true);
            }
        ));
        self.list_box.add_controller(hold);
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
    }
}

impl WidgetImpl for QueuePage {}
impl NavigationPageImpl for QueuePage {}
