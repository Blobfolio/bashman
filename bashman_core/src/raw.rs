/*!
# `Cargo BashMan` — Raw Data

This helps parse the raw TOML structure into the data we care about. From here,
it can be converted into a more agreeable (and validated) structure.
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
use std::{
	convert::TryFrom,
	path::PathBuf,
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
		toml::from_str(src).map_err(|e| BashManError::ParseManifest(e.to_string()))
	}
}

/// # Conversion.
///
/// Working around the static lifetimes is terrible, but this eventually gets
/// there!
impl<'a> Raw<'a> {
	/// # Parse.
	pub(super) fn parse(&'a self) -> Result<Command<'a>, BashManError> {
		// We can process data more directly if there are no subcommands to
		// worry about.
		if self.package.metadata.subcommands.is_empty() {
			return Ok(self.parse_single());
		}

		let mut subcmds: IndexMap<&'_ str, (&'_ str, &'_ str, &'_ str, Vec::<DataKind<'_>>)> = self.package.metadata.subcommands.iter()
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

		self.package.metadata.switches.iter()
			.filter_map(|y|
				DataFlag::new(y.long, y.short, y.description)
					.map(|f| (DataKind::Switch(f), y.subcommands.as_slice()))
			)
			.chain(
				self.package.metadata.options.iter()
					.filter_map(|y|
						DataFlag::new(y.long, y.short, y.description)
							.map(|f|
								(
									DataKind::Option(DataOption::new(
										f,
										y.label.unwrap_or("<VAL>"),
										y.path,
									)),
									y.subcommands.as_slice()
								)
							)
					)
			)
			.chain(
				self.package.metadata.arguments.iter()
					.map(|y|
						(
							DataKind::Arg(DataItem::new(
								y.label.unwrap_or("<VALUES>"),
								y.description
							)),
							y.subcommands.as_slice()
						)
					)
			)
			.try_for_each(|(arg, subs)| {
				if subs.is_empty() { out_args.push(arg); }
				else {
					subs.iter().try_for_each(|sub| {
						if sub.is_empty() { out_args.push(arg.clone()) }
						else {
							subcmds
								.get_mut(sub)
								.ok_or_else(|| BashManError::InvalidSubCommand((*sub).to_string()))?
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
					Some(self.command()),
					v.2,
					self.version(),
					v.1,
					v.3,
					Vec::new(),
				))
			})
		);

		// Finally return the whole thing!
		Ok(Command::new(
			self.name(),
			None,
			self.command(),
			self.version(),
			self.description(),
			out_args,
			self.sections().unwrap_or_default(),
		))
	}

	/// # Parse Single.
	///
	/// This parses without subcommand support.
	fn parse_single(&'a self) -> Command<'a> {
		// Switches.
		let out_args: Vec<DataKind<'_>> = self.package.metadata.switches.iter()
			.filter_map(|y|
				DataFlag::new(y.long, y.short, y.description)
					.map(DataKind::Switch)
			)
			.chain(
				self.package.metadata.options.iter()
					.filter_map(|y|
						DataFlag::new(y.long, y.short, y.description)
							.map(|f| DataKind::Option(DataOption::new(
								f,
								y.label.unwrap_or("<VAL>"),
								y.path,
							)))
					)
			)
			.chain(
				self.package.metadata.arguments.iter()
					.map(|y| DataKind::Arg(DataItem::new(
						y.label.unwrap_or("<VALUES>"),
						y.description
					)))
			)
			.collect();

		// Finally return the whole thing!
		Command::new(
			self.name(),
			None,
			self.command(),
			self.version(),
			self.description(),
			out_args,
			Vec::new(),
		)
	}
}

/// # Getters.
impl<'a> Raw<'a> {
	/// # Bash Directory.
	pub(super) fn bash_dir(&self, dir: &PathBuf) -> Result<PathBuf, BashManError> {
		let path: PathBuf = self.package.metadata.bash_dir
			.map_or_else(|| dir.clone(), |path|
				if path.starts_with('/') { PathBuf::from(path) }
				else {
					let mut tmp = dir.clone();
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

	#[must_use]
	/// # Main Command.
	const fn command(&'a self) -> &'a str {
		self.package.name
	}

	#[must_use]
	/// # Main Description.
	const fn description(&'a self) -> &'a str {
		self.package.description
	}

	/// # Man Directory.
	pub(super) fn man_dir(&self, dir: &PathBuf) -> Result<PathBuf, BashManError> {
		let path: PathBuf = self.package.metadata.man_dir
			.map_or_else(|| dir.clone(), |path|
				if path.starts_with('/') { PathBuf::from(path) }
				else {
					let mut tmp = dir.clone();
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
	fn name(&'a self) -> &'a str {
		self.package.metadata.name.unwrap_or(self.package.name)
	}

	#[must_use]
	/// # Sections.
	fn sections(&'a self) -> Option<Vec<More<'a>>> {
		let out: Vec<More<'a>> = self.package.metadata.sections.iter()
			.filter_map(|y| More::new(y.name, y.inside, &y.lines, &y.items))
			.collect();

		if out.is_empty() { None }
		else { Some(out) }
	}

	#[must_use]
	/// # Main Version.
	const fn version(&'a self) -> &'a str {
		self.package.version
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
