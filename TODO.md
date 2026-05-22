UI issues:

- `ListRow` (song queue):
  - FIX: Rows should have a fixed height, and not resize to fit the available space
  - FIX: The prefix image width should be the same regardless of the image aspect ratio
- `ItemTile` (albums page (artists page unaffected because it does not have images)):
  - FIX: In some cases, tile sizes can be inconsistent due to image sizing issues
  - FIX: Images with non-square aspect ratios look wrong and take up too much space
- `ItemRow` (songs page):
  - FIX: The prefix image width should be the same regardless of the image aspect ratio
- Song page:
  - FIX: Images can in some cases take up too much or too little space
  - FIX: Images with non-square aspect ratios look wrong

Song queue:

- [x] Add an indicator at the top and bottom of the list to show when there
      are more items in that direction
  - IDEA: The indicators could be buttons which offset the visible list by one
    in that direction (and a button to reset and jump back to the playing song)
- [x] Reorder using drag-&-drop
  - TODO: Improvement: Scroll when reaching top/bottom edges
    - IDEA: Also pan if dragging onto the pan button, once panning is implemented
- [-] Multi-selection mode
  - IDEA: Shift+click to select everything between the last selected item and the clicked item
  - [x] Removing multiple items at once
  - [ ] Rating multiple items at once
- [x] Offset the visible items by pressing the buttons at the top and bottom of the queue
  - [x] Holding the button should repeatedly increment the panning offset
  - [x] Show a scroll-to-playing button whenever the playing item is out of view
- [x] Display a landing page
> The "Open from Disk" picker could be improved to accept directories as well
- [x] Drag file/folder onto player to start a queue with them
  - TODO: Add visual feedback when the file is over the window
  > The bottom sheet could open automatically and focus the song
  queue, then show a "Drop here to start playing" message, or the
  window could show an overlay (like Amberol does)

Music library:

- [x] Save/load user settings and application state
  - IDEA: Remember filters?
  - IDEA: Remember if sort order was reversed(?)
- [ ] Allow initiating a full library rebuild
- [x] Search/filtering for songs/albums/artists pages
  - TODO: Improvement: Escape key should empty and unfocus the search field
- [x] Songs/albums/artists sort modes
- [ ] **Songs/albums/artists filtering**
  - TODO: Create a submenu in the sort dropdown for selecting filters
  - [ ] Filter by tags, ratings, year, etc
  - IDEA: Conditional filters: year < 2000, rating > 3, play-count = 0, etc
  > Note: may not work with the dropdown-submenu design
- [ ] Artists page
  - [x] Buttons to play all artists (shuffled/sequential)
  - [x] Show all artists
  - [-] Artist subpage, accessed from each item
    - [-] Display artist info (name/number of albums/average rating)
    - [x] Display all albums by the artist, each opening an Album subpage
    - [x] Play/Shuffle buttons
    - [x] Add to Queue button
- [ ] Albums page
  - [x] Buttons to play all albums (shuffled/sequential)
  - [x] Show all albums
  - [-] Album subpage, accessed from each item
    - [x] Display album info (album cover/title/artist/year/average rating)
    - [x] Play/Shuffle buttons
    - [x] Add to Queue button
    - [x] Display a track list, each opening a Song subpage
    - [x] Visually separate songs from different disks
    - [ ] Tag management (user-specified album tags (inferred from songs?))
    - [x] Go to artist button
- [ ] Songs page
  - [x] Buttons to play all songs (shuffled/sequential)
  - [x] Show all songs
  - [-] Song subpage, accessed from each item
    - [-] Display song info (title/album/artist, maybe album cover)
      - TODO: Needs design improvements
    - [x] Interactive rating widget
    - [x] Play Now: Start a new queue and skip to the selected track
    - [x] Play Next: Insert the song to the next position in the queue
    - [x] Add to Queue button
    - [ ] Tag management (user-specified song tags)
    - [x] Go to album button
    - [x] Go to artist button
- [x] Play counting
> Works, but the counting logic could be improved

Other:

- [x] Toast notifications
  - TODO: Notification for an upcoming "Pause & Close Player"
    (something like: "The player is scheduled to close shortly")
  > This could maybe show a 'Cancel' button to turn it into a regular stopper without closing

Meta:

- [ ] Offline build support
      https://docs.flathub.org/docs/for-app-authors/requirements#no-network-access-during-build
- [ ] Provide MetaInfo
      https://docs.flathub.org/docs/for-app-authors/metainfo-guidelines
- [ ] SVG icon which meets the Flathub quality guidelines
      (The current one might be okay if the shadow was removed, but it is poorly anti-aliased,
       and looks stylistically inconsistent when compared to other Gnome apps)
      https://docs.flathub.org/docs/for-app-authors/metainfo-guidelines/quality-guidelines#app-icon
- [ ] Decide on the brand colors
      https://docs.flathub.org/docs/for-app-authors/metainfo-guidelines/quality-guidelines#brand-colors

Ideas for improvements:

- Marquee long titles
- Ability to disable library directories(?)
> Disabled directories would still retain song data (play counts, etc),
> but be excluded from the actual `songs`/`albums`/`artists` used by the
> `Library` (design needed for enabling/disabling libraries)
- Main player:
  - Volume and lyrics could be accessed from the main player controls instead
    - The volume button could open a popup with the volume slider
      (like in Showtime (and many other video players))
    - Lyrics could be shown above the player controls when pressing the button
      (like in the original [mockup](mockup.md))
  - Display a hamburger menu on the opposite side of the close button:
    - Move the volume widget into the menu
    - Add a rating widget
    - Move the 'About' button into the menu
    - Could also move the settings, and make it a popup window, then something
      else can be moved into that overlay tab (maybe current file details/lyrics?)
- Queue page:
  - Show a track number as well?
- Song page:
  - The library song page and queue subpage could display more information
    about the song, such as track number, disc, year, duration, play count,
    format/sample rate, filename, etc.
  - An 'Open With' or 'Show on Disk' button (maybe a 'File Details' subpage?)
- Library:
  - The library could get rid of the home page, and instead switch the pages
    using a dropdown menu in the headerbar instead (in place of the back button)
  - The 'Go To Album/Artist' buttons could pop instead of pushing when the previous
    page is the same as the one that is about to open
- Files:
  - The library `songs` file could use a custom extension, which could make it possible to load the
    ratings/play counts from a different `songs` file and combine it with the current configuration.
    It is already possible to do so by appending the contents of a `songs` file from another system
    to the end of the local one and let the library resolve the conflicts during rebuild; this would
    make it possible to do so by simply opening the file.
  - The `queue` file could use a custom extension, so it can be loaded by opening the file
    (the `shuffled_queue` file would also require a `queue` file next to it)
  - All files could use the same extension and infer their contents based on the filename
    (`songs.…`, `queue.…`, or `shuffled_queue.…`), but this could limit their usability

People's requests:

- Larger play button on the album page
  - IDEA: Display a large play icon over the album artwork when hovering with the cursor
    (for touch devices, it could be shown on tap and disappear after some time of inactivity)
- Automatic files/folders organization using tags (should be disabled by default)
- Fetching artworks from the internet
- Integration with last.fm
- Tag editing
