#!/bin/sh
# Builds target/bundle/agentZ.app: the app and agentz-server in Contents/MacOS, the Linux
# servers from tooling/build-remote-servers.sh (if built) in Contents/Resources, the icon, and an
# ad-hoc signature. Run from a bundle, macOS shows agentZ's notifications, and `open -g` starts
# it without taking focus.
#
#   tooling/bundle-mac.sh [--debug | --universal]
#
# --debug bundles the debug builds instead of making release ones. --universal makes the release
# that's published: release builds for Apple silicon and Intel joined with lipo, and each one's
# server in Contents/Resources too, for a remote Mac of the other kind.
set -eu

cd "$(dirname "$0")/.."
profile=release
universal=false
case "${1:-}" in
    --debug) profile=debug ;;
    --universal) universal=true ;;
    "") ;;
    *)
        echo "usage: $0 [--debug | --universal]" >&2
        exit 1
        ;;
esac

mac_targets="aarch64-apple-darwin x86_64-apple-darwin"
if [ "$universal" = true ]; then
    for target in $mac_targets; do
        # The release profile's debug info would more than double the download.
        CARGO_PROFILE_RELEASE_STRIP=debuginfo \
            cargo build --release -p app -p agentz_server --bin agentz --bin agentz-server \
            --target "$target"
    done
elif [ "$profile" = release ]; then
    cargo build --release -p app -p agentz_server --bin agentz --bin agentz-server
else
    cargo build -p app -p agentz_server --bin agentz --bin agentz-server
fi

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' crates/app/Cargo.toml | head -n 1)
app=target/bundle/agentZ.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
if [ "$universal" = true ]; then
    for binary in agentz agentz-server; do
        lipo -create -output "$app/Contents/MacOS/$binary" \
            "target/aarch64-apple-darwin/release/$binary" \
            "target/x86_64-apple-darwin/release/$binary"
    done
    for target in $mac_targets; do
        cp "target/$target/release/agentz-server" \
            "$app/Contents/Resources/agentz-server-$target"
    done
else
    # Clones on APFS, so the copies take no space until they change.
    cp -c "target/$profile/agentz" "target/$profile/agentz-server" "$app/Contents/MacOS/"
fi
for server in target/remote-servers/agentz-server-*; do
    if [ -f "$server" ]; then
        cp -c "$server" "$app/Contents/Resources/"
    fi
done

iconset=target/bundle/AppIcon.iconset
rm -rf "$iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" crates/app/resources/app-icon.png \
        --out "$iconset/icon_${size}x${size}.png" > /dev/null
    sips -z "$((size * 2))" "$((size * 2))" crates/app/resources/app-icon.png \
        --out "$iconset/icon_${size}x${size}@2x.png" > /dev/null
done
iconutil -c icns -o "$app/Contents/Resources/AppIcon.icns" "$iconset"
rm -rf "$iconset"

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
	<key>CFBundleIconFile</key>
	<string>AppIcon</string>
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

# Inside out: the servers, then the bundle, which seals the app and its resources.
codesign --force --sign - "$app/Contents/MacOS/agentz-server"
for server in "$app"/Contents/Resources/agentz-server-*-apple-darwin; do
    if [ -f "$server" ]; then
        codesign --force --sign - "$server"
    fi
done
codesign --force --sign - "$app"
codesign --verify --strict "$app"
echo "$app"
