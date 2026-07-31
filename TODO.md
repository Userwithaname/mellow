Song queue:

- [x] Reorder using drag-&-drop
  - TODO: Improvement: Scroll when reaching top/bottom edges
    - IDEA: Also pan if dragging onto the pan button, once panning is implemented
- [-] Multi-selection mode
  - IDEA: Shift+click to select everything between the last selected item and the clicked item
  - IDEA: Long-press+drag to select multiple items
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
- [x] Songs/albums/artists filtering
  - [x] Filter by user-assigned tags
    - IDEA: Add a button to select/deselect all toggles
    - IDEA: Add an "untagged" toggle for showing only items with no tags
- [x] User-assigned custom tags
  - [x] Show suggestions when adding tags
    - [x] It should be possible to assign pre-existing tags without needing to type
    - [x] Typing should only be necessary when creating a new tag
    - [x] While typing, filter existing tags and confirm to add the best candidate
  - IDEA: Choose which tag suggestion is active using the arrow keys, without un-focusing the entry box
  - IDEA: Ability to rename existing tags
- [x] Artists page
  - [x] Artist subpage, accessed from each item
    - IDEA: Display average rating
- [x] Albums page
  - [x] Album subpage, accessed from each item
- [x] Songs page
  - [x] Song subpage, accessed from each item
- [x] Play counting

Other:

- [ ] Redesign the library directory settings
  > For example, opening a separate "Directory Settings" popup window and requiring the user
  > to apply all changes at once could work around the Flatpak freezing issue ([#21](https://github.com/Userwithaname/mellow/issues/21))
  > and also present an opportunity to add "Import"/"Export" buttons to more easily merge
  > library data from different systems
- [ ] Bundle icons instead of relying on system ones
- [x] Toast notifications
  - IDEA: Notification for an upcoming "Pause & Close Player"
    (something like: "The player will close after this song")
  > This could maybe show a 'Cancel' button to turn it into a regular stopper without closing

People's requests (for consideration):

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

GitHub:

- Cleanup README & create a wiki page
  - Move installation instructions to the wiki
  - Add link shortcuts (see the Rust's README for reference: <https://github.com/rust-lang/rust/blob/main/README.md?plain=1>)
