/*!
# `Cargo BashMan` - Error
*/

use fyi_menu::ArgueError;
use std::fmt;



#[derive(Debug, Clone)]
/// # Error.
pub enum BashManError {
	/// # fyi_menu Passthru.
	Argue(ArgueError),
	/// # Invalid Bash output directory.
	InvalidBashDir,
	/// # Invalid Man output directory.
	InvalidManDir,
	/// # Invalid manifest.
	InvalidManifest,
	/// # Invalid subcommand.
	InvalidSubCommand(String),
	/// # Missing subcommand.
	MissingSubCommand,
	/// # Parse manifest.
	ParseManifest,
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
			Self::InvalidManDir => f.write_str("Invalid Man output directory."),
			Self::InvalidManifest => f.write_str("Invalid manifest path."),
			Self::InvalidSubCommand(s) => f.write_fmt(format_args!("Invalid subcommand: {:?}", s)),
			Self::MissingSubCommand => f.write_str("Missing subcommand 'cmd' field."),
			Self::ParseManifest => f.write_str("Unable to parse manifest."),
			Self::WriteBash => f.write_str("Unable to write BASH completions."),
			Self::WriteMan => f.write_str("Unable to write Manual(s)."),
			Self::WriteSubMan(s) => f.write_fmt(format_args!("Unable to write Man for {:?}.", s)),
		}
	}
}
