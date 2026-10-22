#!/bin/sh
# Builds agentz-server for the machines the app reaches over SSH, and copies each binary to
# target/remote-servers/agentz-server-<target>, where development builds of the app look and
# the bundle scripts take the app's own server from.
#
#   tooling/build-remote-servers.sh [rust target...]
#
# The default is x86_64-unknown-linux-musl. Linux servers are static musl builds made with
# `cargo zigbuild` (brew install zig cargo-zigbuild; rustup target add <target>). All of them
# use the release-server profile: stripped and with fat LTO, which make them smaller without
# slowing them, since a published app downloads the servers for other machines.
set -eu

cd "$(dirname "$0")/.."
targets="${*:-x86_64-unknown-linux-musl}"
mkdir -p target/remote-servers
for target in $targets; do
    case "$target" in
        *-linux-musl)
            RUSTFLAGS="-C target-feature=+crt-static" \
                cargo zigbuild --profile release-server -p agentz_server --bin agentz-server \
                --target "$target"
            ;;
        *-apple-darwin)
            cargo build --profile release-server -p agentz_server --bin agentz-server \
                --target "$target"
            ;;
        *)
            echo "unsupported target: $target" >&2
            exit 1
            ;;
    esac
    server="target/remote-servers/agentz-server-$target"
    cp "target/$target/release-server/agentz-server" "$server"
    case "$target" in
        # Apple silicon runs only signed code: an ad-hoc signature, as the app has.
        *-apple-darwin) codesign --force --sign - "$server" ;;
    esac
    echo "$server"
done
