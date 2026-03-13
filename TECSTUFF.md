# Technical Documentation — Cargo

## Project Overview

- **App ID:** `me.rueegger.cargo`
- **Binary:** `cargo-app`
- **Display Name:** Cargo
- **Language:** Rust
- **GUI Framework:** GTK4 + libadwaita 1.8.x
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

### Context Menus (PopoverMenu)

When adding a `PopoverMenu` via `set_parent()` on a widget (e.g. `ColumnView`), you **must**:

1. Store the `PopoverMenu` in the parent widget's `imp` struct (e.g. `RefCell<Option<gtk::PopoverMenu>>`)
2. Call `popover.unparent()` in the `dispose()` method of the parent widget

Without this, GTK will emit warnings on shutdown:
`Finalizing GtkColumnView, but it still has children left: GtkPopoverMenu`

See `CargoWindow` for the reference implementation.

### Async Model

- **Tokio Runtime**: Static runtime for network I/O (suppaftp, russh)
- **async-channel**: Communication from Tokio threads to GTK main thread
- **glib::spawn_future_local**: UI-safe async operations on the GTK main loop
- GTK widgets are NEVER accessed from Tokio threads

### Password Storage (Keyring)

Passwords are stored securely in the system keyring via `libsecret` (GNOME Keyring / Secret Service API).

- **Schema**: `me.rueegger.cargo.Password` with a `site-id` attribute
- **Storage**: Passwords are stored/retrieved per site profile ID
- **Site Manager**: Password field is auto-populated from keyring when selecting a site
- **Sidebar Connect**: If a password exists in the keyring, connects directly without prompting
- **Deletion**: Passwords are removed from the keyring when a site is deleted
- **Flatpak**: Requires `--talk-name=org.freedesktop.secrets` in finish-args

Passwords are never written to `sites.json` on disk.

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
- Meson >= 1.1
- GTK4 development libraries >= 4.20
- libadwaita development libraries >= 1.8
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

### Flatpak Build

```bash
# Generate cargo-sources.json first
python3 flatpak-cargo-generator.py Cargo.lock -o cargo-sources.json

# Build
flatpak-builder --user --install --force-clean _flatpak me.rueegger.cargo.yml

# Run
flatpak run me.rueegger.cargo
```

### Releasing

Use the release script to bump the version across all files:

```bash
./scripts/release.sh <version> "<changelog description>"
```

Example:
```bash
./scripts/release.sh 0.5.0 "Add sidebar with quick-connect and refresh button"
```

The script updates: `Cargo.toml`, `meson.build`, `po/en.po`, `po/de.po`,
and adds a new `<release>` entry to `data/me.rueegger.cargo.metainfo.xml.in`
with today's date. Review changes with `git diff` before committing.

### Publishing

Use the publish script from the flatpak repository:

```bash
cd ~/Projects/flatpak.rueegger.dev
./publish.sh cargo
```

## GitHub Pages

The project has a product landing page served via GitHub Pages from the `docs/` directory on `main`.

- **URL:** https://srueegger.github.io/me.rueegger.cargo/
- **Source:** `docs/index.html` (single-file, no build step)
- **Screenshots:** `docs/screenshots/` (copies of `screenshots/`)
- **Favicon:** `docs/favicon.svg` (copy of app icon)
- **Languages:** Automatic detection (DE/EN) via JavaScript, same approach as flatpak.rueegger.dev

When updating screenshots, remember to copy them into `docs/screenshots/` as well.

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
│   ├── style.css           # Application stylesheet
│   ├── me.rueegger.cargo.desktop.in
│   ├── me.rueegger.cargo.metainfo.xml.in
│   ├── me.rueegger.cargo.gschema.xml
│   ├── me.rueegger.cargo.service.in
│   ├── icons/
│   │   ├── me.rueegger.cargo.svg
│   │   ├── me.rueegger.cargo-symbolic.svg
│   │   └── scalable/actions/       # Custom symbolic icons
│   │       ├── cargo-ftp-symbolic.svg
│   │       ├── cargo-ftps-symbolic.svg
│   │       └── cargo-sftp-symbolic.svg
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
    ├── utils.rs            # Shared utilities (format_size, etc.)
    ├── connection.rs       # Async connection handle (tokio ↔ GTK bridge)
    ├── file_item.rs        # GObject wrapper for file list entries
    ├── file_panel.rs       # Dual-pane file browser panel widget
    ├── site_manager.rs     # Site profile persistence (JSON)
    ├── keyring.rs          # Password storage via libsecret (system keyring)
    │
    ├── window/             # Main application window
    │   ├── mod.rs          # imp struct, TemplateChild fields, core setup
    │   ├── sidebar.rs      # Sidebar with saved connections
    │   ├── connection.rs   # Connect/disconnect logic
    │   ├── transfers.rs    # Transfer buttons, queue UI, enqueue logic
    │   ├── file_operations.rs  # Rename, delete, chmod
    │   ├── drag_drop.rs    # Drag-and-drop between panels
    │   ├── context_menus.rs    # Right-click context menus
    │   └── sync_nav.rs     # Synchronized navigation
    │
    ├── transfer/           # Transfer system
    │   ├── mod.rs          # Re-exports
    │   ├── item.rs         # GObject wrapper for transfer queue entries
    │   └── queue.rs        # Transfer queue logic and conflict handling
    │
    ├── dialogs/
    │   ├── mod.rs
    │   ├── chmod_dialog.rs
    │   ├── delete_dialog.rs
    │   ├── overwrite_dialog.rs
    │   ├── rename_dialog.rs
    │   └── site_manager/   # Site manager dialog
    │       ├── mod.rs      # imp struct, signals, coordination
    │       ├── form.rs     # Form population, saving, field handlers
    │       └── actions.rs  # CRUD, connect, file browser dialogs
    │
    └── protocol/
        ├── mod.rs          # Protocol trait + types + factory
        ├── error.rs        # Unified error types
        ├── ftp.rs          # FTP/FTPS via suppaftp
        └── sftp.rs         # SFTP via russh + russh-sftp
```

### Module Splitting Convention

Large widgets are split into a directory with submodules. Each submodule uses
multiple `impl` blocks on the same type:

```rust
// In window/transfers.rs
use gtk::{glib, prelude::*, subclass::prelude::*};
use super::CargoWindow;

impl CargoWindow {
    pub(crate) fn on_upload_clicked(&self) { /* ... */ }
}
```

Key rules:
- `mod.rs` contains the `mod imp { ... }` block with all `TemplateChild` fields
- Submodules import `subclass::prelude::*` for access to `.imp()`
- Submodules import `adw::prelude::*` when using libadwaita methods
- Visibility: `pub(crate)` for methods called from other modules, `pub(super)` for internal helpers

### Custom Icons

Protocol-specific icons are bundled as GResources in `data/icons/scalable/actions/`.
They follow the `cargo-*-symbolic.svg` naming convention and are registered via
`icon_theme.add_resource_path()` in `application.rs`.

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
| libsecret | 0.7 | GNOME Keyring integration (password storage) |
| thiserror | 2.0 | Derive macro for error types |
| log | 0.4 | Logging facade |
