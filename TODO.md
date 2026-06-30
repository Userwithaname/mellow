Song queue:

- [x] Reorder using drag-&-drop
  - TODO: Improvement: Scroll when reaching top/bottom edges
    - IDEA: Also pan if dragging onto the pan button, once panning is implemented
- [-] Multi-selection mode
  - IDEA: Shift+click to select everything between the last selected item and the clicked item
  - IDEA: Click+drag to select multiple items
  - [x] Removing multiple items at once
  - [ ] Rating multiple items at once
- [x] Display a landing page
> The "Open from Disk" picker could be improved to accept directories as well
- [x] Drag file/folder onto player to start a queue with them

Music library:

- IDEA: Allow initiating a full library rebuild

- [x] Save/load user settings and application state
  - IDEA: Remember filters?
  - IDEA: Remember if sort order was reversed(?)
- [x] Search/filtering for songs/albums/artists pages
- [x] Songs/albums/artists sort modes
- [-] Songs/albums/artists filtering
  - TODO: Add filters for artists as well
  - [ ] Filter by tags
- [x] Artists page
  - [x] Artist subpage, accessed from each item
    - IDEA: Display average rating
- [x] Albums page
  - [-] Album subpage, accessed from each item
    - TODO: Tag management (user-specified album tags (inferred from songs?))
- [x] Songs page
  - [-] Song subpage, accessed from each item
    - TODO: Tag management (user-specified song tags)
- [x] Play counting
> Works, but the counting logic could be improved

Other:

- [x] Toast notifications
  - IDEA: Notification for an upcoming "Pause & Close Player"
    (something like: "The player will close after this song")
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
