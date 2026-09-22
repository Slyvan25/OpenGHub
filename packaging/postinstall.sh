#!/bin/sh
# Runs after the .deb / .rpm is installed: make the udev rule take effect so
# the app can open Logitech devices without a reboot or replug.
set -e
if command -v udevadm >/dev/null 2>&1; then
    udevadm control --reload-rules || true
    udevadm trigger --action=add --subsystem-match=hidraw || true
    udevadm trigger --action=add --subsystem-match=misc --attr-match=name=uinput || true
fi
exit 0
