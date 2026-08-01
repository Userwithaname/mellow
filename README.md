<h1>
<p align="center">
  <img height=128 src="data/icons/io.github.userwithaname.Mellow.png">
  <br>Mellow
</h1>
  <p align="center">
    Listen to music without distraction
  </p>
  <p align="center">
    <img height=512 src="https://github.com/user-attachments/assets/2d76d5d3-a9b8-451d-920d-68d4b217a1b5">
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
- **Gapless playback**: Enjoy stutter-free transitions between songs
- **Song queue**: View and edit the list of playing songs, or schedule a pause
- **Music library**: Browse and play your local music collection
- **Custom tags**: Categorize your music to make it easier to find what to listen to
- **File discovery**: Reorganize your library without worry of losing your data
- **Removable drives**: Unavailable libraries will not lose their library data
- **Fast and lightweight**: Responsive and quick to start, even with large libraries

---

> [!NOTE]
> This software is in active development; features and design may still be subject
> to change. If you encounter any problems while using it, please post about them
> on the [issues page](https://github.com/Userwithaname/mellow/issues).

> [!WARNING]
> Forwards compatibility is not guaranteed; loading a newer configuration using older
> versions or commits could potentially result in your library data being lost.
> Please backup your `songs` file before doing so, if this is important to you. If the
> old version is unable to launch, the Mellow GSchema settings may need to be reset
> (for example, using `gsettings reset-recursively io.github.userwithaname.Mellow`).

# Installing Mellow

> [!TIP]
> Only Linux builds are currently supported. If you would like to try Mellow on
> a different operating system, it may be possible by building it from source.

The recommended way to install Mellow is by downloading it from the
[releases page](https://github.com/Userwithaname/mellow/releases).
It can be installed by opening the Flatpak file in Gnome Software
(or similar), or using the `flatpak` command from the terminal:

```bash
# Note: Ensure the file path and architecture is correct before running
# x86_64:
flatpak install --user ~/Downloads/io.github.userwithaname.Mellow.x86_64.flatpak
# aarch64:
flatpak install --user ~/Downloads/io.github.userwithaname.Mellow.aarch64.flatpak
```

# Uninstalling

If you've installed Mellow using the Flatpak release asset and wish to remove it,
you can either do so through your distribution's software manager, or by using the
`flatpak` command from the terminal:

```bash
flatpak uninstall io.github.userwithaname.Mellow
```

If you've installed Mellow by building it from source, it can be uninstalled by
manually removing the files listed at the bottom of this document.

# Building from source

> [!NOTE]
> The below instructions are meant for Fedora;
> steps may be different for other systems

## Step 1: Installing dependencies

### [Rust & Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### [GStreamer](https://gstreamer.freedesktop.org/documentation/installing/on-linux.html), [GTK](https://gtk-rs.org/gtk4-rs/stable/latest/book/project_setup.html), [Libadwaita](https://gtk-rs.org/gtk4-rs/stable/latest/book/libadwaita.html), and [Meson](https://mesonbuild.com/SimpleStart.html#installing-meson):

```bash
dnf install gstreamer1-devel gtk4-devel libadwaita-devel meson
```

> [!TIP]
> Mellow may also be built using [Cargo](https://doc.rust-lang.org/cargo/commands/cargo-build.html)
> directly by adding `--features no-meson`. Note that this will require manually
> [installing the GSchema](https://gtk-rs.org/gtk4-rs/stable/latest/book/settings.html),
> setting up icons, and creating the application shortcut. Building with Meson
> is recommended for a simpler build process.

### Recommended plugins (not required for building):

```bash
dnf install \
  gstreamer1-plugins-bad-free \
  gstreamer1-plugins-bad-free-extras \
  gstreamer1-plugins-good \
  gstreamer1-plugins-good-extras \
  gstreamer1-plugin-libav
```

## Step 2: Building and installing

### [Build using Meson](https://gtk-rs.org/gtk4-rs/stable/latest/book/meson.html#building-and-running):

Clone the source code and run the following command to build and
install Mellow on your system:

```bash
meson setup builddir --prefix=~/.local && meson install -C builddir
```

The following files and directories will be created:
```
~
├── .cache
│   └── mellow ⟵╮
│       └── …  ⟵┤
├── .config     ├─ Created when launched
│   └── mellow ⟵┤
│       └── …  ⟵╯
└── .local
    ├── bin
    │   └── mellow ⟵─ Main program executable
    └── share
        ├── applications
        │   └── io.github.userwithaname.Mellow.desktop
        ├── dbus-1
        │   └── services
        │       └── io.github.userwithaname.Mellow.service
        ├── glib-2.0
        │   └── schemas
        │       ├── io.github.userwithaname.Mellow.gschema.xml
        │       └── gschemas.compiled ⟵╮
        │           Note: May also contain schemas for other apps
        ├── icons
        │   └── hicolor
        │       └── scalable
        │           └── apps
        │               └── io.github.userwithaname.Mellow.png
        └── mellow
            └── resources.gresource
```

> [!TIP]
> Ensure the `mellow` executable is within your `$PATH` for the shortcut to
> work correctly. If you've used a different build command, the executable
> might be in a different location than shown above.
