#!/usr/bin/env bash
# Runs the kettle suite against one of the apps, and saves its results and screenshots in
# docs/ for the manual:
#
#   ./run.sh gpui | swiftui | react-native
#
# The iOS apps run in a simulator: the booted one, or $SIMULATOR (a name or UDID).
set -euo pipefail
cd "$(dirname "$0")"
root="$(cd ../.. && pwd)"
here="$(pwd)"
export LANG=en_US.UTF-8 LC_ALL=en_US.UTF-8

cargo build -q --manifest-path "$root/Cargo.toml" -p expectation_remote
run="$root/target/debug/expectation-run"
url=http://127.0.0.1:4477

simulator() {
  local id="${SIMULATOR:-}"
  if [ -z "$id" ]; then
    id=$(xcrun simctl list devices booted | sed -nE 's/.*\(([0-9A-F-]{36})\) \(Booted\).*/\1/p' | head -1)
  fi
  if [ -z "$id" ]; then
    id=$(xcrun simctl list devices available | sed -nE 's/.*iPhone.*\(([0-9A-F-]{36})\).*/\1/p' | head -1)
  fi
  xcrun simctl boot "$id" 2>/dev/null || true
  xcrun simctl bootstatus "$id" -b >/dev/null
  echo "$id"
}

# Starts the runner for $1 in the background (more options after it), then the app.
runner() {
  local platform="$1"; shift
  "$run" expectations --platform "$platform" --docs docs --runner app --wait 120 "$@" &
  runner_pid=$!
  sleep 1
}

case "${1:-}" in
  gpui)
    cargo build -q --manifest-path gpui/Cargo.toml
    runner GPUI
    EXPECTATION_URL=$url gpui/target/debug/kettle_gpui >/dev/null 2>&1 &
    app=$!
    status=0; wait $runner_pid || status=$?
    kill $app 2>/dev/null || true
    ;;
  swiftui)
    sim=$(simulator)
    (cd swiftui && xcodegen generate -q && xcodebuild -project Kettle.xcodeproj -scheme Kettle \
      -destination "platform=iOS Simulator,id=$sim" -derivedDataPath build -quiet build)
    xcrun simctl install "$sim" swiftui/build/Build/Products/Debug-iphonesimulator/Kettle.app
    runner SwiftUI
    SIMCTL_CHILD_EXPECTATION_URL=$url xcrun simctl launch --terminate-running-process "$sim" net.sim001.expectation.kettle >/dev/null
    status=0; wait $runner_pid || status=$?
    xcrun simctl terminate "$sim" net.sim001.expectation.kettle 2>/dev/null || true
    ;;
  react-native)
    sim=$(simulator)
    (cd react-native && [ -d node_modules ] || npm install)
    (cd react-native/ios && [ -d Pods ] || pod install)
    xcodebuild -workspace react-native/ios/KettleRN.xcworkspace -scheme KettleRN -configuration Release \
      -destination "platform=iOS Simulator,id=$sim" -derivedDataPath react-native/ios/build -quiet build
    xcrun simctl install "$sim" react-native/ios/build/Build/Products/Release-iphonesimulator/KettleRN.app
    runner "React Native" --screenshot-cmd "xcrun simctl io $sim screenshot --type=png \"\$1\" >/dev/null 2>&1"
    xcrun simctl launch --terminate-running-process "$sim" org.reactjs.native.example.KettleRN >/dev/null
    status=0; wait $runner_pid || status=$?
    xcrun simctl terminate "$sim" org.reactjs.native.example.KettleRN 2>/dev/null || true
    ;;
  *)
    echo "usage: ./run.sh gpui | swiftui | react-native" >&2
    exit 2
    ;;
esac

# Phone screenshots are big: keep them at most 1200 px tall.
find docs -name '*.png' -path '*screenshots*' -exec sips -Z 1200 {} >/dev/null \;
exit $status
