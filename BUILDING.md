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
> Mellow may also be built using [Cargo](https://doc.rust-lang.org/cargo/commands/cargo-build.html).
> Note that this will require manually [installing the GSchema](https://gtk-rs.org/gtk4-rs/stable/latest/book/settings.html),
> setting up icons, and creating the application shortcut. Building with Meson
> is recommended for a simpler build process.

### Recommended GStreamer plugins (may be required to play music):

```bash
dnf install gstreamer1-plugins-bad-free gstreamer1-plugins-bad-free-extras gstreamer1-plugins-good gstreamer1-plugins-good-extras gstreamer1-plugin-libav
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
