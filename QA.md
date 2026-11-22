# Quality Assurance

1. Run `cargo test` and ensure everything passes
2. Uncheck all boxes and re-test to ensure the following:

Playback:

- [x] Pause/play/skip work as expected
- [x] Shuffle/repeat/sequential modes work as expected
- [x] Seeking works as expected
  - [x] Seeking to any point in the song (click or drag)
  - [x] Seeking to the end and releasing the seek bar
  - [x] Seeking to the end and back
- [x] Gapless/non-gapless playback works as expected

Song Queue:

- [x] Loading or starting new queues works as expected
- [x] Adding items works as expected
- [x] Removing items works as expected
- [x] Removal undo works as expected
- [ ] Reordering the queue works as expected
  - FIX: Off-by-one when reordering off-screen items by more than half the queue length in repeat mode
- [x] Selection mode works as expected
  - [x] Removing multiple items at once works as expected
- [x] Stoppers work and behave as expected

Music Library:

- [x] The 'Songs' page and its subpages work as expected
- [x] The 'Albums' page and its subpages work as expected
- [x] The 'Artists' page and its subpages work as expected
- [x] Library building works in the background and doesn't affect functionality
- [x] Searching is quick and works as expected
- [x] Sort modes work as expected
- [x] Filtering works as expected

User Experience:

- [x] The interface is responsive as soon as launched, without delays
  - [x] With existing library
  - [x] On fresh launch
- [x] All actions respond to user input without delay
- [x] All actions provide visual feedback
  - NOTE: Animations/transitions would improve the overall experience
- [x] Lengthy tasks display a progress bar without blocking the interface
- [x] All settings load properly (test with non-default values)
- [ ] Does not leak memory
  - FIX: Memory leaks related to thumbnails and thumbnail creation (#25)
- [x] Non-fatal errors are handled gracefully
- [x] Crash errors are presented to the user
- [x] No other issues found while testing

Design Consistency:

- [x] Similar looking elements work the same an all places
