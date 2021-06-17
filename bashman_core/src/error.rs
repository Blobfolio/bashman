/*!
# `Cargo BashMan` - Error
*/

use argyle::ArgyleError;
use std::error::Error;
use std::fmt;



#[derive(Debug, Clone)]
/// # Error.
pub enum BashManError {
	/// # Argue Passthru.
	Argue(ArgyleError),
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
	InvalidSubCommand(Box<str>),
	/// # Missing subcommand.
	MissingSubCommand,
	/// # Parse manifest.
	ParseManifest(Box<str>),
	/// # Write Bash.
	WriteBash,
	/// # Write Man.
	WriteMan,
	/// # Write Man.
	WriteSubMan(Box<str>),
}

impl Error for BashManError {}

impl fmt::Display for BashManError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Argue(src) => f.write_str(src.as_str()),
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
