/*!
# `Cargo BashMan`
*/

#![forbid(unsafe_code)]

#![deny(
	clippy::allow_attributes_without_reason,
	clippy::correctness,
	unreachable_pub,
)]

#![warn(
	clippy::complexity,
	clippy::nursery,
	clippy::pedantic,
	clippy::perf,
	clippy::style,

	clippy::allow_attributes,
	clippy::clone_on_ref_ptr,
	clippy::create_dir,
	clippy::filetype_is_file,
	clippy::format_push_string,
	clippy::get_unwrap,
	clippy::impl_trait_in_params,
	clippy::lossy_float_literal,
	clippy::missing_assert_message,
	clippy::missing_docs_in_private_items,
	clippy::needless_raw_strings,
	clippy::panic_in_result_fn,
	clippy::pub_without_shorthand,
	clippy::rest_pat_in_fully_bound_structs,
	clippy::semicolon_inside_block,
	clippy::str_to_string,
	clippy::string_to_string,
	clippy::todo,
	clippy::undocumented_unsafe_blocks,
	clippy::unneeded_field_pattern,
	clippy::unseparated_literal_suffix,
	clippy::unwrap_in_result,

	macro_use_extern_crate,
	missing_copy_implementations,
	missing_docs,
	non_ascii_idents,
	trivial_casts,
	trivial_numeric_casts,
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



/// # Main.
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
/// # Actual main.
fn _main() -> Result<(), BashManError> {
	// Parse CLI arguments.
	let args = Argue::new(FLAG_HELP | FLAG_VERSION).map_err(BashManError::Argue)?;

	// Check for invalid CLI options.
	if let Some(boo) = args.check_keys(
		&[b"--no-bash", b"--no-credits", b"--no-man"],
		&[b"--features", b"--manifest-path", b"-f", b"-m"],
	) {
		return Err(BashManError::InvalidCli(String::from_utf8_lossy(boo).into()));
	}

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
/// # Print Help.
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
