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



#[derive(Debug, Clone, Deserialize)]
/// # Raw Data.
///
/// The outermost struct.
pub(super) struct Raw<'a> {
	#[serde(borrow)]
	package: RawPackage<'a>
}

impl<'a> TryFrom<&'a str> for Raw<'a> {
	type Error = BashManError;

	fn try_from(src: &'a str) -> Result<Self, Self::Error> {
		toml::from_str(src).map_err(|e| BashManError::ParseManifest(Box::from(e.to_string())))
	}
}

/// # Conversion.
///
/// Working around the static lifetimes is terrible, but this eventually gets
/// there!
impl<'a> Raw<'a> {
	/// # Parse Single.
	///
	/// This parses without subcommand support.
	fn parse_single(&'a self) -> Result<Command<'a>, BashManError> {
		// Switches.
		let out_args: Vec<DataKind<'_>> = self.package.metadata.switches.iter()
			.map(DataKind::try_from)
			.chain(
				self.package.metadata.options.iter().map(DataKind::try_from)
			)
			.chain(
				self.package.metadata.arguments.iter().map(|y| Ok(DataKind::from(y)))
			)
			.try_fold(Vec::with_capacity(self.count_args()), |mut v, a| {
				v.push(a?);
				Ok(v)
			})?;

		// Finally return the whole thing!
		Ok(Command::new(
			self.name(),
			None,
			self.package.name,
			self.package.version,
			self.package.description,
			out_args,
			self.sections()?,
		))
	}

	/// # Arg Size Hint.
	fn count_args(&self) -> usize {
		self.package.metadata.switches.len() +
		self.package.metadata.options.len() +
		self.package.metadata.arguments.len()
	}
}

/// # Getters.
impl<'a> Raw<'a> {
	/// # Bash Directory.
	pub(super) fn bash_dir(&self, dir: &Path) -> Result<PathBuf, BashManError> {
		let path: PathBuf = self.package.metadata.bash_dir
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
	fn name(&self) -> &'a str {
		self.package.metadata.name.unwrap_or(self.package.name)
	}

	/// # Sections.
	fn sections(&'a self) -> Result<Vec<More<'a>>, BashManError> {
		self.package.metadata.sections.iter()
			.map(More::try_from)
			.try_fold(Vec::with_capacity(self.package.metadata.sections.len()), |mut v, a| {
				v.push(a?);
				Ok(v)
			})
	}
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Package Data.
///
/// This is what is found under "package".
struct RawPackage<'a> {
	#[serde(deserialize_with = "deserialize_nonempty_str")]
	name: &'a str,

	#[serde(deserialize_with = "deserialize_nonempty_str")]
	version: &'a str,

	#[serde(deserialize_with = "deserialize_nonempty_str")]
	description: &'a str,

	#[serde(with = "RawMeta")]
	metadata: RawBashMan<'a>,
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
struct RawBashMan<'a> {
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	name: Option<&'a str>,

	#[serde(rename = "bash-dir")]
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	bash_dir: Option<&'a str>,

	#[serde(rename = "man-dir")]
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	man_dir: Option<&'a str>,

	#[serde(rename = "credits-dir")]
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	credits_dir: Option<&'a str>,

	#[serde(default)]
	subcommands: Vec<RawSubCmd<'a>>,

	#[serde(default)]
	switches: Vec<RawSwitch<'a>>,

	#[serde(default)]
	options: Vec<RawOption<'a>>,

	#[serde(default)]
	arguments: Vec<RawArg<'a>>,

	#[serde(default)]
	sections: Vec<RawSection<'a>>,
}



#[derive(Debug, Copy, Clone, Deserialize)]
/// # Raw Subcommand.
///
/// This is what is found under "package.metadata.bashman.subcommands".
struct RawSubCmd<'a> {
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	name: Option<&'a str>,

	#[serde(deserialize_with = "deserialize_nonempty_str")]
	cmd: &'a str,

	#[serde(deserialize_with = "deserialize_nonempty_str")]
	description: &'a str,
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Switch.
///
/// This is what is found under "package.metadata.bashman.switches".
struct RawSwitch<'a> {
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	short: Option<&'a str>,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	long: Option<&'a str>,

	#[serde(deserialize_with = "deserialize_nonempty_str")]
	description: &'a str,

	#[serde(default)]
	subcommands: Vec<&'a str>,
}



#[derive(Debug, Clone, Deserialize)]
/// Raw Option.
///
/// This is what is found under "package.metadata.bashman.options".
struct RawOption<'a> {
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	short: Option<&'a str>,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	long: Option<&'a str>,

	#[serde(deserialize_with = "deserialize_nonempty_str")]
	description: &'a str,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	label: Option<&'a str>,

	#[serde(default)]
	path: bool,

	#[serde(default)]
	subcommands: Vec<&'a str>,
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Argument.
///
/// This is what is found under "package.metadata.bashman.arguments".
struct RawArg<'a> {
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nonempty_opt_str")]
	label: Option<&'a str>,

	#[serde(deserialize_with = "deserialize_nonempty_str")]
	description: &'a str,

	#[serde(default)]
	subcommands: Vec<&'a str>,
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Section.
///
/// This is what is found under "package.metadata.bashman.subcommands".
struct RawSection<'a> {
	#[serde(deserialize_with = "deserialize_nonempty_str")]
	name: &'a str,

	#[serde(default)]
	inside: bool,

	#[serde(default)]
	lines: Vec<&'a str>,

	#[serde(default)]
	items: Vec<[&'a str; 2]>
}



/// # Deserialize: Require Non-Empty Str
///
/// This will return an error if a string is present but empty.
fn deserialize_nonempty_str<'de, D>(deserializer: D) -> Result<&'de str, D::Error>
where D: Deserializer<'de> {
	let wrapper = <&'de str as Deserialize>::deserialize(deserializer)?;
	if wrapper.is_empty() { Err(serde::de::Error::custom("Value cannot be empty.")) }
	else { Ok(wrapper) }
}

#[allow(clippy::unnecessary_wraps)] // Not our structure to decide.
/// # Deserialize: Require Non-Empty Str
///
/// This will return `None` if the string is empty.
fn deserialize_nonempty_opt_str<'de, D>(deserializer: D) -> Result<Option<&'de str>, D::Error>
where D: Deserializer<'de> {
	Ok(
		Option::<&'de str>::deserialize(deserializer)
			.ok()
			.flatten()
			.filter(|x| ! x.is_empty())
	)
}



impl<'a> TryFrom<&'a Raw<'a>> for Command<'a> {
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
					y.cmd,
					(
						y.name.unwrap_or(y.cmd),
						y.description,
						y.cmd,
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
								.get_mut(sub)
								.ok_or_else(|| BashManError::InvalidSubCommand(Box::from(*sub)))?
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
			subcmds.drain(..).map(|(_, v)| {
				DataKind::SubCommand(Command::new(
					v.0,
					Some(src.package.name),
					v.2,
					src.package.version,
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
			src.package.name,
			src.package.version,
			src.package.description,
			out_args,
			src.sections()?,
		))
	}
}

impl<'a> TryFrom<&'a RawSwitch<'a>> for DataKind<'a> {
	type Error = BashManError;

	fn try_from(src: &'a RawSwitch<'a>) -> Result<Self, Self::Error> {
		if src.short.is_some() || src.long.is_some() {
			Ok(DataKind::Switch(
				DataFlag {
					short: src.short,
					long: src.long,
					description: src.description,
				}
			))
		}
		else {
			Err(BashManError::InvalidFlag)
		}
	}
}

impl<'a> TryFrom<&'a RawOption<'a>> for DataKind<'a> {
	type Error = BashManError;

	fn try_from(src: &'a RawOption<'a>) -> Result<Self, Self::Error> {
		if src.short.is_some() || src.long.is_some() {
			Ok(DataKind::Option(
				DataOption {
					flag: DataFlag {
						short: src.short,
						long: src.long,
						description: src.description,
					},
					label: src.label.unwrap_or("<VAL>"),
					path: src.path
				}
			))
		}
		else {
			Err(BashManError::InvalidFlag)
		}
	}
}

impl<'a> From<&'a RawArg<'a>> for DataKind<'a> {
	fn from(src: &'a RawArg<'a>) -> Self {
		DataKind::Arg(DataItem {
			label: src.label.unwrap_or("<VALUES>"),
			description: src.description
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

impl<'a> TryFrom<&'a RawSection<'a>> for More<'a> {
	type Error = BashManError;

	fn try_from(src: &'a RawSection<'a>) -> Result<Self, Self::Error> {
		Ok(More {
			label: src.name,
			indent: src.inside,
			data: match (src.lines.is_empty(), src.items.len()) {
				// Neither.
				(true, 0) => return Err(BashManError::InvalidSection),
				// Just lines.
				(false, 0) => vec![DataKind::Paragraph(src.lines.clone())],
				// Just items.
				(true, len) => src.items.iter().try_fold(Vec::with_capacity(len), |mut v, &a| {
					v.push(DataKind::try_from(a)?);
					Ok(v)
				})?,
				// Both.
				(false, len) => std::iter::once(Ok(DataKind::Paragraph(src.lines.clone())))
					.chain(src.items.iter().map(|&a| DataKind::try_from(a)))
					.try_fold(Vec::with_capacity(len + 1), |mut v, a| {
						v.push(a?);
						Ok(v)
					})?,
			}
		})
	}
}
