project('mellow', 'rust', version: '0.4.1', meson_version: '>= 1.4')

dependency('gtk4', version: '>= 4.22', required: get_option('flatpak'))
dependency('libadwaita-1', version: '>= 1.9', required: get_option('flatpak'))
dependency('gstreamer-1.0', version: '>= 1.26', required: get_option('flatpak'))
dependency('gstreamer-plugins-base-1.0', version: '>= 1.26', required: get_option('flatpak'))
dependency('gstreamer-plugins-bad-1.0', version: '>= 1.26', required: get_option('flatpak'))

gnome = import('gnome')

base_id = 'io.github.userwithaname.Mellow'

if get_option('profile') == 'devel'
  profile = 'Devel'
  application_id = '@0@.@1@'.format(base_id, profile)
else
  profile = ''
  application_id = base_id
endif

bindir = get_option('prefix') / get_option('bindir')
localedir = get_option('prefix') / get_option('localedir')
datadir = get_option('prefix') / get_option('datadir')
pkgdatadir = datadir / meson.project_name()

# Execute `meson.build` files in these directories
subdir('data')
subdir('src')

gnome.post_install(
  glib_compile_schemas: true,
  gtk_update_icon_cache: true,
  update_desktop_database: true,
)
