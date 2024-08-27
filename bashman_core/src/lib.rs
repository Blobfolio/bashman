/*!
# `Cargo BashMan` — Lib
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

#![expect(clippy::module_name_repetitions, reason = "Repetition is preferred.")]
#![expect(clippy::redundant_pub_crate, reason = "Unresolvable.")]



pub(crate) mod credits;
mod data;
mod error;
mod raw;

pub use error::BashManError;
use raw::Raw;
pub(crate) use data::{
	Command,
	DataFlag,
	DataItem,
	DataKind,
	DataOption,
	More,
};
use std::path::PathBuf;



/// # Enable BASH completions.
pub const FLAG_BASH: u8 =    0b0001;

/// # Enable CREDITS.md.
pub const FLAG_CREDITS: u8 = 0b0010;

/// # Enable MAN page(s).
pub const FLAG_MAN: u8 =     0b0100;

/// # All Flags.
pub const FLAG_ALL: u8 =     0b0111;



/// # Parse.
///
/// This is the sole public output of the entire library. It accepts a manifest
/// path, parses it, and builds and writes the appropriate outputs.
///
/// ## Errors
///
/// Returns an error if the BASH/Man output paths are invalid, or any other
/// metadata parsing issues come up.
pub fn parse(manifest: PathBuf, flags: u8, features: Option<&str>) -> Result<(), BashManError> {
	// Clean up the manifest path.
	let manifest = std::fs::canonicalize(manifest)
		.map_err(|_| BashManError::InvalidManifest)?;

	// Load and parse.
	let raw = std::fs::read_to_string(&manifest)
		.map_err(|_| BashManError::InvalidManifest)
		.and_then(Raw::try_from)?;
	let cmd = Command::try_from(&raw)?;

	// Establish a shared buffer to use to write chunked Man/BASH output to
	// (before sending said output to a file). A Vec is used instead of a
	// BufWriter because the manuals need to send their completed output to
	// two different writers.
	let mut buf: Vec<u8> = Vec::new();

	// Get the manifest's parent directory in case we have any relative paths
	// to deal with.
	let dir = manifest.parent().ok_or(BashManError::InvalidManifest)?.to_path_buf();

	// Write BASH.
	if FLAG_BASH == flags & FLAG_BASH {
		cmd.write_bash(&raw.bash_dir(&dir)?, &mut buf)?;
		buf.truncate(0);
	}

	// Write Man.
	if FLAG_MAN == flags & FLAG_MAN {
		cmd.write_man(&raw.man_dir(&dir)?, &mut buf)?;
		buf.truncate(0);
	}

	// Write Credits.
	if FLAG_CREDITS == flags & FLAG_CREDITS {
		cmd.write_credits(
			&manifest,
			features,
			&raw.credits_dir(&dir)?,
			&mut buf
		)?;
	}

	Ok(())
}
