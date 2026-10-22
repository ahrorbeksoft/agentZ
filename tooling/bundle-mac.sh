#!/bin/sh
# Builds target/bundle/agentZ.app: the app and agentz-server in Contents/MacOS, the servers
# from tooling/build-remote-servers.sh (if built) in Contents/Resources, the icon, and an
# ad-hoc signature. Run from a bundle, macOS shows agentZ's notifications, and `open -g` starts
# it without taking focus.
#
#   tooling/bundle-mac.sh [--debug | --dmg <rust target>]
#
# --debug bundles the debug builds instead of making release ones. --dmg makes the release
# that's published for one kind of Mac (aarch64-apple-darwin or x86_64-apple-darwin),
# target/bundle/agentZ-macos-<arch>.dmg: that Mac's app and server only, since the published
# app downloads other machines' servers from its release.
set -eu

usage() {
    echo "usage: $0 [--debug | --dmg aarch64-apple-darwin | --dmg x86_64-apple-darwin]" >&2
    exit 1
}

cd "$(dirname "$0")/.."
profile=release
target=
case "${1:-}" in
    --debug) profile=debug ;;
    --dmg)
        target=${2:-}
        case "$target" in
            aarch64-apple-darwin | x86_64-apple-darwin) ;;
            *) usage ;;
        esac
        ;;
    "") ;;
    *) usage ;;
esac

if [ -n "$target" ]; then
    # The release profile's debug info would more than double the download, and the symbol
    # table is a large part of the rest (Zed strips local symbols from its bundles for the
    # same reason); a panic's backtrace then has addresses only.
    CARGO_PROFILE_RELEASE_STRIP=symbols \
        cargo build --release -p app --bin agentz --target "$target"
    if [ ! -f "target/remote-servers/agentz-server-$target" ]; then
        tooling/build-remote-servers.sh "$target"
    fi
elif [ "$profile" = release ]; then
    cargo build --release -p app -p agentz_server --bin agentz --bin agentz-server
else
    cargo build -p app -p agentz_server --bin agentz --bin agentz-server
fi

version=$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
app=target/bundle/agentZ.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
if [ -n "$target" ]; then
    cp "target/$target/release/agentz" "$app/Contents/MacOS/agentz"
    cp "target/remote-servers/agentz-server-$target" "$app/Contents/MacOS/agentz-server"
else
    # Clones on APFS, so the copies take no space until they change.
    cp -c "target/$profile/agentz" "target/$profile/agentz-server" "$app/Contents/MacOS/"
    for server in target/remote-servers/agentz-server-*; do
        if [ -f "$server" ]; then
            cp -c "$server" "$app/Contents/Resources/"
        fi
    done
fi

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

if [ -z "$target" ]; then
    echo "$app"
    exit
fi
dmg="target/bundle/agentZ-macos-${target%%-*}.dmg"
contents=target/bundle/dmg
rm -rf "$contents" "$dmg"
mkdir -p "$contents"
ditto "$app" "$contents/agentZ.app"
# To drag the app to, as in Zed's DMG.
ln -s /Applications "$contents/Applications"
# LZMA (ULMO, macOS 10.15 and later) rather than the zlib of Zed's DMG (UDZO): about a third
# smaller, as xz is than gzip.
hdiutil create -volname agentZ -srcfolder "$contents" -ov -format ULMO "$dmg"
rm -rf "$contents"
echo "$dmg"
