// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

mod chmod_dialog;
mod delete_dialog;
mod overwrite_dialog;
mod rename_dialog;
pub mod site_manager;

pub use chmod_dialog::*;
pub use delete_dialog::*;
pub use overwrite_dialog::*;
pub use rename_dialog::*;
