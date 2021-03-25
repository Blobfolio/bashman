/*!
# `Cargo BashMan` — Lib
*/

#![warn(clippy::filetype_is_file)]
#![warn(clippy::integer_division)]
#![warn(clippy::needless_borrow)]
#![warn(clippy::nursery)]
#![warn(clippy::pedantic)]
#![warn(clippy::perf)]
#![warn(clippy::suboptimal_flops)]
#![warn(clippy::unneeded_field_pattern)]
#![warn(macro_use_extern_crate)]
#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(non_ascii_idents)]
#![warn(trivial_casts)]
#![warn(trivial_numeric_casts)]
#![warn(unreachable_pub)]
#![warn(unused_crate_dependencies)]
#![warn(unused_extern_crates)]
#![warn(unused_import_braces)]

#![allow(clippy::module_name_repetitions)]



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
use std::{
	convert::TryFrom,
	path::PathBuf,
};



#[allow(clippy::missing_panics_doc)] // This can't really panic; the path exists.
/// # Parse.
///
/// This is the sole public output of the entire library. It accepts a manifest
/// path, parses it, and builds and writes the appropriate outputs.
///
/// ## Errors
///
/// Returns an error if the BASH/Man output paths are invalid, or any other
/// metadata parsing issues come up.
pub fn parse(manifest: PathBuf) -> Result<(), BashManError> {
	// Clean up the manifest path.
	let manifest = std::fs::canonicalize(manifest)
		.map_err(|_| BashManError::InvalidManifest)?;

	// Load it as a string.
	let content = std::fs::read_to_string(&manifest)
		.map_err(|_| BashManError::InvalidManifest)?;

	// Parse the raw data.
	let raw = Raw::try_from(content.as_str())?;
	let cmd = Command::try_from(&raw)?;

	// Establish a shared buffer to use to write chunked Man/BASH output to
	// (before sending said output to a file). A Vec is used instead of a
	// BufWriter because the manuals need to send their completed output to
	// two different writers.
	let mut buf: Vec<u8> = Vec::new();

	// Write BASH.
	let dir = manifest.parent().unwrap().to_path_buf();
	cmd.write_bash(&raw.bash_dir(&dir)?, &mut buf)?;

	// Write Man.
	buf.truncate(0);
	cmd.write_man(&raw.man_dir(&dir)?, &mut buf)?;

	Ok(())
}
