# Quality Assurance

1. Run `cargo test` and ensure everything passes
2. Uncheck all boxes and re-test to ensure the following:

Playback:

- [x] Pause/play/skip work as expected
- [x] Shuffle/repeat/sequential modes work as expected
- [x] Seeking works as expected
  - BUG: (Flatpak is unaffected) Playback error with certain files when seeking to
    the beginning of the song:
    `gst_base_parse_finish_frame: assertion 'size > 0 || frame->out_buffer' failed`
  - [x] Seeking to any point in the song (click or drag)
  - [x] Seeking to the end and releasing the seek bar
  - [x] Seeking to the end and back
- [x] Gapless/non-gapless playback works as expected
- [x] Non-fatal errors are handled gracefully

Song Queue:

- [x] Starting a new queue works as expected
- [x] Adding items works as expected
- [x] Removing items works as expected
- [x] Removal undo works as expected
- [x] Reordering the queue works as expected
  - TODO: Improvement: Scroll when dragging close to the view borders
    - IDEA: Also pan if dragging onto the pan button, once panning is implemented
- [x] Selection mode works as expected
  - [x] Removing multiple items at once works as expected
- [x] Stoppers work and behave as expected
  - IDEA: Improvement: Stoppers could stay at the same relative position in the queue
    when toggling shuffle mode (for example, 5 songs ahead)
- [x] The landing page is shown for empty queues and works without issues

Music Library:

- [x] The 'Songs' page and its subpages work as expected
- [x] The 'Albums' page and its subpages work as expected
- [x] The 'Artists' page and its subpages work as expected
- [x] Library building works in the background and doesn't affect functionality
- [x] Searching is quick and works as expected
  - BUG: (Flatpak is unaffected) Items sometimes don't show up until scrolling after searching
- [x] Sort modes work as expected
- [x] Filtering works as expected
  - BUG: (Flatpak is unaffected) Items sometimes don't show up until scrolling after changing filters

User Experience:

- [x] The interface is responsive as soon as launched, without delays
  - [x] With existing library
  - [x] On fresh launch
- [x] All actions respond to user input without delay
- [x] All actions provide visual feedback
- [x] Lengthy tasks display a progress bar without blocking the interface
- [x] All settings load properly (test with non-default values)
- [ ] Does not leak memory
  - FIX: Memory leaks related to thumbnails and thumbnail creation (#25)
- [x] No other issues found while testing

Design Consistency:

- [x] Similar looking elements work the same an all places
