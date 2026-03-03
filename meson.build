project('cargo-app', 'rust',
  version: '0.6.1',
  meson_version: '>= 1.1.0',
  license: 'GPL-2.0-only',
)

i18n = import('i18n')
gnome = import('gnome')

base_id = 'me.rueegger.cargo'

dependency('glib-2.0', version: '>= 2.86')
dependency('gio-2.0', version: '>= 2.86')
dependency('gtk4', version: '>= 4.20')
dependency('libadwaita-1', version: '>= 1.8')

profile = get_option('profile')
if profile == 'release'
  application_id = base_id
else
  application_id = base_id
endif

version = meson.project_version()

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
localedir = prefix / get_option('localedir')
datadir = prefix / get_option('datadir')
pkgdatadir = datadir / meson.project_name()

conf = configuration_data()
conf.set_quoted('APP_ID', application_id)
conf.set_quoted('VERSION', version)
conf.set_quoted('GETTEXT_PACKAGE', meson.project_name())
conf.set_quoted('LOCALEDIR', localedir)
conf.set_quoted('PKGDATADIR', pkgdatadir)

subdir('data')
subdir('src')
subdir('po')

gnome.post_install(
  glib_compile_schemas: true,
  gtk_update_icon_cache: true,
  update_desktop_database: true,
)
