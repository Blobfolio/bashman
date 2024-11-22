/*!
# Cargo BashMan: Raw Data Parsing.
*/

pub(super) mod keyword;
pub(super) mod pkg;
pub(super) mod target;
mod cargo;
mod util;

use crate::{
	BashManError,
	Dependency,
	KeyWord,
	TargetTriple,
};
use std::{
	cmp::Ordering,
	collections::BTreeSet,
	path::{
		Path,
		PathBuf,
	},
};



#[derive(Debug)]
/// # Package Manifest.
///
/// This includes all of the relevant pieces of data teased out of the
/// raw `Cargo.toml`.
pub(crate) struct Manifest {
	/// # Manifest File.
	src: PathBuf,

	/// # Manifest Directory.
	dir: PathBuf,

	/// # Bash Output Directory.
	dir_bash: Option<PathBuf>,

	/// # Manual Output Directory.
	dir_man: Option<PathBuf>,

	/// # Credits Output Directory.
	dir_credits: Option<PathBuf>,

	/// # Subcommands.
	subcommands: Vec<Subcommand>,

	/// # Target (For Credits).
	target: Option<TargetTriple>,

	/// # Dependencies.
	dependencies: Vec<Dependency>,
}

impl Manifest {
	/// # From File.
	///
	/// Read and parse a `Cargo.toml` file, teasing from it everything we need
	/// to write all the things we might want to write.
	///
	/// This is, of course, monstrous, but nothing compared to the raw
	/// deserialization we had the foresight to separate out into its own
	/// module. Haha.
	pub(crate) fn from_file<P: AsRef<Path>>(src: P, target: Option<TargetTriple>)
	-> Result<Self, BashManError> {
		// Unpack a bunch of shit.
		let (dir, src) = manifest_source(src.as_ref())?;
		let (
			cargo::RawMainPackage { dir_bash, dir_man, dir_credits, subcommands, credits },
			mut deps,
		) = cargo::fetch(&src, target)?;

		// Abosrb the extra credits into the real dependencies.
		deps.extend(credits);

		// Collect into a vec and resort, pushing conditional dependencies to
		// the end of the list.
		let mut dependencies: Vec<Dependency> = deps.into_iter().collect();
		dependencies.sort_by(|a, b| {
			let a_cond = a.conditional();
			let b_cond = b.conditional();

			if a_cond == b_cond { a.cmp(b) }
			else if a_cond { Ordering::Greater }
			else { Ordering::Less }
		});

		// Finally!
		Ok(Self {
			src,
			dir_bash: dir_bash.map(|v| dir.join(v)),
			dir_man: dir_man.map(|v| dir.join(v)),
			dir_credits: dir_credits.map(|v| dir.join(v)),
			dir,
			subcommands,
			target,
			dependencies,
		})
	}

	#[cfg(test)]
	/// # From Dummy.
	///
	/// Like `Manifest::from_file`, but uses a static dataset for testing
	/// purposes.
	pub(crate) fn from_test() -> Result<Self, BashManError> {
		let (dir, src) = manifest_source("skel/metadata.json".as_ref())?;

		let target = TargetTriple::try_from("x86_64-unknown-linux-gnu".to_owned()).ok();
		assert!(target.is_some(), "Target failed.");

		let (
			cargo::RawMainPackage { dir_bash, dir_man, dir_credits, subcommands, credits },
			mut deps,
		) = cargo::fetch_test(target)?;

		// Abosrb the extra credits into the real dependencies.
		deps.extend(credits);

		// Finally!
		Ok(Self {
			src,
			dir_bash: dir_bash.map(|v| dir.join(v)),
			dir_man: dir_man.map(|v| dir.join(v)),
			dir_credits: dir_credits.map(|v| dir.join(v)),
			dir,
			subcommands,
			target,
			dependencies: deps.into_iter().collect(),
		})
	}
}

impl Manifest {
	/// # Dependencies.
	pub(crate) fn dependencies(&self) -> &[Dependency] { &self.dependencies }

	/// # Bash Directory.
	///
	/// Return the directory bash completions should be written to, or an error
	/// if it doesn't exist or is not a directory.
	pub(crate) fn dir_bash(&self) -> Result<PathBuf, BashManError> {
		let has_data =
			1 < self.subcommands.len() ||
			self.subcommands.first().is_some_and(|s| {
				! s.data.flags.is_empty() ||
				! s.data.options.is_empty() ||
				s.data.args.is_some()
			});

		if ! has_data { Err(BashManError::Noop) }
		else if let Some(dir) = self.dir_bash.as_ref() {
			if let Ok(dir) = std::fs::canonicalize(dir) {
				if dir.is_dir() { return Ok(dir); }
			}

			Err(BashManError::Dir("bash completions", dir.to_string_lossy().into_owned()))
		}
		else { Ok(self.dir.clone()) }
	}

	/// # Credits Directory.
	///
	/// Return the directory the crate credits should be written to, or an
	/// error if it doesn't exist or is not a directory.
	pub(crate) fn dir_credits(&self) -> Result<PathBuf, BashManError> {
		if let Some(dir) = self.dir_credits.as_ref() {
			if let Ok(dir) = std::fs::canonicalize(dir) {
				if dir.is_dir() { return Ok(dir); }
			}

			Err(BashManError::Dir("credits", dir.to_string_lossy().into_owned()))
		}
		else { Ok(self.dir.clone()) }
	}

	/// # Manual Directory.
	///
	/// Return the directory bash completions should be written to, or an error
	/// if it doesn't exist or is not a directory.
	pub(crate) fn dir_man(&self) -> Result<PathBuf, BashManError> {
		let has_data =
			1 < self.subcommands.len() ||
			self.subcommands.first().is_some_and(|s| {
				! s.data.flags.is_empty() ||
				! s.data.options.is_empty() ||
				s.data.args.is_some() ||
				! s.data.sections.is_empty()
			});

		if ! has_data { Err(BashManError::Noop) }
		else if let Some(dir) = self.dir_man.as_ref() {
			if let Ok(dir) = std::fs::canonicalize(dir) {
				if dir.is_dir() { return Ok(dir); }
			}

			Err(BashManError::Dir("MAN page", dir.to_string_lossy().into_owned()))
		}
		else { Ok(self.dir.clone()) }
	}

	/// # Main Command.
	pub(crate) fn main_cmd(&self) -> Option<&Subcommand> {
		self.subcommands.iter().find(|s| s.parent.is_none())
	}

	/// # Cargo File.
	pub(crate) fn src(&self) -> &Path { &self.src }

	/// # (Sub)commands.
	pub(crate) fn subcommands(&self) -> &[Subcommand] { self.subcommands.as_slice() }

	/// # Target?
	pub(crate) const fn target(&self) -> Option<TargetTriple> { self.target }
}



#[derive(Debug, Default)]
/// # Manifest Data.
///
/// All the flags and shit.
pub(crate) struct ManifestData {
	/// Boolean Flags.
	flags: BTreeSet<Flag>,

	/// # Option Flags.
	options: BTreeSet<OptionFlag>,

	/// # Trailing Args.
	args: Option<TrailingArg>,

	/// # Extra Sections.
	sections: Vec<Section>,
}

impl ManifestData {
	/// # Args.
	pub(crate) const fn args(&self) -> Option<&TrailingArg> { self.args.as_ref() }

	/// # Flags.
	pub(crate) const fn flags(&self) -> &BTreeSet<Flag> { &self.flags }

	/// # Option Flags.
	pub(crate) const fn options(&self) -> &BTreeSet<OptionFlag> { &self.options }

	/// # Sections.
	pub(crate) fn sections(&self) -> &[Section] { &self.sections }
}



#[derive(Debug)]
/// # Subcommand.
pub(crate) struct Subcommand {
	/// # Nice Name.
	nice_name: Option<String>,

	/// # Command.
	name: KeyWord,

	/// # Description.
	description: String,

	/// # Version.
	version: String,

	/// # Parent?
	parent: Option<(String, KeyWord)>,

	/// # Data.
	data: ManifestData,
}

impl Subcommand {
	/// # Bin.
	pub(crate) fn bin(&self) -> &str { self.name.as_str() }

	/// # Data.
	pub(crate) const fn data(&self) -> &ManifestData { &self.data }

	/// # Description.
	pub(crate) fn description(&self) -> &str { &self.description }

	/// # Is Main?
	pub(crate) const fn is_main(&self) -> bool { self.parent.is_none() }

	/// # Nice Name.
	pub(crate) fn nice_name(&self) -> &str {
		self.nice_name.as_deref().unwrap_or_else(|| self.name.as_str())
	}

	/// # Parent Bin.
	pub(crate) fn parent_bin(&self) -> Option<&str> {
		self.parent.as_ref().map(|(_, k)| k.as_str())
	}

	/// # Parent Nice Name.
	pub(crate) fn parent_nice_name(&self) -> Option<&str> {
		self.parent.as_ref().map(|(k, _)| k.as_str())
	}

	/// # Version.
	pub(crate) fn version(&self) -> &str { self.version.as_str() }
}



#[derive(Debug, Clone)]
/// # Flag.
pub(crate) struct Flag {
	/// # Short.
	short: Option<KeyWord>,

	/// # Long.
	long: Option<KeyWord>,

	/// # Description.
	description: String,

	/// # Allow Duplicate?
	duplicate: bool,
}

impl Eq for Flag {}

impl Ord for Flag {
	fn cmp(&self, other: &Self) -> Ordering {
		let a = self.sort_key();
		let b = other.sort_key();

		// Compare case-insensitively, unless we need it for a tie-breaker.
		match a.bytes().map(|b| b.to_ascii_lowercase()).cmp(b.bytes().map(|b| b.to_ascii_lowercase())) {
			Ordering::Equal => a.cmp(b),
			cmp => cmp,
		}
	}
}

impl PartialEq for Flag {
	#[inline]
	fn eq(&self, other: &Self) -> bool { self.sort_key() == other.sort_key() }
}

impl PartialOrd for Flag {
	#[inline]
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl Flag {
	/// # Sort Key.
	///
	/// Return the non-dashed portion of the short or long key to give us
	/// something cheap to sort by.
	fn sort_key(&self) -> &str {
		// Prefer the long key, fall back to the short one.
		self.long.as_ref()
			.or(self.short.as_ref())
			.map_or("", |s| s.as_str().trim_start_matches('-'))
	}
}

impl Flag {
	/// # Description.
	pub(crate) fn description(&self) -> &str { &self.description }

	/// # Duplicate?
	pub(crate) const fn duplicate(&self) -> bool { self.duplicate }

	/// # Long Key.
	pub(crate) fn long(&self) -> Option<&str> { self.long.as_ref().map(KeyWord::as_str) }

	/// # Short Key.
	pub(crate) fn short(&self) -> Option<&str> { self.short.as_ref().map(KeyWord::as_str) }
}



#[derive(Debug, Clone)]
/// # Option Flag.
pub(crate) struct OptionFlag {
	/// # Flag.
	flag: Flag,

	/// # Label Name.
	label: String,

	/// # Path Value?
	path: bool,
}

impl Eq for OptionFlag {}

impl Ord for OptionFlag {
	#[inline]
	fn cmp(&self, other: &Self) -> Ordering { self.flag.cmp(&other.flag) }
}

impl PartialEq for OptionFlag {
	#[inline]
	fn eq(&self, other: &Self) -> bool { self.flag == other.flag }
}

impl PartialOrd for OptionFlag {
	#[inline]
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl OptionFlag {
	/// # Duplicate?
	pub(crate) const fn duplicate(&self) -> bool { self.flag.duplicate() }

	/// # Description.
	pub(crate) fn description(&self) -> &str { self.flag.description() }

	/// # Label.
	pub(crate) fn label(&self) -> &str { &self.label }

	/// # Long Key.
	pub(crate) fn long(&self) -> Option<&str> { self.flag.long() }

	/// # Path Value?
	pub(crate) const fn path(&self) -> bool { self.path }

	/// # Short Key.
	pub(crate) fn short(&self) -> Option<&str> { self.flag.short() }
}



#[derive(Debug, Clone)]
/// # Trailing Argument.
pub(crate) struct TrailingArg {
	/// # Label.
	label: String,

	/// # Description.
	description: String,
}

impl Eq for TrailingArg {}

impl Ord for TrailingArg {
	#[inline]
	fn cmp(&self, other: &Self) -> Ordering { self.label.cmp(&other.label) }
}

impl PartialEq for TrailingArg {
	#[inline]
	fn eq(&self, other: &Self) -> bool { self.label == other.label }
}

impl PartialOrd for TrailingArg {
	#[inline]
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl TrailingArg {
	/// # Description.
	pub(super) fn description(&self) -> &str { &self.description }

	/// # Label.
	pub(super) fn label(&self) -> &str { &self.label }
}



#[derive(Debug, Clone)]
/// # Extra Section.
pub(crate) struct Section {
	/// # Name.
	name: String,

	/// # Indent?
	inside: bool,

	/// # Lines.
	lines: String,

	/// # Key/Value Pairs.
	items: Vec<[String; 2]>,
}

impl Section {
	/// # Inside?
	pub(super) const fn inside(&self) -> bool { self.inside }

	/// # Items?
	pub(super) fn items(&self) -> Option<&[[String; 2]]> {
		if self.items.is_empty() { None }
		else { Some(self.items.as_slice()) }
	}

	/// # Lines?
	pub(super) fn lines(&self) -> Option<&str> {
		if self.lines.is_empty() { None }
		else { Some(self.lines.as_str()) }
	}

	/// # Name.
	pub(super) fn name(&self) -> &str { &self.name }
}



/// # Manifest Source Directory and File.
///
/// The source path used to initialize a new `Manifest` might be a file or
/// directory. We actually need both, so figure out which it is, infer the
/// other, and return them.
fn manifest_source(src: &Path) -> Result<(PathBuf, PathBuf), BashManError> {
	// The source should exist.
	let mut src = std::fs::canonicalize(src)
		.map_err(|_| BashManError::Read(src.to_string_lossy().into_owned()))?;

	let dir =
		// If it is a directory, infer the file.
		if src.is_dir() {
			let tmp = src.join("Cargo.toml");
			std::mem::replace(&mut src, tmp)
		}
		// If it is a file, infer the directory.
		else {
			src.parent()
				.ok_or_else(|| BashManError::Read(src.to_string_lossy().into_owned()))?
				.to_path_buf()
		};

	// Additional error checking will come later!
	Ok((dir, src))
}
