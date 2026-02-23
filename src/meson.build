cargo_bin = find_program('cargo')

cargo_options = ['--manifest-path', meson.project_source_root() / 'Cargo.toml']
cargo_options += ['--target-dir', meson.project_build_root() / 'target']

if profile == 'release'
  cargo_options += ['--release']
  rust_target = 'release'
else
  rust_target = 'debug'
endif

cargo_env = environment()
cargo_env.set('APP_ID', application_id)
cargo_env.set('GETTEXT_PACKAGE', meson.project_name())
cargo_env.set('LOCALEDIR', localedir)
cargo_env.set('PKGDATADIR', pkgdatadir)

custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: meson.project_name(),
  console: true,
  install: true,
  install_dir: bindir,
  depends: resources,
  env: cargo_env,
  command: [
    'sh', '-c',
    cargo_bin.full_path() + ' build ' + ' '.join(cargo_options) +
    ' && cp ' + meson.project_build_root() / 'target' / rust_target / meson.project_name() + ' @OUTPUT@',
  ],
)
