#!/bin/sh
# Writes the manifest to submit to Flathub: the same build as the local one,
# but taking the .deb from the GitHub release by URL + sha256 (Flathub builds
# have no local files and no network beyond declared sources), with
# x-checker-data so Flathub's bot proposes future versions by itself.
#
#   packaging/flatpak/make-flathub-manifest.sh 0.1.1 [path/to/OpenGHub_0.1.1_amd64.deb]
#
# Prints the manifest on stdout.
set -e
v="$1"
deb="$2"
[ -n "$v" ] || { echo "usage: $0 <version> [deb]" >&2; exit 1; }
here=$(dirname "$0")
repo=https://github.com/Slyvan25/OpenGHub

if [ -n "$deb" ]; then
    sha=$(sha256sum "$deb" | cut -d' ' -f1)
else
    tmp=$(mktemp)
    curl -sSL -o "$tmp" "$repo/releases/download/v$v/OpenGHub_${v}_amd64.deb"
    sha=$(sha256sum "$tmp" | cut -d' ' -f1)
    rm -f "$tmp"
fi

# Everything above `sources:` is shared with the local manifest.
sed '/^ *sources:/,$d' "$here/io.github.slyvan25.OpenGHub.yml"
cat <<EOF
    sources:
      - type: file
        url: $repo/releases/download/v$v/OpenGHub_${v}_amd64.deb
        sha256: $sha
        x-checker-data:
          type: json
          url: https://api.github.com/repos/Slyvan25/OpenGHub/releases/latest
          version-query: .tag_name | sub("^v"; "")
          url-query: .assets[] | select(.name | test("OpenGHub_.*_amd64\\\\.deb\$")) | .browser_download_url
      - type: file
        path: io.github.slyvan25.OpenGHub.metainfo.xml
EOF
