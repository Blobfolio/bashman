##
# Development Recipes
#
# This justfile is intended to be run from inside a Docker sandbox:
# https://github.com/Blobfolio/righteous-sandbox
#
# docker run \
#	--rm \
#	-v "{{ invocation_directory() }}":/share \
#	-it \
#	--name "righteous_sandbox" \
#	"righteous/sandbox:debian"
#
# Alternatively, you can just run cargo commands the usual way and ignore these
# recipes.
##

pkg_id      := "cargo-bashman"
pkg_name    := "Cargo BashMan"
pkg_dir1    := justfile_directory() + "/bashman"
pkg_dir2    := justfile_directory() + "/bashman_core"

cargo_dir   := "/tmp/" + pkg_id + "-cargo"
cargo_bin   := cargo_dir + "/x86_64-unknown-linux-gnu/release/" + pkg_id
doc_dir     := justfile_directory() + "/doc"
release_dir := justfile_directory() + "/release"



# Build Release!
@build:
	# First let's build the Rust bit.
	cargo build \
		--bin "{{ pkg_id }}" \
		--release \
		--target x86_64-unknown-linux-gnu \
		--target-dir "{{ cargo_dir }}"


# Build Debian package!
@build-deb: clean build
	# Do completions/man.
	{{ cargo_bin }} -m "{{ pkg_dir1 }}/Cargo.toml"

	# cargo-deb doesn't support target_dir flags yet.
	[ ! -d "{{ justfile_directory() }}/target" ] || rm -rf "{{ justfile_directory() }}/target"
	mv "{{ cargo_dir }}" "{{ justfile_directory() }}/target"

	# Build the deb.
	cargo-deb \
		--no-build \
		-p {{ pkg_id }} \
		-o "{{ justfile_directory() }}/release"

	just _fix-chown "{{ release_dir }}"
	mv "{{ justfile_directory() }}/target" "{{ cargo_dir }}"


@clean:
	# Most things go here.
	[ ! -d "{{ cargo_dir }}" ] || rm -rf "{{ cargo_dir }}"

	# But some Cargo apps place shit in subdirectories even if they place
	# *other* shit in the designated target dir. Haha.
	[ ! -d "{{ justfile_directory() }}/target" ] || rm -rf "{{ justfile_directory() }}/target"
	[ ! -d "{{ pkg_dir1 }}/target" ] || rm -rf "{{ pkg_dir1 }}/target"
	[ ! -d "{{ pkg_dir2 }}/target" ] || rm -rf "{{ pkg_dir2 }}/target"

	cargo update -w


# Clippy.
@clippy:
	clear
	cargo clippy \
		--workspace \
		--release \
		--target x86_64-unknown-linux-gnu \
		--target-dir "{{ cargo_dir }}"


# Build Docs.
@doc:
	# Make the docs.
	cargo doc \
		--workspace \
		--release \
		--no-deps \
		--target x86_64-unknown-linux-gnu \
		--target-dir "{{ cargo_dir }}"

	# Move the docs and clean up ownership.
	[ ! -d "{{ doc_dir }}" ] || rm -rf "{{ doc_dir }}"
	mv "{{ cargo_dir }}/x86_64-unknown-linux-gnu/doc" "{{ justfile_directory() }}"
	just _fix-chown "{{ doc_dir }}"


# Test Run.
@run +ARGS:
	cargo run \
		--bin "{{ pkg_id }}" \
		--release \
		--target x86_64-unknown-linux-gnu \
		--target-dir "{{ cargo_dir }}" \
		-- {{ ARGS }}


# Unit tests!
@test:
	clear
	cargo test \
		--workspace \
		--target x86_64-unknown-linux-gnu \
		--target-dir "{{ cargo_dir }}"
	cargo test \
		--release \
		--workspace \
		--target x86_64-unknown-linux-gnu \
		--target-dir "{{ cargo_dir }}"


# Get/Set version.
version:
	#!/usr/bin/env bash

	# Current version.
	_ver1="$( toml get "{{ pkg_dir1 }}/Cargo.toml" package.version | \
		sed 's/"//g' )"

	# Find out if we want to bump it.
	_ver2="$( whiptail --inputbox "Set {{ pkg_name }} version:" --title "Release Version" 0 0 "$_ver1" 3>&1 1>&2 2>&3 )"

	exitstatus=$?
	if [ $exitstatus != 0 ] || [ "$_ver1" = "$_ver2" ]; then
		exit 0
	fi

	fyi success "Setting version to $_ver2."

	# Set the release version!
	just _version "{{ pkg_dir1 }}" "$_ver2"
	just _version "{{ pkg_dir2 }}" "$_ver2"


# Set version for real.
@_version DIR VER:
	[ -f "{{ DIR }}/Cargo.toml" ] || exit 1

	# Set the release version!
	toml set "{{ DIR }}/Cargo.toml" package.version "{{ VER }}" > /tmp/Cargo.toml
	just _fix-chown "/tmp/Cargo.toml"
	mv "/tmp/Cargo.toml" "{{ DIR }}/Cargo.toml"


# Init dependencies.
@_init:
	mkdir "/tmp/bashman-test"


# Fix file/directory permissions.
@_fix-chmod PATH:
	[ ! -e "{{ PATH }}" ] || find "{{ PATH }}" -type f -exec chmod 0644 {} +
	[ ! -e "{{ PATH }}" ] || find "{{ PATH }}" -type d -exec chmod 0755 {} +


# Fix file/directory ownership.
@_fix-chown PATH:
	[ ! -e "{{ PATH }}" ] || chown -R --reference="{{ justfile() }}" "{{ PATH }}"
