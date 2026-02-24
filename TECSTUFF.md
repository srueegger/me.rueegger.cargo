# Technical Documentation — Cargo

## Project Overview

- **App ID:** `me.rueegger.cargo`
- **Binary:** `cargo-app`
- **Display Name:** Cargo
- **Language:** Rust
- **GUI Framework:** GTK4 + libadwaita 1.7.x
- **Build System:** Meson + Cargo
- **Distribution:** Flatpak (GNOME 49 Runtime)
- **License:** GPL-2.0-only

## Architecture

### Build System

The project uses Meson as the primary build system, which delegates to Cargo for Rust compilation.

- `meson.build` (root): Project definition, dependency checks, subdirectories
- `src/meson.build`: Invokes `cargo build` with appropriate profile flags
- `data/meson.build`: Processes desktop files, metainfo, GSchema, GResources
- `build.rs`: Compiles GResources before Rust compilation

### Environment Variables (Build Time)

These are set by Meson and consumed by `src/config.rs` via `env!()`:

- `APP_ID`: Application identifier (`me.rueegger.cargo`)
- `GETTEXT_PACKAGE`: Gettext domain (`cargo-app`)
- `LOCALEDIR`: Path to locale files
- `PKGDATADIR`: Path to installed data files

### GObject Subclassing Pattern

All GTK widgets use the standard gtk-rs subclassing pattern:

1. Inner `mod imp` with `#[glib::object_subclass]`
2. `glib::wrapper!` macro for the public type
3. `CompositeTemplate` for UI-bound widgets

### Async Model

- **Tokio Runtime**: Static runtime for network I/O (suppaftp, russh)
- **async-channel**: Communication from Tokio threads to GTK main thread
- **glib::spawn_future_local**: UI-safe async operations on the GTK main loop
- GTK widgets are NEVER accessed from Tokio threads

### Protocol Abstraction

New protocols can be added by implementing the `Protocol` trait:

```rust
#[async_trait]
pub trait Protocol: Send + Sync {
    async fn connect(&mut self, config: &ConnectionConfig) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn list_dir(&self, path: &str) -> Result<Vec<RemoteEntry>>;
    // ... additional methods
}
```

## Development

### Prerequisites

- Rust stable toolchain
- Meson >= 0.59
- GTK4 development libraries >= 4.16
- libadwaita development libraries >= 1.6
- gettext tools

### Building

```bash
# Debug build
meson setup build
meson compile -C build

# Release build
meson setup build-release -Dprofile=release
meson compile -C build-release
```

### Running

```bash
# After meson build (sets up GSchema etc.)
./build/target/debug/cargo-app

# Or via meson
meson devenv -C build cargo-app
```

### Flatpak Build (in Distrobox)

```bash
# Generate cargo-sources.json first
python3 flatpak-cargo-generator.py Cargo.lock -o cargo-sources.json

# Build
flatpak-builder --user --install --force-clean _flatpak me.rueegger.cargo.yml

# Run
flatpak run me.rueegger.cargo
```

### Publishing

Use the publish script from the flatpak repository:

```bash
cd ~/Projects/flatpak.rueegger.dev
./publish.sh cargo
```

## i18n / Translations

**Every user-visible string must be translatable.** This is a hard requirement.

- Primary language: English (EN)
- Translations: German (DE)
- DE_AT and DE_CH fall back to `de.po` via standard gettext behavior
- `.po` files are in `po/`, listed in `LINGUAS`
- `POTFILES` lists all files containing translatable strings

### Rules for new code

1. **UI files (`.ui`)**: Every user-visible string must have `translatable="yes"`
2. **Rust source**: Wrap all user-visible strings with `gettext()` from `gettextrs`
   - Simple strings: `gettext("Disconnect")`
   - Format strings: `gettext("Error: %s").replace("%s", &value)` (use `%s` placeholder)
   - Plurals: `ngettext("%u item", "%u items", n).replace("%u", &n.to_string())`
3. **Desktop/metainfo files**: Handled automatically by `i18n.merge_file()` in meson
4. **POTFILES**: Add any new source file that contains `gettext()` calls
5. **Both `.po` files**: Add `msgid`/`msgstr` entries for every new string in `de.po` and `en.po`

### What NOT to translate

- Log messages (`log::error!`, `log::warn!`, etc.)
- Technical identifiers (icon names, CSS classes, GSettings keys)
- Library names in credits (proper nouns)

## Git Workflow

- Each feature is developed on its own branch
- Branches are merged to `main` and then deleted
- Commit messages are in English, conventional style
- No references to AI tools in commits

## File Structure

```
me.rueegger.cargo/
├── Cargo.toml              # Rust package manifest
├── build.rs                # GResource compilation
├── meson.build             # Root Meson config
├── meson.options           # Build profile option
├── me.rueegger.cargo.yml   # Flatpak manifest
├── data/
│   ├── meson.build
│   ├── cargo-app.gresource.xml
│   ├── me.rueegger.cargo.desktop.in
│   ├── me.rueegger.cargo.metainfo.xml.in
│   ├── me.rueegger.cargo.gschema.xml
│   ├── me.rueegger.cargo.service.in
│   ├── icons/
│   │   ├── me.rueegger.cargo.svg
│   │   └── me.rueegger.cargo-symbolic.svg
│   └── ui/
│       ├── window.ui
│       ├── file_panel.ui
│       └── site_manager_dialog.ui
├── po/
│   ├── meson.build
│   ├── POTFILES
│   ├── LINGUAS
│   ├── de.po
│   └── en.po
└── src/
    ├── meson.build
    ├── main.rs
    ├── config.rs
    ├── application.rs
    ├── window.rs
    └── protocol/
        ├── mod.rs          # Protocol trait + types + factory
        ├── error.rs        # Unified error types
        ├── ftp.rs          # FTP/FTPS via suppaftp
        └── sftp.rs         # SFTP via russh + russh-sftp
```

## Dependency Policy

**Always use the latest stable versions of all dependencies.** When adding or updating
a dependency, check for the newest release on crates.io and use that version. This
ensures we get the latest bug fixes, security patches, and performance improvements.

Periodically audit `Cargo.toml` and update any outdated dependencies.

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| gtk4 | 0.10 | GTK4 Rust bindings |
| libadwaita | 0.8 | Adwaita widget library (wraps C libadwaita 1.8.x) |
| glib | 0.21 | GLib Rust bindings |
| gio | 0.21 | GIO Rust bindings |
| gettext-rs | 0.7 | Internationalization |
| suppaftp | 8.0.2 | FTP/FTPS client (with tokio-rustls-aws-lc-rs) |
| russh | 0.57 | SSH client (pure Rust) |
| russh-keys | 0.49 | SSH key handling and agent support |
| russh-sftp | 2.1 | SFTP over SSH |
| tokio | 1.49 | Async runtime |
| async-channel | 2.5 | Cross-thread communication |
| async-trait | 0.1 | Async trait support |
| thiserror | 2.0 | Derive macro for error types |
| log | 0.4 | Logging facade |
