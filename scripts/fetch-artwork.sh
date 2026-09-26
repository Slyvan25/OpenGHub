#!/usr/bin/env bash
#
# Downloads Logitech's product renders for the devices attached to THIS machine,
# into this user's OpenGHub data directory.
#
#   ./scripts/fetch-artwork.sh              # every connected Logitech device
#   ./scripts/fetch-artwork.sh c08d 407f    # specific product ids
#   ./scripts/fetch-artwork.sh c08d https://…/some-render.png   # explicit URL
#
# The images stay on your machine and are never redistributed with OpenGHub:
# they remain Logitech's copyright. This script only automates what you could do
# by right-clicking the image on logitechg.com and saving it.
#
# Nothing is downloaded for a device that is not in the table below — drop a PNG
# named <product-id>.png into the target directory by hand instead.
#
# Per-zone lighting masks are not downloadable: G HUB fetches them per device
# from its own depot service, keyed by `render_icon_key` in the device schema.
# If you have them, name them <product-id>-zone<N>.png alongside the base render
# and OpenGHub will use the alpha channel as a mask.

set -euo pipefail

# Must match artwork::dir() in src-tauri/src/artwork.rs — lowercase.
DEST="${OPENGHUB_ARTWORK_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/openghub/devices}"
CDN="https://resource.logitechg.com"
# Cloudinary transform: cap the width, keep transparency, force PNG.
TRANSFORM="w_1200,c_limit,q_auto,f_png/d_transparent.gif"

# product id -> path under /content/dam/gaming/en/products/
declare -A ART=(
  [c08d]="g502-lightspeed-gaming-mouse/g502-lightspeed-gallery-1.png"
  [407f]="g502-lightspeed-gaming-mouse/g502-lightspeed-gallery-1.png"
  [c08b]="g502-hero-gaming-mouse/g502-hero-gallery-1.png"
  [c097]="g502-x-gaming-mouse/g502-x-black-gallery-1.png"
  [c098]="g502-x-plus-gaming-mouse/g502-x-plus-black-gallery-1.png"
  [c095]="g502x-plus/2026-update/gallery/g502-x-plus-top-angle-black-gallery-1.png"
  [4099]="g502x-plus/2026-update/gallery/g502-x-plus-top-angle-black-gallery-1.png"
  [4074]="g305-lightspeed-wireless-gaming-mouse/g305-gallery-black-1.png"
  [c090]="g203-lightsync-gaming-mouse/g203-lightsync-gallery-black-1.png"
  [c092]="g102-lightsync-gaming-mouse/g102-lightsync-gallery-black-1.png"
  [c094]="pro-x-superlight-wireless-mouse/pro-x-superlight-black-gallery-1.png"
  [c09b]="pro-x-superlight-2/pro-x-superlight-2-black-gallery-1.png"
  [c088]="pro-wireless-gaming-mouse/pro-wireless-gallery-1.png"
  [c086]="g903-lightspeed-gaming-mouse/g903-hero-gallery-1.png"
  [c087]="g703-lightspeed-gaming-mouse/g703-hero-gallery-1.png"
  [c33f]="g815-rgb-mechanical-gaming-keyboard/g815-gallery-1.png"
  [407c]="g915/g915-gallery-1.png"
  [c33e]="g915/g915-gallery-1.png"
  [c545]="g915-tkl/g915-tkl-gallery-1.png"
  [c339]="pro-mechanical-gaming-keyboard/pro-keyboard-gallery-1.png"
  [c343]="pro-x-tkl-gaming-keyboard/pro-x-tkl-black-gallery-1.png"
  [4097]="pro-x-60-gaming-keyboard/pro-x-60-black-gallery-1.png"
  [c336]="g213-prodigy-rgb-gaming-keyboard/g213-gallery-1.png"
  [0ab5]="g733-lightspeed-wireless-rgb-gaming-headset/g733-black-gallery-1.png"
  [0afe]="pro-x-2-lightspeed-wireless-gaming-headset/pro-x-2-black-gallery-1.png"
  [0a87]="g935-7-1-surround-sound-lightsync-gaming-headset/g935-gallery-1.png"
  [c901]="litra-beam/litra-beam-gallery-1.png"
  [c900]="litra-glow/litra-glow-gallery-1.png"
)

mkdir -p "$DEST"

download() {
  local id="$1" url="$2" out="$DEST/$1.png"
  printf '  %-6s ' "$id"
  if curl -sfL --max-time 30 -o "$out.part" "$url"; then
    # A CDN miss returns an HTML error page with a 200, so check it is an image.
    if head -c4 "$out.part" | grep -q $'\x89PNG'; then
      mv "$out.part" "$out"
      echo "OK  -> $out"
      return 0
    fi
    echo "FAILED (not a PNG — the path is probably wrong)"
  else
    echo "FAILED (download error)"
  fi
  rm -f "$out.part"
  return 1
}

# Explicit "<id> <url>" pair.
if [[ $# -eq 2 && "$2" == http* ]]; then
  echo "Saving to $DEST"
  download "$1" "$2"
  exit $?
fi

if [[ $# -gt 0 ]]; then
  ids=("$@")
else
  # Read HID-level product ids from sysfs rather than lsusb: a device behind a
  # Unifying/LIGHTSPEED receiver reports the *receiver's* id on the USB bus, but
  # its own id at the HID layer — which is the one the artwork is keyed on.
  mapfile -t ids < <(
    cat /sys/class/hidraw/hidraw*/device/uevent 2>/dev/null |
      grep -oiE '^HID_ID=[0-9a-f]+:0+046D:0+([0-9A-F]{4})' |
      sed -E 's/.*:0+([0-9A-F]{4})$/\1/' |
      tr 'A-Z' 'a-z' | sort -u
  )
  if [[ ${#ids[@]} -eq 0 ]]; then
    echo "No Logitech devices found." >&2
    echo "Pass product ids explicitly, e.g. $0 c08d" >&2
    exit 1
  fi
  echo "Found ${#ids[@]} connected Logitech device(s): ${ids[*]}"
fi

echo "Saving to $DEST"
missing=()
for id in "${ids[@]}"; do
  id="${id,,}"
  if [[ -n "${ART[$id]:-}" ]]; then
    download "$id" "$CDN/$TRANSFORM/content/dam/gaming/en/products/${ART[$id]}" || missing+=("$id")
  else
    printf '  %-6s not in the table\n' "$id"
    missing+=("$id")
  fi
done

if [[ ${#missing[@]} -gt 0 ]]; then
  cat <<EOF

No image for: ${missing[*]}
Find the product on logitechg.com, right-click its render, and save it as
  $DEST/<product-id>.png
Then add the path to the ART table in this script so it works next time.
EOF
fi
