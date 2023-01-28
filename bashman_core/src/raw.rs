/*!
# `Cargo BashMan` — Raw Data

This module contains intermediate data structures to hold the raw data from
Cargo.toml, namely for the purpose of utilizing [`serde`] deserialization.

From here, a proper [`Command`] is created, which contains the same data in a
form more amenable to BASH/Man generation.

The windy web of static references is a bit of a hot mess, but more than halves
the number of allocations resulting from attempting the feat more directly.

So be it.
*/

use crate::{
	BashManError,
	Command,
	DataFlag,
	DataItem,
	DataKind,
	DataOption,
	More,
};
use indexmap::IndexMap;
use serde::{
	Deserialize,
	Deserializer,
};
use std::path::{
	Path,
	PathBuf,
};
use trimothy::TrimMut;



#[derive(Debug, Clone, Deserialize)]
/// # Raw Data.
///
/// The outermost struct.
pub(super) struct Raw {
	package: RawPackage
}

impl TryFrom<String> for Raw {
	type Error = BashManError;

	fn try_from(src: String) -> Result<Self, Self::Error> {
		toml::from_str(&src)
			.map_err(|e| BashManError::ParseManifest(Box::from(e.to_string())))
	}
}

/// # Conversion.
///
/// Working around the static lifetimes is terrible, but this eventually gets
/// there!
impl Raw {
	/// # Parse Single.
	///
	/// This parses without subcommand support.
	fn parse_single(&self) -> Result<Command<'_>, BashManError> {
		// Switches.
		let out_args: Vec<DataKind<'_>> = self.package.metadata.switches.iter()
			.map(DataKind::try_from)
			.chain(
				self.package.metadata.options.iter().map(DataKind::try_from)
			)
			.chain(
				self.package.metadata.arguments.iter().map(|y| Ok(DataKind::from(y)))
			)
			.collect::<Result<_, _>>()?;

		// Finally return the whole thing!
		Ok(Command::new(
			self.name(),
			None,
			&self.package.name,
			&self.package.version,
			&self.package.description,
			out_args,
			self.sections()?,
		))
	}
}

/// # Getters.
impl Raw {
	/// # Bash Directory.
	pub(super) fn bash_dir(&self, dir: &Path) -> Result<PathBuf, BashManError> {
		let path: PathBuf = self.package.metadata.bash_dir
			.as_ref()
			.map_or_else(|| dir.to_path_buf(), |path|
				if path.starts_with('/') { PathBuf::from(path) }
				else {
					let mut tmp = dir.to_path_buf();
					tmp.push(path);
					tmp
				}
			);

		if path.is_dir() {
			std::fs::canonicalize(path).map_err(|_| BashManError::InvalidBashDir)
		}
		else {
			Err(BashManError::InvalidBashDir)
		}
	}

	/// # Credits Directory.
	pub(super) fn credits_dir(&self, dir: &Path) -> Result<PathBuf, BashManError> {
		let path: PathBuf = self.package.metadata.credits_dir
			.as_ref()
			.map_or_else(|| dir.to_path_buf(), |path|
				if path.starts_with('/') { PathBuf::from(path) }
				else {
					let mut tmp = dir.to_path_buf();
					tmp.push(path);
					tmp
				}
			);

		if path.is_dir() {
			std::fs::canonicalize(path).map_err(|_| BashManError::InvalidCreditsDir)
		}
		else {
			Err(BashManError::InvalidCreditsDir)
		}
	}

	/// # Man Directory.
	pub(super) fn man_dir(&self, dir: &Path) -> Result<PathBuf, BashManError> {
		let path: PathBuf = self.package.metadata.man_dir
			.as_ref()
			.map_or_else(|| dir.to_path_buf(), |path|
				if path.starts_with('/') { PathBuf::from(path) }
				else {
					let mut tmp = dir.to_path_buf();
					tmp.push(path);
					tmp
				}
			);

		if path.is_dir() {
			std::fs::canonicalize(path).map_err(|_| BashManError::InvalidManDir)
		}
		else {
			Err(BashManError::InvalidManDir)
		}
	}

	#[must_use]
	/// # Name.
	fn name(&self) -> &str {
		self.package.metadata.name.as_deref().unwrap_or(&self.package.name)
	}

	/// # Sections.
	fn sections(&self) -> Result<Vec<More<'_>>, BashManError> {
		self.package.metadata.sections.iter()
			.map(More::try_from)
			.collect::<Result<_, _>>()
	}
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Package Data.
///
/// This is what is found under "package".
struct RawPackage {
	#[serde(deserialize_with = "deserialize_nonempty_str")]
	name: String,

	#[serde(deserialize_with = "deserialize_nonempty_str")]
	version: String,

	#[serde(deserialize_with = "deserialize_nonempty_str")]
	description: String,

	#[serde(with = "RawMeta")]
	metadata: RawBashMan,
}



#[derive(Deserialize)]
/// # Wrapper.
///
/// We don't care about metadata beyond metadata.bashman. This removes a level
/// of complexity.
struct RawMeta<T> {
	bashman: T,
}

impl<T> RawMeta<T> {
	fn deserialize<'de, D>(deserializer: D) -> Result<T, D::Error>
	where T: Deserialize<'de>, D: Deserializer<'de> {
		let wrapper = <Self as Deserialize>::deserialize(deserializer)?;
		Ok(wrapper.bashman)
	}
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Package Metadata (bashman).
///
/// This is what is found under "package.metadata.bashman".
struct RawBashMan {
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	name: Option<String>,

	#[serde(rename = "bash-dir")]
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	bash_dir: Option<String>,

	#[serde(rename = "man-dir")]
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	man_dir: Option<String>,

	#[serde(rename = "credits-dir")]
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	credits_dir: Option<String>,

	#[serde(default)]
	subcommands: Vec<RawSubCmd>,

	#[serde(default)]
	switches: Vec<RawSwitch>,

	#[serde(default)]
	options: Vec<RawOption>,

	#[serde(default)]
	arguments: Vec<RawArg>,

	#[serde(default)]
	sections: Vec<RawSection>,
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Subcommand.
///
/// This is what is found under "package.metadata.bashman.subcommands".
struct RawSubCmd {
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	name: Option<String>,

	#[serde(deserialize_with = "deserialize_nonempty_str")]
	cmd: String,

	#[serde(deserialize_with = "deserialize_nonempty_str")]
	description: String,
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Switch.
///
/// This is what is found under "package.metadata.bashman.switches".
struct RawSwitch {
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	short: Option<String>,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	long: Option<String>,

	#[serde(deserialize_with = "deserialize_nonempty_str")]
	description: String,

	#[serde(default)]
	duplicate: bool,

	#[serde(default)]
	subcommands: Vec<String>,
}



#[derive(Debug, Clone, Deserialize)]
/// Raw Option.
///
/// This is what is found under "package.metadata.bashman.options".
struct RawOption {
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	short: Option<String>,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	long: Option<String>,

	#[serde(deserialize_with = "deserialize_nonempty_str")]
	description: String,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	label: Option<String>,

	#[serde(default)]
	path: bool,

	#[serde(default)]
	duplicate: bool,

	#[serde(default)]
	subcommands: Vec<String>,
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Argument.
///
/// This is what is found under "package.metadata.bashman.arguments".
struct RawArg {
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	label: Option<String>,

	#[serde(deserialize_with = "deserialize_nonempty_str")]
	description: String,

	#[serde(default)]
	subcommands: Vec<String>,
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Section.
///
/// This is what is found under "package.metadata.bashman.subcommands".
struct RawSection {
	#[serde(deserialize_with = "deserialize_nonempty_str")]
	name: String,

	#[serde(default)]
	inside: bool,

	#[serde(default)]
	lines: Vec<String>,

	#[serde(default)]
	items: Vec<[String; 2]>
}



/// # Deserialize: Require Non-Empty Str
///
/// This will return an error if a string is present but empty.
fn deserialize_nonempty_str<'de, D>(deserializer: D) -> Result<String, D::Error>
where D: Deserializer<'de> {
	let mut wrapper = String::deserialize(deserializer)?;
	wrapper.trim_mut();
	if wrapper.is_empty() { Err(serde::de::Error::custom("Value cannot be empty.")) }
	else { Ok(wrapper) }
}

#[allow(clippy::unnecessary_wraps)] // Not our structure to decide.
/// # Deserialize: Require Non-Empty Str
///
/// This will return `None` if the string is empty.
fn deserialize_nonempty_opt_str<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where D: Deserializer<'de> {
	Ok(
		Option::<String>::deserialize(deserializer)
			.ok()
			.flatten()
			.and_then(|mut x| {
				x.trim_mut();
				if x.is_empty() { None }
				else { Some(x) }
			})
	)
}



impl<'a> TryFrom<&'a Raw> for Command<'a> {
	type Error = BashManError;

	fn try_from(src: &'a Raw) -> Result<Self, Self::Error> {
		// We can process data more directly if there are no subcommands to
		// worry about.
		if src.package.metadata.subcommands.is_empty() {
			return src.parse_single();
		}

		let mut subcmds: IndexMap<&'_ str, (&'_ str, &'_ str, &'_ str, Vec::<DataKind<'_>>)> = src.package.metadata.subcommands.iter()
			.map(|y|
				(
					y.cmd.as_str(),
					(
						y.name.as_deref().unwrap_or(&y.cmd),
						y.description.as_str(),
						y.cmd.as_str(),
						Vec::new(),
					)
				)
			)
			.collect();

		let mut out_args: Vec<DataKind<'_>> = Vec::new();

		src.package.metadata.switches.iter()
			.map(|y| (DataKind::try_from(y), y.subcommands.as_slice()))
			.chain(
				src.package.metadata.options.iter()
					.map(|y| (DataKind::try_from(y), y.subcommands.as_slice()))
			)
			.chain(
				src.package.metadata.arguments.iter()
					.map(|y| (Ok(DataKind::from(y)), y.subcommands.as_slice()))
			)
			.try_for_each(|(arg, subs)| {
				let arg = arg?;
				if subs.is_empty() { out_args.push(arg); }
				else {
					subs.iter().try_for_each(|sub| {
						if sub.is_empty() { out_args.push(arg.clone()); }
						else {
							subcmds
								.get_mut(sub.as_str())
								.ok_or_else(|| BashManError::InvalidSubCommand(Box::from(sub.as_str())))?
								.3
								.push(arg.clone());
						}
						Ok(())
					})?;
				}

				Ok(())
			})?;

		// Drain the subcommands into args.
		out_args.extend(
			subcmds.into_values().map(|v| {
				DataKind::SubCommand(Command::new(
					v.0,
					Some(&src.package.name),
					v.2,
					&src.package.version,
					v.1,
					v.3,
					Vec::new(),
				))
			})
		);

		// Finally return the whole thing!
		Ok(Command::new(
			src.name(),
			None,
			&src.package.name,
			&src.package.version,
			&src.package.description,
			out_args,
			src.sections()?,
		))
	}
}

impl<'a> TryFrom<&'a RawSwitch> for DataKind<'a> {
	type Error = BashManError;

	fn try_from(src: &'a RawSwitch) -> Result<Self, Self::Error> {
		if src.short.is_some() || src.long.is_some() {
			Ok(DataKind::Switch(
				DataFlag {
					short: src.short.as_deref(),
					long: src.long.as_deref(),
					description: &src.description,
					duplicate: src.duplicate,
				}
			))
		}
		else {
			Err(BashManError::InvalidFlag)
		}
	}
}

impl<'a> TryFrom<&'a RawOption> for DataKind<'a> {
	type Error = BashManError;

	fn try_from(src: &'a RawOption) -> Result<Self, Self::Error> {
		if src.short.is_some() || src.long.is_some() {
			Ok(DataKind::Option(
				DataOption {
					flag: DataFlag {
						short: src.short.as_deref(),
						long: src.long.as_deref(),
						description: &src.description,
						duplicate: src.duplicate,
					},
					label: src.label.as_deref().unwrap_or("<VAL>"),
					path: src.path,
				}
			))
		}
		else {
			Err(BashManError::InvalidFlag)
		}
	}
}

impl<'a> From<&'a RawArg> for DataKind<'a> {
	fn from(src: &'a RawArg) -> Self {
		DataKind::Arg(DataItem {
			label: src.label.as_deref().unwrap_or("<VALUES>"),
			description: &src.description
		})
	}
}

impl<'a> TryFrom<[&'a str; 2]> for DataKind<'a> {
	type Error = BashManError;

	fn try_from(src: [&'a str; 2]) -> Result<Self, Self::Error> {
		if src[0].is_empty() || src[1].is_empty() {
			Err(BashManError::InvalidItem)
		}
		else {
			Ok(DataKind::Item(DataItem { label: src[0], description: src[1] }))
		}
	}
}

impl<'a> TryFrom<&'a RawSection> for More<'a> {
	type Error = BashManError;

	fn try_from(src: &'a RawSection) -> Result<Self, Self::Error> {
		Ok(More {
			label: &src.name,
			indent: src.inside,
			data: match (src.lines.is_empty(), src.items.len()) {
				// Neither.
				(true, 0) => return Err(BashManError::InvalidSection),
				// Just lines.
				(false, 0) => vec![DataKind::Paragraph(
					src.lines.iter()
						.map(String::as_str)
						.collect()
				)],
				// Just items.
				(true, _) => src.items.iter().map(|[a, b]|
					DataKind::try_from([a.as_str(), b.as_str()])
				)
					.collect::<Result<_, _>>()?,
				// Both.
				(false, _) => std::iter::once(Ok(DataKind::Paragraph(
						src.lines.iter()
						.map(String::as_str)
						.collect()
					)))
					.chain(src.items.iter().map(|[a, b]| DataKind::try_from([a.as_str(), b.as_str()])))
					.collect::<Result<_, _>>()?,
			}
		})
	}
}
