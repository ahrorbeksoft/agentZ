#!/bin/sh
# Builds target/bundle/agentZ.app: the app and agentz-server in Contents/MacOS, the Linux
# servers from tooling/build-remote-servers.sh (if built) in Contents/Resources, and an ad-hoc
# signature. Run from a bundle, macOS shows agentZ's notifications, and `open -g` starts it
# without taking focus.
#
#   tooling/bundle-mac.sh [--debug]
#
# --debug bundles the debug builds instead of making release ones.
set -eu

cd "$(dirname "$0")/.."
profile=release
case "${1:-}" in
    --debug) profile=debug ;;
    "") ;;
    *)
        echo "usage: $0 [--debug]" >&2
        exit 1
        ;;
esac

if [ "$profile" = release ]; then
    cargo build --release -p app -p agentz_server --bin agentz --bin agentz-server
else
    cargo build -p app -p agentz_server --bin agentz --bin agentz-server
fi

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' crates/app/Cargo.toml | head -n 1)
app=target/bundle/agentZ.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
# Clones on APFS, so the copies take no space until they change.
cp -c "target/$profile/agentz" "target/$profile/agentz-server" "$app/Contents/MacOS/"
for server in target/remote-servers/agentz-server-*; do
    if [ -f "$server" ]; then
        cp -c "$server" "$app/Contents/Resources/"
    fi
done

cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleDisplayName</key>
	<string>agentZ</string>
	<key>CFBundleExecutable</key>
	<string>agentz</string>
	<key>CFBundleIdentifier</key>
	<string>dev.agentz.agentZ</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>agentZ</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>$version</string>
	<key>CFBundleVersion</key>
	<string>$version</string>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.developer-tools</string>
	<key>LSMinimumSystemVersion</key>
	<string>10.15.7</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSSupportsAutomaticGraphicsSwitching</key>
	<true/>
</dict>
</plist>
EOF
plutil -lint "$app/Contents/Info.plist" > /dev/null

# Inside out: the server, then the bundle, which seals the app and its resources.
codesign --force --sign - "$app/Contents/MacOS/agentz-server"
codesign --force --sign - "$app"
codesign --verify --strict "$app"
echo "$app"
