/*!
# `Cargo BashMan`
*/

#![forbid(unsafe_code)]

#![warn(
	clippy::filetype_is_file,
	clippy::integer_division,
	clippy::needless_borrow,
	clippy::nursery,
	clippy::pedantic,
	clippy::perf,
	clippy::suboptimal_flops,
	clippy::unneeded_field_pattern,
	macro_use_extern_crate,
	missing_copy_implementations,
	missing_debug_implementations,
	missing_docs,
	non_ascii_idents,
	trivial_casts,
	trivial_numeric_casts,
	unreachable_pub,
	unused_crate_dependencies,
	unused_extern_crates,
	unused_import_braces,
)]



use argyle::{
	Argue,
	ArgyleError,
	FLAG_HELP,
	FLAG_VERSION,
};
use bashman_core::{
	BashManError,
	FLAG_ALL,
	FLAG_BASH,
	FLAG_CREDITS,
	FLAG_MAN,
};
use fyi_msg::Msg;
use std::path::PathBuf;



/// Main.
fn main() {
	match _main() {
		Ok(()) => {},
		Err(BashManError::Argue(ArgyleError::WantsVersion)) => {
			println!(concat!("Cargo BashMan v", env!("CARGO_PKG_VERSION")));
		},
		Err(BashManError::Argue(ArgyleError::WantsHelp)) => { helper(); },
		Err(e) => { Msg::error(e.to_string()).die(1); },
	}
}

#[inline]
// Actual main.
fn _main() -> Result<(), BashManError> {
	// Parse CLI arguments.
	let args = Argue::new(FLAG_HELP | FLAG_VERSION).map_err(BashManError::Argue)?;

	let mut flags: u8 = FLAG_ALL;
	if args.switch(b"--no-bash") {
		flags &= ! FLAG_BASH;
	}
	if args.switch(b"--no-credits") {
		flags &= ! FLAG_CREDITS;
	}
	if args.switch(b"--no-man") {
		flags &= ! FLAG_MAN;
	}

	let features = args.option2(b"-f", b"--features").and_then(|x| std::str::from_utf8(x).ok());

	let manifest =
		if let Some(p) = args.option2_os(b"-m", b"--manifest-path") {
			PathBuf::from(p)
		}
		else {
			std::env::current_dir()
				.map_err(|_| BashManError::InvalidManifest)?
				.join("Cargo.toml")
		};

	bashman_core::parse(manifest, flags, features)?;

	Ok(())
}

#[cold]
/// Print Help.
fn helper() {
	println!(concat!(
		r"
   __              __
   \ `-._......_.-` /
    `.  '.    .'  .'
     //  _`\/`_  \\
    ||  /\O||O/\  ||
    |\  \_/||\_/  /|
    \ '.   \/   .' /
    / ^ `'~  ~'`   \
   /  _-^_~ -^_ ~-  |    ", "\x1b[38;5;199mCargo BashMan\x1b[0;38;5;69m v", env!("CARGO_PKG_VERSION"), "\x1b[0m", r"
   | / ^_ -^_- ~_^\ |    A BASH completion script and MAN
   | |~_ ^- _-^_ -| |    page generator for Rust projects.
   | \  ^-~_ ~-_^ / |
   \_/;-.,____,.-;\_/
======(_(_(==)_)_)======

USAGE:
    cargo bashman [FLAGS] [OPTIONS]

FLAGS:
    -h, --help                  Prints help information.
        --no-bash               Do not generate BASH completions.
        --no-credits            Do not generate CREDITS.md.
        --no-man                Do not generate MAN page(s).
    -V, --version               Prints version information.

OPTIONS:
    -f, --features <FEATURES>   Comma-separated list of optional features to
                                include when generating CREDITS.md.
    -m, --manifest-path <FILE>  Read file paths from this list.
"
    ));
}
