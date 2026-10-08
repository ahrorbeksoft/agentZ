#!/bin/sh
# Installs agentZ from its GitHub releases: the app on a Mac; on Linux, agentz-server, for a
# machine you reach from the app over SSH, and the app too on a desktop.
#
#   curl -fsSL https://ahrorbeksoft.github.io/agentZ/install.sh | sh
#
# Run it again to update. AGENTZ_VERSION=0.1.0 installs that release instead of the latest one.
# On a Mac, AGENTZ_APP_DIR picks where the app goes (/Applications by default, or
# ~/Applications when /Applications isn't writable). On Linux, the app goes in ~/.local when
# this runs in a desktop session; AGENTZ_APP=1 installs it anyway, AGENTZ_APP=0 never.
set -eu

repository=ahrorbeksoft/agentZ
releases="https://github.com/$repository/releases"

say() {
    printf '%s\n' "$*"
}

fail() {
    printf 'agentZ: %s\n' "$*" >&2
    exit 1
}

need() {
    command -v "$1" > /dev/null 2>&1 || fail "this needs $1"
}

download() {
    curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location --retry 3 \
        --output "$2" "$1" || fail "couldn't download $1"
}

sha256() {
    if command -v sha256sum > /dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    elif command -v shasum > /dev/null 2>&1; then
        shasum -a 256 "$1" | cut -d ' ' -f 1
    else
        fail "this needs sha256sum or shasum"
    fi
}

# Checks a download against the release's SHA256SUMS.
verify() {
    expected=$(awk -v name="$2" '$2 == name || $2 == "*" name { print $1 }' "$temporary/SHA256SUMS")
    [ -n "$expected" ] || fail "the release has no checksum for $2"
    [ "$(sha256 "$1")" = "$expected" ] || fail "$2 arrived damaged (its checksum doesn't match)"
}

install_mac() {
    need ditto
    asset=agentZ-macos.zip
    say "Downloading agentZ $version for macOS..."
    download "$base/$asset" "$temporary/$asset"
    verify "$temporary/$asset" "$asset"
    ditto -x -k "$temporary/$asset" "$temporary/unpacked"
    [ -d "$temporary/unpacked/agentZ.app" ] || fail "$asset has no agentZ.app in it"
    # curl doesn't quarantine what it downloads, but a zip carried over from a browser would be.
    xattr -dr com.apple.quarantine "$temporary/unpacked/agentZ.app" 2> /dev/null || true

    directory=${AGENTZ_APP_DIR:-}
    if [ -z "$directory" ]; then
        if [ -w /Applications ]; then
            directory=/Applications
        else
            directory="$HOME/Applications"
        fi
    fi
    mkdir -p "$directory"
    destination="$directory/agentZ.app"
    if [ -e "$destination" ]; then
        mv "$destination" "$temporary/previous.app" || fail "couldn't replace $destination"
    fi
    if ! mv "$temporary/unpacked/agentZ.app" "$destination"; then
        if [ -e "$temporary/previous.app" ]; then
            mv "$temporary/previous.app" "$destination"
        fi
        fail "couldn't put agentZ in $directory"
    fi

    say "Installed agentZ $version in $destination"
    say "Open it from $directory, or run: open \"$destination\""
    say "If agentZ was running, quit and reopen it to use the new version."
}

install_linux() {
    target="$arch-unknown-linux-musl"
    asset="agentz-server-$target"
    say "Downloading agentz-server $version for $target..."
    download "$base/$asset" "$temporary/$asset"
    verify "$temporary/$asset" "$asset"

    # Where the app installs servers over SSH, with the hash it checks, so it finds this one
    # already in place and doesn't upload its own.
    directory="$HOME/.agentz/server/$version"
    mkdir -p "$directory"
    staged="$directory/agentz-server.tmp.$$"
    cp "$temporary/$asset" "$staged"
    chmod 755 "$staged"
    # A rename leaves a server that runs from the old file running.
    mv "$staged" "$directory/agentz-server"
    sha256 "$directory/agentz-server" > "$directory/agentz-server.sha256"

    bin_directory="$HOME/.local/bin"
    mkdir -p "$bin_directory"
    ln -sf "$directory/agentz-server" "$bin_directory/agentz-server"

    say "Installed agentz-server $version in $directory"
    say "In agentZ on your computer, open Settings > Machines > Add Machine and enter this"
    say "machine's SSH address (for example $(id -un)@$(uname -n)). The app starts the server when"
    say "it connects."
    case ":$PATH:" in
        *":$bin_directory:"*) ;;
        *) say "To run agentz-server yourself, add $bin_directory to your PATH." ;;
    esac
}

# The app, as Zed's install.sh puts Zed in ~/.local: agentz.app there, agentz in ~/.local/bin,
# and its desktop entry pointing at both.
install_linux_app() {
    asset="agentZ-linux-$arch.tar.gz"
    if ! awk -v name="$asset" '$2 == name || $2 == "*" name { found = 1 } END { exit !found }' \
        "$temporary/SHA256SUMS"; then
        say "agentZ $version has no app for $arch Linux; installed only the server."
        return
    fi
    say "Downloading agentZ $version for Linux..."
    download "$base/$asset" "$temporary/$asset"
    verify "$temporary/$asset" "$asset"
    mkdir -p "$temporary/unpacked"
    tar -xzf "$temporary/$asset" -C "$temporary/unpacked"
    [ -x "$temporary/unpacked/agentz.app/bin/agentz" ] || fail "$asset has no agentz.app in it"

    mkdir -p "$HOME/.local"
    destination="$HOME/.local/agentz.app"
    if [ -e "$destination" ]; then
        mv "$destination" "$temporary/previous.app" || fail "couldn't replace $destination"
    fi
    if ! mv "$temporary/unpacked/agentz.app" "$destination"; then
        if [ -e "$temporary/previous.app" ]; then
            mv "$temporary/previous.app" "$destination"
        fi
        fail "couldn't put agentZ in $HOME/.local"
    fi

    if command -v ldd > /dev/null 2>&1; then
        missing=$(ldd "$destination/bin/agentz" 2> /dev/null | sed -n 's/^[[:space:]]*\(.*\) => not found$/\1/p')
        if [ -n "$missing" ]; then
            say "Your system is missing libraries that agentZ needs:"
            say "$missing" | sed 's/^/    /'
            say "Install them with your package manager, or agentZ won't start."
        fi
    fi

    bin_directory="$HOME/.local/bin"
    applications="$HOME/.local/share/applications"
    mkdir -p "$bin_directory" "$applications"
    ln -sf "$destination/bin/agentz" "$bin_directory/agentz"
    app_id=dev.agentz.agentZ
    sed -e "s|^TryExec=agentz$|TryExec=$destination/bin/agentz|" \
        -e "s|^Exec=agentz$|Exec=$destination/bin/agentz|" \
        -e "s|^Icon=$app_id$|Icon=$destination/share/icons/hicolor/1024x1024/apps/$app_id.png|" \
        "$destination/share/applications/$app_id.desktop" > "$applications/$app_id.desktop"

    say "Installed agentZ $version in $destination"
    say "Open it from your applications, or run: $bin_directory/agentz"
    say "If agentZ was running, quit and reopen it to use the new version."
}

need curl
need uname
need awk

case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 ;;
    aarch64 | arm64) arch=aarch64 ;;
    *) fail "$(uname -m) processors aren't supported" ;;
esac

if [ -n "${AGENTZ_VERSION:-}" ]; then
    version=${AGENTZ_VERSION#v}
else
    latest=$(curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location \
        --head --output /dev/null --write-out '%{url_effective}' "$releases/latest") ||
        fail "couldn't reach $releases"
    case "$latest" in
        */tag/v*) version=${latest##*/tag/v} ;;
        *) fail "there are no releases yet" ;;
    esac
fi
case "$version" in
    "" | *[!0-9A-Za-z.+-]*) fail "\"$version\" isn't a version" ;;
esac
base="$releases/download/v$version"

temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT
trap 'exit 1' INT TERM
download "$base/SHA256SUMS" "$temporary/SHA256SUMS"

case "$(uname -s)" in
    Darwin) install_mac ;;
    Linux)
        if [ "$(uname -o 2> /dev/null)" = Android ]; then
            fail "Android isn't supported"
        fi
        install_linux
        case "${AGENTZ_APP:-}" in
            1) install_linux_app ;;
            0) ;;
            *)
                if [ -n "${WAYLAND_DISPLAY:-}" ] || [ -n "${DISPLAY:-}" ]; then
                    install_linux_app
                fi
                ;;
        esac
        ;;
    *) fail "this installs on macOS and Linux, not $(uname -s)" ;;
esac
