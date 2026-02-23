desktop_file = i18n.merge_file(
  input: 'me.rueegger.cargo.desktop.in',
  output: 'me.rueegger.cargo.desktop',
  type: 'desktop',
  po_dir: '../po',
  install: true,
  install_dir: datadir / 'applications',
)

desktop_utils = find_program('desktop-file-validate', required: false)
if desktop_utils.found()
  test('Validate desktop file', desktop_utils,
    args: [desktop_file],
  )
endif

metainfo_file = i18n.merge_file(
  input: 'me.rueegger.cargo.metainfo.xml.in',
  output: 'me.rueegger.cargo.metainfo.xml',
  po_dir: '../po',
  install: true,
  install_dir: datadir / 'metainfo',
)

appstreamcli = find_program('appstreamcli', required: false)
if appstreamcli.found()
  test('Validate appstream file', appstreamcli,
    args: ['validate', '--no-net', '--explain', metainfo_file],
  )
endif

install_data(
  'me.rueegger.cargo.gschema.xml',
  install_dir: datadir / 'glib-2.0' / 'schemas',
)

compile_schemas = find_program('glib-compile-schemas', required: false)
if compile_schemas.found()
  test('Validate schema file',
    compile_schemas,
    args: ['--strict', '--dry-run', meson.current_source_dir()],
  )
endif

service_conf = configuration_data()
service_conf.set('APP_ID', application_id)
service_conf.set('BINDIR', bindir)

configure_file(
  input: 'me.rueegger.cargo.service.in',
  output: 'me.rueegger.cargo.service',
  configuration: service_conf,
  install: true,
  install_dir: datadir / 'dbus-1' / 'services',
)

resources = gnome.compile_resources(
  'cargo-app',
  'cargo-app.gresource.xml',
  gresource_bundle: true,
  source_dir: meson.current_source_dir(),
  install: true,
  install_dir: pkgdatadir,
)

subdir('icons')
