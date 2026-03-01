#!/usr/bin/env bash
set -euo pipefail

# Release script for Cargo (me.rueegger.cargo)
# Usage: ./scripts/release.sh <version> "<changelog description>"
# Example: ./scripts/release.sh 0.5.0 "Add sidebar with quick-connect and refresh button"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# --- Argument validation ---

if [[ $# -lt 2 ]]; then
    echo "Usage: $0 <version> \"<changelog description>\""
    echo "Example: $0 0.5.0 \"Add sidebar with quick-connect and refresh button\""
    exit 1
fi

NEW_VERSION="$1"
DESCRIPTION="$2"

if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Error: Version must be in format X.Y.Z (e.g. 0.5.0)"
    exit 1
fi

if [[ -z "$DESCRIPTION" ]]; then
    echo "Error: Changelog description cannot be empty"
    exit 1
fi

# --- Read current version ---

CURRENT_VERSION=$(grep '^version = ' "$REPO_ROOT/Cargo.toml" | head -1 | sed 's/version = "\(.*\)"/\1/')

if [[ -z "$CURRENT_VERSION" ]]; then
    echo "Error: Could not read current version from Cargo.toml"
    exit 1
fi

if [[ "$CURRENT_VERSION" == "$NEW_VERSION" ]]; then
    echo "Error: New version ($NEW_VERSION) is the same as current version ($CURRENT_VERSION)"
    exit 1
fi

TODAY=$(date +%Y-%m-%d)

echo "Bumping version: $CURRENT_VERSION → $NEW_VERSION"
echo "Date: $TODAY"
echo "Changelog: $DESCRIPTION"
echo ""

# --- Update version in files ---

# Cargo.toml
sed -i "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" "$REPO_ROOT/Cargo.toml"
echo "  Updated Cargo.toml"

# meson.build
sed -i "s/version: '$CURRENT_VERSION'/version: '$NEW_VERSION'/" "$REPO_ROOT/meson.build"
echo "  Updated meson.build"

# po/en.po
sed -i "s/Project-Id-Version: cargo-app $CURRENT_VERSION/Project-Id-Version: cargo-app $NEW_VERSION/" "$REPO_ROOT/po/en.po"
echo "  Updated po/en.po"

# po/de.po
sed -i "s/Project-Id-Version: cargo-app $CURRENT_VERSION/Project-Id-Version: cargo-app $NEW_VERSION/" "$REPO_ROOT/po/de.po"
echo "  Updated po/de.po"

# metainfo.xml.in — insert new release before existing releases
RELEASE_BLOCK="    <release version=\"$NEW_VERSION\" date=\"$TODAY\">\n      <description>\n        <p>$DESCRIPTION<\/p>\n      <\/description>\n    <\/release>"
sed -i "s|  <releases>|  <releases>\n$RELEASE_BLOCK|" "$REPO_ROOT/data/me.rueegger.cargo.metainfo.xml.in"
echo "  Updated data/me.rueegger.cargo.metainfo.xml.in"

echo ""
echo "Done! Version bumped from $CURRENT_VERSION to $NEW_VERSION."
echo "Review the changes with 'git diff', then commit when ready."
