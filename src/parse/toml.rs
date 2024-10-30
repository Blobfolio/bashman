/*!
# Cargo BashMan: Raw TOML Parsing.

TOML is a terrible format to work with. This module exists to quarantine the
mess as much as possible. Haha.
*/

use crate::{
	BashManError,
	KeyWord,
	PackageName,
};
use semver::Version;
use serde::{
	Deserialize,
	Deserializer,
};
use std::{
	collections::{
		BTreeMap,
		BTreeSet,
	},
	path::Path,
};
use super::util;
use trimothy::NormalizeWhitespace;



#[derive(Debug, Deserialize)]
/// # Top Level Struct.
///
/// The only things we care about are `package.*`.
pub(super) struct Raw {
	/// # Package Details.
	pub(super) package: RawPackage,
}

impl Raw {
	/// # From File.
	///
	/// Read and parse a TOML file.
	pub(super) fn from_file<P: AsRef<Path>>(src: P) -> Result<Self, BashManError> {
		let src: &Path = src.as_ref();
		let raw = std::fs::read_to_string(src)
			.map_err(|_| BashManError::Read(src.to_string_lossy().into_owned()))?;
		Self::from_toml(&raw)
	}

	/// # From TOML.
	///
	/// Parse the contents of a TOML file into our raw structures, cleaning a
	/// few of the more tedious things while we're here.
	pub(super) fn from_toml(src: &str) -> Result<Self, BashManError> {
		let mut out: Self = toml::from_str(src)
			.map_err(|e| BashManError::ParseToml(e.to_string()))?;

		// Prune flags that are missing keys.
		out.package.metadata.flags.retain(|s| s.short.is_some() || s.long.is_some());
		out.package.metadata.options.retain(|s| s.short.is_some() || s.long.is_some());

		// Prune sections that are missing text.
		out.package.metadata.sections.retain(|s| ! s.lines.is_empty() || ! s.items.is_empty());

		// Populate empty subcommand lists with an empty string, which is what
		// we use for top-level stuff.
		let iter = out.package.metadata.flags.iter_mut().map(|s| &mut s.subcommands)
			.chain(out.package.metadata.options.iter_mut().map(|s| &mut s.subcommands))
			.chain(out.package.metadata.args.iter_mut().map(|s| &mut s.subcommands));
		for v in iter {
			if v.is_empty() { v.insert(String::new()); }
		}

		// Check for duplicate subcommands.
		let mut subs = BTreeMap::<&str, BTreeSet<&KeyWord>>::new();
		subs.insert("", BTreeSet::new());
		for e in &out.package.metadata.subcommands {
			if subs.insert(e.cmd.as_str(), BTreeSet::new()).is_some() {
				return Err(BashManError::DuplicateKeyWord(e.cmd.clone()));
			}
		}

		// Check for duplicate keys.
		let iter = out.package.metadata.flags.iter().map(|f| (f.short.as_ref(), f.long.as_ref(), &f.subcommands))
			.chain(out.package.metadata.options.iter().map(|f| (f.short.as_ref(), f.long.as_ref(), &f.subcommands)));
		for (short, long, flag_subs) in iter {
			for s in flag_subs {
				let entry = subs.get_mut(s.as_str())
					.ok_or_else(|| BashManError::UnknownCommand(s.clone()))?;
				for key in [short, long].into_iter().flatten() {
					if ! entry.insert(key) {
						return Err(BashManError::DuplicateKeyWord(key.clone()))?;
					}
				}
			}
		}

		Ok(out)
	}
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Package Data.
///
/// This is what is found under "package".
pub(super) struct RawPackage {
	/// # Package Name.
	pub(super) name: PackageName,

	/// # Package Version.
	pub(super) version: Version,

	#[serde(deserialize_with = "util::deserialize_nonempty_str_normalized")]
	/// # Package Description.
	pub(super) description: String,

	#[serde(with = "RawMeta")]
	/// # Bashman Metadata.
	pub(super) metadata: RawBashMan,
}



#[derive(Deserialize)]
/// # Raw Package Metadata (Wrapper).
///
/// We don't care about metadata beyond "package.metadata.bashman"; this
/// removes a level of complexity.
struct RawMeta<T> {
	/// # Bashman Key.
	bashman: T,
}

impl<T> RawMeta<T> {
	#[inline]
	/// # Deserialize.
	fn deserialize<'de, D>(deserializer: D) -> Result<T, D::Error>
	where T: Deserialize<'de>, D: Deserializer<'de> {
		<Self as Deserialize>::deserialize(deserializer).map(|w| w.bashman)
	}
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Package Metadata (bashman).
///
/// This is what is found under "package.metadata.bashman".
pub(super) struct RawBashMan {
	#[serde(rename = "name")]
	#[serde(default)]
	#[serde(deserialize_with = "util::deserialize_nonempty_opt_str_normalized")]
	/// # Package Nice Name.
	pub(super) nice_name: Option<String>,

	#[serde(rename = "bash-dir")]
	#[serde(default)]
	#[serde(deserialize_with = "util::deserialize_nonempty_opt_str")]
	/// # Directory For Bash Completions.
	pub(super) dir_bash: Option<String>,

	#[serde(rename = "man-dir")]
	#[serde(default)]
	#[serde(deserialize_with = "util::deserialize_nonempty_opt_str")]
	/// # Directory for MAN pages.
	pub(super) dir_man: Option<String>,

	#[serde(rename = "credits-dir")]
	#[serde(default)]
	#[serde(deserialize_with = "util::deserialize_nonempty_opt_str")]
	/// # Directory for Credits.
	pub(super) dir_credits: Option<String>,

	#[serde(default)]
	/// # Subcommands.
	pub(super) subcommands: Vec<RawSubCmd>,

	#[serde(rename = "switches")]
	#[serde(default)]
	/// # Switches.
	pub(super) flags: Vec<RawSwitch>,

	#[serde(default)]
	/// # Options.
	pub(super) options: Vec<RawOption>,

	#[serde(rename = "arguments")]
	#[serde(default)]
	/// # Arguments.
	pub(super) args: Vec<RawArg>,

	#[serde(default)]
	/// # Sections.
	pub(super) sections: Vec<RawSection>,
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Subcommand.
///
/// This is what is found under "package.metadata.bashman.subcommands".
pub(super) struct RawSubCmd {
	#[serde(default)]
	#[serde(deserialize_with = "util::deserialize_nonempty_opt_str_normalized")]
	/// # Nice Name.
	pub(super) name: Option<String>,

	/// # (Sub)command.
	pub(super) cmd: KeyWord,

	#[serde(deserialize_with = "util::deserialize_nonempty_str_normalized")]
	/// # Description.
	pub(super) description: String,
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Switch.
///
/// This is what is found under "package.metadata.bashman.switches".
pub(super) struct RawSwitch {
	#[serde(default)]
	/// # Short Key.
	pub(super) short: Option<KeyWord>,

	#[serde(default)]
	/// # Long Key.
	pub(super) long: Option<KeyWord>,

	#[serde(deserialize_with = "util::deserialize_nonempty_str_normalized")]
	/// # Description.
	pub(super) description: String,

	#[serde(default)]
	/// # Allow Duplicates.
	pub(super) duplicate: bool,

	#[serde(default)]
	/// # Applicable (Sub)commands.
	pub(super) subcommands: BTreeSet<String>,
}



#[derive(Debug, Clone, Deserialize)]
/// Raw Option.
///
/// This is what is found under "package.metadata.bashman.options".
pub(super) struct RawOption {
	#[serde(default)]
	/// # Short Key.
	pub(super) short: Option<KeyWord>,

	#[serde(default)]
	/// # Long Key.
	pub(super) long: Option<KeyWord>,

	#[serde(deserialize_with = "util::deserialize_nonempty_str_normalized")]
	/// # Description.
	pub(super) description: String,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_label")]
	/// # Value Label.
	pub(super) label: Option<String>,

	#[serde(default)]
	/// # Value is Path?
	pub(super) path: bool,

	#[serde(default)]
	/// # Allow Duplicates.
	pub(super) duplicate: bool,

	#[serde(default)]
	/// # Applicable (Sub)commands.
	pub(super) subcommands: BTreeSet<String>,
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Argument.
///
/// This is what is found under "package.metadata.bashman.arguments".
pub(super) struct RawArg {
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_label")]
	/// # Value Label.
	pub(super) label: Option<String>,

	#[serde(deserialize_with = "util::deserialize_nonempty_str_normalized")]
	/// # Description.
	pub(super) description: String,

	#[serde(default)]
	/// # Applicable (Sub)commands.
	pub(super) subcommands: BTreeSet<String>,
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Section.
///
/// This is what is found under "package.metadata.bashman.subcommands".
pub(super) struct RawSection {
	#[serde(deserialize_with = "deserialize_section_name")]
	/// # Section Name.
	pub(super) name: String,

	#[serde(default)]
	/// # Indent?
	pub(super) inside: bool,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_lines")]
	/// # Text Lines.
	pub(super) lines: Vec<String>,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_items")]
	/// # Text Bullets.
	pub(super) items: Vec<[String; 2]>
}



#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: Section Items.
fn deserialize_items<'de, D>(deserializer: D) -> Result<Vec<[String; 2]>, D::Error>
where D: Deserializer<'de> {
	let mut out = Vec::<[String; 2]>::deserialize(deserializer).unwrap_or_default();
	out.retain_mut(|line| {
		util::normalize_string(&mut line[0]);
		util::normalize_string(&mut line[1]);
		! line[0].is_empty() || ! line[1].is_empty()
	});

	Ok(out)
}

#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: Optional Option/Arg Label.
///
/// This will return `None` if the string is empty.
fn deserialize_label<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where D: Deserializer<'de> {
	Ok(
		<String>::deserialize(deserializer).ok()
			.and_then(|mut x| {
				util::normalize_string(&mut x);
				if x.is_empty() { None }
				else {
					if ! x.starts_with('<') { x.insert(0, '<'); }
					if ! x.ends_with('>') { x.push('>'); }
					Some(x)
				}
			})
	)
}

#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: Section Lines.
fn deserialize_lines<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where D: Deserializer<'de> {
	let mut out = Vec::<String>::deserialize(deserializer).unwrap_or_default();
	let mut any = false;
	out.retain_mut(|line| {
		util::normalize_string(line);
		if line.is_empty() && ! any { false }
		else {
			any = true;
			true
		}
	});

	// Remove trailing empty lines.
	while out.last().filter(|v| v.is_empty()).is_some() {
		out.truncate(out.len() - 1);
	}

	Ok(out)
}

/// # Deserialize: Section Name.
///
/// This will return an error if a string is present but empty.
fn deserialize_section_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where D: Deserializer<'de> {
	let tmp = <String>::deserialize(deserializer)?;
	let mut out: String = tmp.normalized_control_and_whitespace()
		.flat_map(char::to_uppercase)
		.collect();

	let last = out.chars().last()
		.ok_or_else(|| serde::de::Error::custom("value cannot be empty"))?;
	if ! last.is_ascii_punctuation() { out.push(':'); }
	Ok(out)
}
