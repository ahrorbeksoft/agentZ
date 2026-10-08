#!/bin/sh
# Builds target/bundle/agentZ-linux-<arch>.tar.gz, Zed's Linux bundle: agentz.app with the app in
# bin and agentz-server beside it, the servers for other Linux machines from
# tooling/build-remote-servers.sh in remote-servers, a desktop entry and the icon. site/install.sh
# puts it in ~/.local.
#
#   tooling/bundle-linux.sh
#
# The server is the static musl build, the one SSH machines run, so the app can install its own
# on a machine like this one. Unlike Zed's bundle, it carries no libraries: the app links only
# libc, xkbcommon and xcb, which every desktop has, and loads Wayland and Vulkan as it starts.
set -eu

cd "$(dirname "$0")/.."
arch=$(uname -m)
server_target="$arch-unknown-linux-musl"
app_id=dev.agentz.agentZ

# The release profile's debug info would more than double the download.
CARGO_PROFILE_RELEASE_STRIP=debuginfo cargo build --release -p app --bin agentz
if [ ! -f "target/remote-servers/agentz-server-$server_target" ]; then
    tooling/build-remote-servers.sh "$server_target"
fi

bundle=target/bundle/linux
app="$bundle/agentz.app"
rm -rf "$bundle"
mkdir -p "$app/bin" "$app/remote-servers" "$app/share/applications" \
    "$app/share/icons/hicolor/1024x1024/apps"
cp target/release/agentz "$app/bin/agentz"
cp "target/remote-servers/agentz-server-$server_target" "$app/bin/agentz-server"
for server in target/remote-servers/agentz-server-*; do
    case "$server" in
        *"-$server_target") ;;
        *) cp "$server" "$app/remote-servers/" ;;
    esac
done
# Artifacts lose their modes on the way between CI jobs.
chmod 755 "$app/bin/agentz" "$app/bin/agentz-server" "$app"/remote-servers/* 2> /dev/null || true
cp crates/app/resources/app-icon.png "$app/share/icons/hicolor/1024x1024/apps/$app_id.png"

# Exec and Icon are names here; install.sh points them at where it puts the app.
cat > "$app/share/applications/$app_id.desktop" <<EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=agentZ
GenericName=Coding Agents
Comment=Work with coding agents across projects and machines
TryExec=agentz
Exec=agentz
Icon=$app_id
StartupNotify=true
StartupWMClass=$app_id
Categories=Development;
EOF

archive="target/bundle/agentZ-linux-$arch.tar.gz"
rm -f "$archive"
tar -czf "$archive" -C "$bundle" agentz.app
echo "$archive"
