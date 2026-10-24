Song queue:

- [x] Reorder using drag-&-drop
  - [x] Scroll when dragging items off-screen
  - [-] Pan when dragging items past the first/last visible item
    - FIX: Off-by-one when reordering off-screen items by more than half the queue length in repeat mode
- [-] Multi-selection mode
  - IDEA: Shift+click to select everything between the last selected item and the clicked item
  - IDEA: Long-press+drag to select multiple items
  - IDEA: Allow reordering multiple items at once
  - [ ] Allow rating multiple items at once
- [x] Display a landing page
> The "Open from Disk" picker could be improved to accept directories as well
- [x] Drag file/folder onto player to start a queue with them
> Could use some visual improvements
- IDEA: Keep stoppers at the same relative position in the queue when toggling shuffle mode
  (for example, 5 songs ahead)

Music library:

- IDEA: It might be possible to reduce UI code repetition for songs/albums/artists using proc macros

- [x] Save/load user settings and application state
  - IDEA: Remember filters?
  - IDEA: Remember if sort order was reversed(?)
- [x] User-assigned custom tags
  - IDEA: Keyboard navigation for renaming: make the entire tag widget focusable, and rename using F2
  - IDEA: Choose which tag suggestion is active using the arrow keys, without un-focusing the entry box
- [ ] Multi-selection for artists/albums/songs pages
  - [ ] Same design/behavior as for queue multi-selection (and maintain consistency going forward)
  - [ ] Play button should only play the selected items
  - [ ] Allow rating multiple items at once
- [x] Artists page
  - [x] Artist subpage, accessed from each item
    - IDEA: Display average rating
- [x] Albums page
  - [x] Album subpage, accessed from each item
- [x] Songs page
  - [x] Song subpage, accessed from each item
- [ ] Directories page (for browsing by the library directory trees)
  - [ ] The top level page should show the configured library directories
  - [ ] Clicking on a (sub)directory should descend into that directory and show its contents
  - [ ] Clicking on a song should open the relevant song page
  - [ ] Play all songs within any current directory recursively by pressing the play button in the header
  - IDEA: An 'Open With' button for opening files/directories externally

General:

- [ ] Overlay redesign
  - [ ] Move the overlay to a sidebar, and either:
    - Add a button to the main player for opening the overlay and remove the bottom bar, or
    - Use repsonsive design to switch between a sidebar and bottom sheet based on
      window width, and keep the overlay on the side if the window is wide enough
      so no buttons need to be added (and use the current design if not)
  - [ ] Move the settings to a popup window, accessed from a new menu in the overlay header
    - [ ] Apply library directory changes as a single task when the settings window is closed
    - IDEA: Add buttons for exporting/importing `songs` files (ratings/play counts/etc)
      - IDEA: Allow automatically importing from (and exporting to?) configured `songs` files

Other:

- [ ] Bundle icons instead of relying on system ones
- [x] Toast notifications

Requests:

- Larger play button on the album page
  - IDEA: Display a large play icon over the album artwork when hovering with the cursor
    (for touch devices, it could be shown on tap and disappear after some time of inactivity)
- Automatic file/folder organization using tags (note: should be disabled by default)
- Song file tag editing
- Networking
  - Fetching artworks from the internet
  - Fetching lyrics from the internet
  - Integration with last.fm

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
- Lyrics:
  - Extended LRC format support (Walaoke & A2 - <https://en.wikipedia.org/wiki/LRC_(file_format)>)
    - Even if this will not be supported, it might make sense to (for example) remove the A2 word
      timestamps so they don't show up in the lyrics
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

Meta:

- [x] Provide MetaInfo: <https://docs.flathub.org/docs/for-app-authors/metainfo-guidelines>
- [ ] Comply with Flathub quality guidelines: <https://docs.flathub.org/docs/for-app-authors/metainfo-guidelines/quality-guidelines>
  - TODO: Create a compliant SVG icon: <https://docs.flathub.org/docs/for-app-authors/metainfo-guidelines/quality-guidelines#app-icon>
  - TODO: Use unedited screenshots: <https://docs.flathub.org/docs/for-app-authors/metainfo-guidelines/quality-guidelines#just-the-app-window>
- [ ] Offline build support: <https://docs.flathub.org/docs/for-app-authors/requirements#no-network-access-during-build>
> Go through everything again in detail before submitting (<https://docs.flathub.org/docs/category/for-app-authors>)
