.PHONY: build test release release-patch release-major

build:
	cargo build --release

test:
	cargo test

# bumps the minor version, tags, and pushes — the release workflow builds the binaries
release:
	cargo release minor --execute

release-patch:
	cargo release patch --execute

release-major:
	cargo release major --execute
