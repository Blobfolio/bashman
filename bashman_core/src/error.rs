/*!
# `Cargo BashMan` - Error
*/

use argue::ArgueError;
use std::fmt;



#[derive(Debug, Clone)]
/// # Error.
pub enum BashManError {
	/// # Argue Passthru.
	Argue(ArgueError),
	/// # Invalid Bash output directory.
	InvalidBashDir,
	/// # Invalid flag.
	InvalidFlag,
	/// # Invalid item.
	InvalidItem,
	/// # Invalid Man output directory.
	InvalidManDir,
	/// # Invalid manifest.
	InvalidManifest,
	/// # Invalid section.
	InvalidSection,
	/// # Invalid subcommand.
	InvalidSubCommand(String),
	/// # Missing subcommand.
	MissingSubCommand,
	/// # Parse manifest.
	ParseManifest(String),
	/// # Write Bash.
	WriteBash,
	/// # Write Man.
	WriteMan,
	/// # Write Man.
	WriteSubMan(String),
}

impl fmt::Display for BashManError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Argue(src) => f.write_str(src.as_ref()),
			Self::InvalidBashDir => f.write_str("Invalid BASH output directory."),
			Self::InvalidFlag => f.write_str("Flags require at least one short/long key."),
			Self::InvalidItem => f.write_str("Items require a key and value."),
			Self::InvalidManDir => f.write_str("Invalid Man output directory."),
			Self::InvalidManifest => f.write_str("Invalid manifest path."),
			Self::InvalidSection => f.write_str("Sections cannot be empty."),
			Self::InvalidSubCommand(s) => f.write_fmt(format_args!("Invalid subcommand: {:?}", s)),
			Self::MissingSubCommand => f.write_str("Missing subcommand 'cmd' field."),
			Self::ParseManifest(e) => f.write_fmt(format_args!("Unable to parse manifest: {:?}", e)),
			Self::WriteBash => f.write_str("Unable to write BASH completions."),
			Self::WriteMan => f.write_str("Unable to write Manual(s)."),
			Self::WriteSubMan(s) => f.write_fmt(format_args!("Unable to write Man for {:?}.", s)),
		}
	}
}
