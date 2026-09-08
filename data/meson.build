# Execute `meson.build` files in these directories
# subdir('resources')

if host_machine.system() == 'linux'
  subdir('icons')

  config = configuration_data()
  config.set('APP_ID', application_id)
  config.set('BINDIR', bindir)

  configure_file(
    input: '@0@.desktop.in'.format(base_id),
    output: '@0@.desktop'.format(application_id),
    configuration: config,
    install: true,
    install_dir: datadir / 'applications',
  )
  configure_file(
    input: '@0@.gschema.xml.in'.format(base_id),
    output: '@0@.gschema.xml'.format(application_id),
    configuration: config,
    install: true,
    install_dir: datadir / 'glib-2.0' / 'schemas',
  )
  configure_file(
    input: '@0@.service.in'.format(base_id),
    output: '@0@.service'.format(application_id),
    configuration: config,
    install: true,
    install_dir: datadir / 'dbus-1' / 'services',
  )

  if get_option('flatpak').enabled()
    configure_file(
      input: '@0@.metainfo.xml.in'.format(base_id),
      output: '@0@.metainfo.xml'.format(application_id),
      configuration: config,
      install: true,
      install_dir: datadir / 'metainfo',
    )
  endif
endif
