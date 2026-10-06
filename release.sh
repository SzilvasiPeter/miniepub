#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "Usage: $0 <core|cli|tui> <major|minor|patch|X.Y.Z>"
    echo "  member   workspace member to release (core, cli, tui)"
    echo "  major    bump major version (0.1.0 -> 1.0.0)"
    echo "  minor    bump minor version (0.1.0 -> 0.2.0)"
    echo "  patch    bump patch version (0.1.0 -> 0.1.1)"
    echo "  X.Y.Z    set explicit version"
    exit 1
}

[[ $# -ne 2 ]] && usage

member=$1
bump=$2
package=$(cargo get --entry "./$member/" package.name)

case "$bump" in
    major|minor|patch) cargo set-version --package "$package" --bump "$bump" ;;
    *) cargo set-version --package "$package" "$bump" ;;
esac

# Refresh Cargo.lock with the new version and verify the build.
cargo check

version=$(cargo get --entry "./$member/" package.version)
branch=$(git branch --show-current)
tag="$package-v$version"

git add -- Cargo.lock */Cargo.toml
git commit -m "release: $tag"
git tag -a "$tag" -m "$tag"
git push origin "$branch" --tags

echo "Released $tag"
