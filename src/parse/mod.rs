/*!
# Cargo BashMan: Raw Data Parsing.
*/

pub(super) mod keyword;
pub(super) mod pkg;
pub(super) mod target;
mod metadata;
mod toml;
mod util;

use crate::{
	BashManError,
	Dependency,
	KeyWord,
	TargetTriple,
};
use std::{
	cmp::Ordering,
	collections::{
		BTreeMap,
		BTreeSet,
	},
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

	/// # Extra Credits?
	credits: BTreeSet<Dependency>,
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
	pub(crate) fn from_file<P: AsRef<Path>>(src: P) -> Result<Self, BashManError> {
		// Unpack a ton of shit.
		let (dir, src) = manifest_source(src.as_ref())?;
		let toml::Raw { package } = toml::Raw::from_file(&src)?;
		let toml::RawPackage { name, version, description, metadata } = package;
		let toml::RawBashMan { nice_name, dir_bash, dir_man, dir_credits, subcommands, flags, options, args, sections, credits } = metadata;

		// Build the subcommands.
		let mut subs = BTreeMap::<String, Subcommand>::new();
		let main = Subcommand {
			nice_name,
			name: KeyWord::from(name),
			description,
			version: version.to_string(),
			parent: None,
			data: ManifestData::default(),
		};
		for raw in subcommands {
			let sub = Subcommand::from_raw(
				raw,
				main.version.clone(),
				Some((main.nice_name().to_owned(), main.name.clone())),
			);
			subs.insert(sub.name.as_str().to_owned(), sub);
		}
		subs.insert(String::new(), main);

		// Add Flags.
		for (flag, mut subcommands) in flags.into_iter().map(Flag::from_raw) {
			// Process the last subcommand separately so we can save a clone.
			if let Some(last) = subcommands.pop_last() {
				for s in subcommands {
					add_subcommand_flag(&mut subs, s, flag.clone())?;
				}
				add_subcommand_flag(&mut subs, last, flag)?;
			}
		}

		// Add Options.
		for (flag, mut subcommands) in options.into_iter().map(OptionFlag::from_raw) {
			// Process the last subcommand separately so we can save a clone.
			if let Some(last) = subcommands.pop_last() {
				for s in subcommands {
					add_subcommand_option(&mut subs, s, flag.clone())?;
				}
				add_subcommand_option(&mut subs, last, flag)?;
			}
		}

		// Add Args.
		for (flag, mut subcommands) in args.into_iter().map(TrailingArg::from_raw) {
			// Process the last subcommand separately so we can save a clone.
			if let Some(last) = subcommands.pop_last() {
				for s in subcommands {
					add_subcommand_arg(&mut subs, s, flag.clone())?;
				}
				add_subcommand_arg(&mut subs, last, flag)?;
			}
		}

		// Sections go everywhere.
		if ! sections.is_empty() {
			let sections: Vec<_> = sections.into_iter().map(Section::from).collect();
			for s in subs.values_mut() {
				s.data.sections.extend_from_slice(&sections);
			}
		}

		// Finally!
		Ok(Self {
			src,
			dir_bash: dir_bash.map(|v| dir.join(v)),
			dir_man: dir_man.map(|v| dir.join(v)),
			dir_credits: dir_credits.map(|v| dir.join(v)),
			dir,
			subcommands: subs.into_values().collect(),
			credits: credits.into_iter().map(Dependency::from).collect(),
		})
	}
}

impl Manifest {
	/// # Bash Directory.
	///
	/// Return the directory bash completions should be written to, or an error
	/// if it doesn't exist or is not a directory.
	pub(crate) fn dir_bash(&self) -> Result<PathBuf, BashManError> {
		if let Some(dir) = self.dir_bash.as_ref() {
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
		if let Some(dir) = self.dir_man.as_ref() {
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
}

impl Manifest {
	#[inline]
	/// # Fetch Dependencies.
	///
	/// Run `cargo metadata` to figure out what all dependencies are in the
	/// tree and return them, or an error if it fails.
	pub(crate) fn dependencies(&self, target: Option<TargetTriple>)
	-> Result<Vec<Dependency>, BashManError> {
		let src: &Path = self.src();

		// Fetch the required dependencies first.
		let mut out = metadata::fetch_dependencies(src, false, target)?;

		// Try again with all features enabled and add anything extra under
		// the assumption that they're optional. If this fails, we'll stick
		// with what we've already found.
		if let Ok(all) = metadata::fetch_dependencies(src, true, target) {
			if out.len() < all.len() {
				for mut dep in all {
					dep.context |= Dependency::FLAG_OPTIONAL;
					out.insert(dep);
				}
			}
		}

		// Add any custom entries.
		out.extend(self.credits.iter().cloned());

		// Done!
		Ok(out.into_iter().collect())
	}
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
	/// # From Raw.
	fn from_raw(raw: toml::RawSubCmd, version: String, parent: Option<(String, KeyWord)>)
	-> Self {
		Self {
			nice_name: raw.name,
			name: raw.cmd,
			description: raw.description,
			version,
			parent,
			data: ManifestData::default(),
		}
	}
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
	/// # From Raw.
	///
	/// Return self along with whatever subcommands apply, if any.
	fn from_raw(raw: toml::RawSwitch) -> (Self, BTreeSet<String>) {
		let toml::RawSwitch { short, long, description, duplicate, subcommands } = raw;
		(Self { short, long, description, duplicate }, subcommands)
	}

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
	/// # From Raw.
	///
	/// Return self along with whatever subcommands apply, if any.
	fn from_raw(raw: toml::RawOption) -> (Self, BTreeSet<String>) {
		let toml::RawOption { short, long, description, label, path, duplicate, subcommands } = raw;
		(
			Self {
				flag: Flag { short, long, description, duplicate },
				label: label.unwrap_or_else(|| "<VAL>".to_owned()),
				path,
			},
			subcommands,
		)
	}
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
	/// # From Raw.
	///
	/// Return self along with whatever subcommands apply, if any.
	fn from_raw(raw: toml::RawArg) -> (Self, BTreeSet<String>) {
		let toml::RawArg { label, description, subcommands } = raw;
		(
			Self {
				label: label.unwrap_or_else(|| "<ARG(S)…>".to_owned()),
				description,
			},
			subcommands,
		)
	}
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

impl From<toml::RawSection> for Section {
	#[inline]
	fn from(raw: toml::RawSection) -> Self {
		Self {
			name: raw.name,
			inside: raw.inside,
			lines: if raw.lines.is_empty() { String::new() } else { raw.lines.join("\n.RE\n") },
			items: raw.items,
		}
	}
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



/// # Add Subcommand Flag.
fn add_subcommand_flag(subs: &mut BTreeMap<String, Subcommand>, key: String, flag: Flag)
-> Result<(), BashManError> {
	subs.get_mut(&key)
		.ok_or(BashManError::UnknownCommand(key))?
		.data
		.flags
		.insert(flag);
	Ok(())
}

/// # Add Subcommand Option Flag.
fn add_subcommand_option(
	subs: &mut BTreeMap<String, Subcommand>,
	key: String,
	flag: OptionFlag,
) -> Result<(), BashManError> {
	subs.get_mut(&key)
		.ok_or(BashManError::UnknownCommand(key))?
		.data
		.options
		.insert(flag);
	Ok(())
}

/// # Add Subcommand Trailing Arg.
fn add_subcommand_arg(
	subs: &mut BTreeMap<String, Subcommand>,
	key: String,
	flag: TrailingArg,
) -> Result<(), BashManError> {
	let res = subs.get_mut(&key)
		.ok_or_else(|| BashManError::UnknownCommand(key.clone()))?
		.data
		.args
		.replace(flag)
		.is_none();

	if res { Ok(()) }
	else { Err(BashManError::MultipleArgs(key)) }
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



#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn t_manifest() {
		let manifest = match Manifest::from_file("skel/test.toml") {
			Ok(m) => m,
			Err(e) => panic!("{e}"),
		};

		// Let's test a few basic properties of the subcommands.
		let mut iter = manifest.subcommands.iter();

		// The main command should be first even though it is alphabetically
		// second.
		let next = iter.next().expect("Missing manifest subcommand.");
		assert_eq!(next.name.as_str(), "bashman");
		assert_eq!(next.version, "0.5.2");
		assert_eq!(next.data.flags.len(), 1);
		assert_eq!(next.data.options.len(), 1);
		assert!(next.data.args.is_some());
		assert_eq!(next.data.sections.len(), 2);

		// This is third in the file, but should be second because sorting
		// matters for the actual subcommands.
		let next = iter.next().expect("Missing manifest subcommand.");
		assert_eq!(next.name.as_str(), "action");
		assert_eq!(next.data.flags.len(), 2);
		assert_eq!(next.data.options.len(), 1);
		assert!(next.data.args.is_none());
		assert_eq!(next.data.sections.len(), 2);

		// One more!
		let next = iter.next().expect("Missing manifest subcommand.");
		assert_eq!(next.name.as_str(), "make");
		assert_eq!(next.data.flags.len(), 4);
		assert_eq!(next.data.options.len(), 1);
		assert!(next.data.args.is_none());
		assert_eq!(next.data.sections.len(), 2);

		// That should be it.
		assert!(iter.next().is_none());
	}
}
