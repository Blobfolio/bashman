/*!
# `Cargo BashMan` - Error
*/

use std::{
	fmt,
	path::PathBuf,
};



#[derive(Debug, Clone)]
/// # Error.
pub enum BashManError {
	/// # Bash directory is bad.
	InvalidBashDir,
	/// # Manual directory is bad.
	InvalidManDir,
	/// # Invalid manifest.
	InvalidManifest,
	/// # General invalid path.
	InvalidPath(PathBuf),
	/// # Missing package section.
	MissingPackage,
	/// # Missing package.metadata section.
	MissingPackageMeta,
	/// # Unable to parse manifest.
	ParseManifest,
	/// # Unable to write Bash completions.
	WriteBash(PathBuf),
	/// # Unable to write Manual.
	WriteMan(PathBuf),
}

impl fmt::Display for BashManError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::InvalidBashDir => f.write_str("Invalid BASH output directory."),
			Self::InvalidManDir => f.write_str("Invalid MAN output directory."),
			Self::InvalidManifest => f.write_str("Invalid manifest."),
			Self::InvalidPath(path) => f.write_fmt(format_args!("Invalid path: {:?}", path)),
			Self::MissingPackage => f.write_str("Missing [package] section."),
			Self::MissingPackageMeta => f.write_str("Missing [package.metadata.bashman] section."),
			Self::ParseManifest => f.write_str("Unable to parse manifest."),
			Self::WriteBash(path) => f.write_fmt(format_args!("Unable to write BASH completions to: {:?}", path)),
			Self::WriteMan(path) => f.write_fmt(format_args!("Unable to write MAN to: {:?}", path)),
		}
	}
}
