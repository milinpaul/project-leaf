#!/usr/bin/env bash
# System packages GPUI needs to build on Linux. Adjust if the gpui version changes.
set -euo pipefail
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  clang pkg-config libssl-dev \
  libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev \
  libx11-xcb-dev libxcb1-dev libvulkan-dev \
  libfontconfig-dev libfreetype-dev libasound2-dev libzstd-dev
