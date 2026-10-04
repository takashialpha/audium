# A command runner, not a build system: cargo already knows what is stale, and
# a second layer of timestamps would only be another thing that can be wrong.
#
# Every command CI runs lives here, and CI calls these recipes rather than
# spelling the flags out again, so a green local run and a green pipeline
# cannot mean two different things.

# list what there is to run
default:
    @just --list --unsorted

# build the player; TARGET is optional (e.g. x86_64-unknown-linux-gnu)
build target="":
    #!/bin/sh
    set -eu
    # The release workflow needs an explicit target triple; a local build does
    # not. Both go through here so the profile cannot differ between them.
    if [ -n "{{ target }}" ]; then
        cargo build --release --target "{{ target }}"
    else
        cargo build --release
    fi

# everything CI runs
check: fmt lint test

# check formatting
fmt:
    cargo fmt --all -- --check

# lint with the crate's own strict lint set
lint:
    cargo clippy --all-targets -- -D warnings

# run the tests (see CONTRIBUTING.md for why there are so few)
test:
    cargo test --verbose

# throw away everything built
clean:
    cargo clean
