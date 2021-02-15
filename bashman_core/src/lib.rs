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

#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::map_err_ignore)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]

mod data;
mod error;
mod raw;

pub use error::BashManError;
pub use raw::Raw;
pub use data::{
	Command,
	DataFlag,
	DataItem,
	DataKind,
	DataOption,
	More,
};
use std::{
	convert::TryFrom,
	path::Path,
};



/// # Parse.
pub fn parse<P>(manifest: P) -> Result<(), BashManError>
where P: AsRef<Path> {
	// Clean up the manifest path.
	let manifest = std::fs::canonicalize(manifest)
		.map_err(|_| BashManError::InvalidManifest)?;

	// Load it as a string.
	let content = std::fs::read_to_string(&manifest)
		.map_err(|_| BashManError::InvalidManifest)?;

	// Parse the raw data.
	let raw = Raw::try_from(content.as_str())?;

	// The actually-useful data!
	let cmd = raw.parse()?;

	// A base directory.
	let dir = manifest.parent().unwrap().to_path_buf();
	let bash_dir = raw.bash_dir(&dir)?;
	let man_dir = raw.man_dir(&dir)?;

	// Write BASH.
	cmd.write_bash(&bash_dir)?;
	cmd.write_man(&man_dir)?;

	Ok(())
}
