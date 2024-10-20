/*!
# `Cargo BashMan` - Error
*/

use std::error::Error;
use std::fmt;



#[derive(Debug, Clone, Eq, PartialEq)]
/// # Error.
pub enum BashManError {
	/// # Invalid Bash output directory.
	InvalidBashDir,

	/// # Invalid Credits output directory.
	InvalidCreditsDir,

	/// # Invalid CLI Option.
	InvalidCli(Box<str>),

	/// # Invalid flag.
	InvalidFlag,

	/// # Invalid item.
	InvalidItem,

	/// # Invalid/missing Cargo.lock.
	InvalidLock,

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

	/// # Write Credits.
	WriteCredits,

	/// # Write Man.
	WriteMan,

	/// # Write Man.
	WriteSubMan(Box<str>),

	/// # Print Help (Not an Error).
	PrintHelp,

	/// # Print Version (Not an Error).
	PrintVersion,
}

impl Error for BashManError {}

impl fmt::Display for BashManError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::InvalidBashDir => f.write_str("Invalid BASH output directory."),
			Self::InvalidCli(s) => f.write_fmt(format_args!("Invalid/unknown CLI option: {s}")),
			Self::InvalidCreditsDir => f.write_str("Invalid credits output directory."),
			Self::InvalidFlag => f.write_str("Flags require at least one short/long key."),
			Self::InvalidItem => f.write_str("Items require a key and value."),
			Self::InvalidLock => f.write_str("Invalid or missing Cargo.lock."),
			Self::InvalidManDir => f.write_str("Invalid Man output directory."),
			Self::InvalidManifest => f.write_str("Invalid manifest path."),
			Self::InvalidSection => f.write_str("Sections cannot be empty."),
			Self::InvalidSubCommand(s) => f.write_fmt(format_args!("Invalid subcommand: {s:?}")),
			Self::MissingSubCommand => f.write_str("Missing subcommand 'cmd' field."),
			Self::ParseManifest(e) => f.write_fmt(format_args!("Unable to parse manifest: {e:?}")),
			Self::WriteBash => f.write_str("Unable to write BASH completions."),
			Self::WriteCredits => f.write_str("Unable to write CREDITS.md."),
			Self::WriteMan => f.write_str("Unable to write Manual(s)."),
			Self::WriteSubMan(s) => f.write_fmt(format_args!("Unable to write Man for {s:?}.")),
			Self::PrintHelp | Self::PrintVersion => Ok(()),
		}
	}
}
