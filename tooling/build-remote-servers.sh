#!/bin/sh
# Builds agentz-server for the machines the app reaches over SSH, and copies each binary to
# target/remote-servers/agentz-server-<target>, where development builds of the app look.
#
#   tooling/build-remote-servers.sh [rust target...]
#
# The default is x86_64-unknown-linux-musl. Linux servers are static musl builds made with
# `cargo zigbuild` (brew install zig cargo-zigbuild; rustup target add <target>), stripped,
# since the release profile's debug info makes them about 48 MB instead of 14.
set -eu

cd "$(dirname "$0")/.."
targets="${*:-x86_64-unknown-linux-musl}"
mkdir -p target/remote-servers
for target in $targets; do
    case "$target" in
        *-linux-musl)
            CARGO_PROFILE_RELEASE_STRIP=symbols \
                RUSTFLAGS="-C target-feature=+crt-static" \
                cargo zigbuild --release -p agentz_server --bin agentz-server --target "$target"
            ;;
        *-apple-darwin)
            CARGO_PROFILE_RELEASE_STRIP=symbols \
                cargo build --release -p agentz_server --bin agentz-server --target "$target"
            ;;
        *)
            echo "unsupported target: $target" >&2
            exit 1
            ;;
    esac
    cp "target/$target/release/agentz-server" "target/remote-servers/agentz-server-$target"
    echo "target/remote-servers/agentz-server-$target"
done
