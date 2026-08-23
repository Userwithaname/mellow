cargo = find_program('cargo')
cargo_options = ['--manifest-path', meson.project_source_root() / 'Cargo.toml']
cargo_options += ['--target-dir', meson.project_build_root() / 'target']

if get_option('profile') == 'devel'
  rust_target = 'debug'
elif get_option('profile') == 'bench'
  environment().append('RUSTFLAGS', '-C force-frame-pointers=yes')
  cargo_options += ['--profile=bench']
  cargo_options += ['--features=verbose-logs']
  rust_target = 'release'
else
  cargo_options += ['--release']
  rust_target = 'release'
endif

custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: meson.project_name(),
  console: true,
  install: true,
  install_dir: bindir,
  # depends: resources,
  env: {
    'CARGO_HOME': meson.project_build_root() / 'cargo-home',
    'APP_ID': application_id,
    'APP_VERSION': meson.project_version(),
    'RESOURCES_FILE': pkgdatadir / 'resources.gresource',
  },
  command: [
    cargo,
    '--version',
    '&&',
    cargo,
    'build',
    cargo_options,
    '&&',
    'cp',
    meson.project_build_root() / 'target' / rust_target / meson.project_name(),
    '@OUTPUT@',
  ]
)
