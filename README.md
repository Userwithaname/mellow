<h1>
<p align="center">
  <img height=128 src="data/icons/io.github.userwithaname.Mellow.png">
  <br>Mellow
</h1>
  <p align="center">
    <img height=512 src="https://github.com/user-attachments/assets/2d76d5d3-a9b8-451d-920d-68d4b217a1b5">
  </p>
  <p align="center">
    Listen to music without distraction
  </p>
  <p align="center">
    <a href="https://github.com/Userwithaname/mellow/releases/">Releases</a> |
    <a href="#about">About</a> |
    <a href="#features">Features</a> |
    <a href="#installing">Installing</a> |
    <a href="#uninstalling">Uninstalling</a> |
    <a href="BUILDING.md">Building</a>
  </p>
</p>

---

# About

Mellow is an experimental music player, which strives for an immersive listening
experience by minimizing distractions. Non-crucial elements of the interface are
hidden away, bringing music to the spotlight.

Unlike most players, Mellow puts the currently playing song at the base of the
interface. This means that there is no "back" button on the main player, which
might be tempting to press. Rather, everything that is unrelated to the currently
playing song is done inside an overlay, hidden from view except when needed.

This divides the interface into two parts; the main player (the "now"), and the
overlay (the "not now"). The main player features the currently playing song and
player controls, and the overlay is used for everything else; browsing the library
to find what to play, editing the song queue to choose what plays next, setting the
shuffle and repeat modes, and configuring the application.
When the overlay is closed, it is time to enjoy the music.

# Features

- **Sleek and minimal interface**: Less for the eyes, more for the ears
- **Adaptive colors**: Interface colors adapt to match the current artwork
- **Song queue**: Edit the list of playing songs, or schedule a pause
- **Music library**: Browse and play your local music collection
- **Custom tags**: Categorize your music to make it easier to find what to listen to
- **Gapless playback**: Enjoy stutter-free transitions between songs
- **Lyrics**: Displays synchronized or unsynchronized song lyrics (if present locally)
- **File discovery**: Reorganize your library without worry of losing your data
- **Removable drives**: Unavailable libraries will not lose their library data
- **Fast and lightweight**: Responsive and quick to start, even with large libraries

# Installing

The recommended way to install Mellow on Linux is by downloading it from the
[releases page](https://github.com/Userwithaname/mellow/releases). It can be
installed by opening the Flatpak file in Gnome Software (or similar), or using
the `flatpak` command from the terminal:

```bash
# Note: Ensure the file path and architecture is correct before running
# x86_64:
flatpak install --user ~/Downloads/io.github.userwithaname.Mellow.x86_64.flatpak
# aarch64:
flatpak install --user ~/Downloads/io.github.userwithaname.Mellow.aarch64.flatpak
```

For building Mellow from source, see [BUILDING.rs](BUILDING.rs)

> [!TIP]
> By building from source, it may be possible to run Mellow on other platforms

> [!TIP]
> Existing configurations are expected to work with all future stable versions of Mellow, but it
> is recommended to backup your configuration if you plan on trying older versions or commits

# Uninstalling

If you've installed Mellow using the Flatpak release asset and wish to remove it,
you can either do so through your distribution's software manager, or by using the
`flatpak` command from the terminal:

```bash
flatpak uninstall io.github.userwithaname.Mellow
```

If you've installed Mellow by building it from source, it can be uninstalled by
manually removing the files listed at the bottom of [BUILDING.md](BUILDING.md)
