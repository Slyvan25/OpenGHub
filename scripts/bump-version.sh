#!/bin/sh
# Sets the app version in the three places it lives and refreshes Cargo.lock.
#   scripts/bump-version.sh 0.2.0
set -e
v="$1"
case "$v" in
    [0-9]*.[0-9]*.[0-9]*) ;;
    *) echo "usage: $0 X.Y.Z" >&2; exit 1 ;;
esac
cd "$(dirname "$0")/.."
sed -i "s/^  \"version\": \".*\",/  \"version\": \"$v\",/" package.json src-tauri/tauri.conf.json
sed -i "s/^version = \".*\"/version = \"$v\"/" src-tauri/Cargo.toml
sed -i "s/^pkgver=.*/pkgver=$v/" packaging/arch/PKGBUILD
(cd src-tauri && cargo update -p openghub --offline >/dev/null 2>&1 || cargo update -p openghub >/dev/null)
npm install --package-lock-only --ignore-scripts >/dev/null 2>&1 || true
git add package.json package-lock.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock packaging/arch/PKGBUILD
echo "version $v set — now: git commit -m \"release: v$v\" && git tag -a v$v -m \"OpenGHub $v\" && git push origin HEAD v$v"
