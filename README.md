# Cargo

A dual-pane file transfer application for GNOME, built with GTK4 and libadwaita.

## Features

- **Dual-pane file browser** — Browse local and remote files side by side
- **FTP/FTPS support** — Connect to FTP servers with optional TLS encryption
- **SFTP support** — Secure file transfer over SSH with key, password, or agent authentication
- **Site Manager** — Save and organize your server connections
- **Transfer Queue** — Visual progress with pause, resume, and cancel controls per transfer
- **Synchronized Navigation** — Optionally keep both panels in sync when changing directories
- **GNOME Integration** — Follows GNOME Human Interface Guidelines with native libadwaita look

## Installation

### From Flatpak Repository

```bash
# Add the repository
flatpak remote-add --if-not-exists rueegger-dev https://flatpak.rueegger.dev/rueegger-dev.flatpakrepo

# Install Cargo
flatpak install rueegger-dev me.rueegger.cargo
```

### Building from Source

Requirements:
- Rust (stable toolchain)
- Meson >= 0.59
- GTK4 >= 4.16
- libadwaita >= 1.6
- gettext

```bash
meson setup build
meson compile -C build
meson install -C build
```

## License

Cargo is licensed under the [GNU General Public License v2.0](LICENSE).

## Author

Samuel Rüegger — [samuel@rueegger](mailto:samuel@rueegger)
