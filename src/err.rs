/*!
# Cargo BashMan: Errors.
*/

use crate::{
	KeyWord,
	TargetTriple,
};
use fyi_ansi::{
	ansi,
	csi,
	dim,
};
use std::fmt;



/// # Help Text.
const HELP: &str = concat!(r"
   __              __
   \ `-._......_.-` /
    `.  '.    .'  .'
     //  _`\/`_  \\
    ||  /\O||O/\  ||
    |\  \_/||\_/  /|
    \ '.   \/   .' /
    / ^ `'~  ~'`   \
   /  _-^_~ -^_ ~-  |    ", csi!(199), "Cargo BashMan", ansi!((cornflower_blue) " v", env!("CARGO_PKG_VERSION")), r"
   | / ^_ -^_- ~_^\ |    A BASH completion script and MAN
   | |~_ ^- _-^_ -| |    page generator for Rust projects.
   | \  ^-~_ ~-_^ / |
   \_/;-.,____,.-;\_/
======(_(_(==)_)_)======

USAGE:
    cargo bashman [FLAGS] [OPTIONS]

FLAGS:
    -h, --help                  Print help information to STDOUT and exit.
        --no-bash               Do not generate BASH completions.
        --no-credits            Do not generate CREDITS.md.
        --no-man                Do not generate MAN page(s).
        --print-targets         Print the supported target triples (for use
                                with -t/--target) to STDOUT and exit.
    -V, --version               Print version information to STDOUT and exit.

OPTIONS:
    -m, --manifest-path <FILE>  Read file paths from this list.
    -t, --target <TRIPLE>       Limit CREDITS.md to dependencies used by the
                                target <TRIPLE>, e.g. x86_64-unknown-linux-gnu.
                                See --print-targets for the supported values.
");



#[derive(Debug, Clone, Eq, PartialEq)]
/// # Errors.
pub(super) enum BashManError {
	/// # Bash Completions.
	Bash,

	/// # Cargo Failed.
	Cargo,

	/// # Credits Failed.
	Credits,

	/// # Directory.
	Dir(&'static str, String),

	/// # Duplicate Key.
	DuplicateKeyWord(KeyWord),

	/// # Keyword.
	KeyWord(String),

	/// # Invalid CLI.
	InvalidCli(String),

	/// # Man Failed.
	Man,

	/// # Multiple Trailing Args.
	MultipleArgs(String),

	/// # Nothing?
	Noop,

	/// # Package Name.
	PackageName(String),

	/// # Cargo Metadata (JSON) Parsing Error.
	ParseCargoMetadata(String),

	/// # Read Error.
	Read(String),

	/// # Unknown Target Triple.
	Target,

	/// # Unknown Subcommand.
	UnknownCommand(String),

	/// # Write Error.
	Write(String),

	/// # Print Help (not really an error).
	PrintHelp,

	/// # Print Targets (not really an error).
	PrintTargets,

	/// # Print Version (not really an error).
	PrintVersion,
}

impl std::error::Error for BashManError {}

impl fmt::Display for BashManError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let s = match self {
			Self::Bash => "Unable to generate bash completions.",
			Self::Cargo => concat!(
				"Unable to execute ",
				dim!("cargo metadata"),
				".",
			),
			Self::Credits => "Unable to generate crate credits.",
			Self::Dir(k, v) => return write!(f, "Invalid {k} directory: {v}"),
			Self::DuplicateKeyWord(k) => return write!(
				f,
				"Duplicate {}: {}",
				k.label(),
				k.as_str(),
			),
			Self::InvalidCli(s) => return write!(f, "Invalid CLI argument: {s}"),
			Self::KeyWord(s) =>
				if s.is_empty() { "Keywords cannot be empty." }
				else { return write!(f, "Invalid keyword: {s}"); },
			Self::Man => "Unable to generate MAN page(s).",
			Self::MultipleArgs(s) =>
				if s.is_empty() { "Multiple trailing arguments defined." }
				else { return write!(f, "Multiple trailing arguments defined: {s}.") },
			Self::Noop => "Nothing to do!",
			Self::PackageName(s) =>
				if s.is_empty() { "Package name cannot be empty." }
				else { return write!(f, "Invalid package name: {s}"); },
			Self::ParseCargoMetadata(s) => return write!(f, "Cargo metadata parsing error: {s}"),
			Self::Read(s) => return write!(f, "Unable to read: {s}"),
			Self::UnknownCommand(s) => return write!(f, "Unknown (sub)command: {s}"),
			Self::Write(s) => return write!(f, "Unable to write: {s}"),
			Self::PrintHelp => HELP,
			Self::Target | Self::PrintTargets => return TargetTriple::print(f),
			Self::PrintVersion => concat!("Cargo BashMan v", env!("CARGO_PKG_VERSION")),
		};
		f.write_str(s)
	}
}
