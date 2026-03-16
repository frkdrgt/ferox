#!/usr/bin/env bash
# Usage: ./scripts/release.sh 0.2.0
# Creates a git tag and pushes it — triggers GitHub Actions release build.

set -euo pipefail

VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
    echo "Usage: $0 <version>   e.g. $0 0.2.0"
    exit 1
fi

TAG="v$VERSION"

# Update version in Cargo.toml
sed -i "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml

echo "→ Updated Cargo.toml to $VERSION"

# Commit version bump
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to $VERSION"

# Tag and push
git tag "$TAG"
git push origin HEAD
git push origin "$TAG"

echo ""
echo "✓ Tag $TAG pushed."
echo "  GitHub Actions will now build Windows .exe and macOS .dmg."
echo "  Check: https://github.com/$(git remote get-url origin | sed 's|.*github.com[:/]||' | sed 's|\.git$||')/releases/tag/$TAG"
