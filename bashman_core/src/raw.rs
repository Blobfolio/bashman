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



/// # Helper: Clone Args.
macro_rules! clone_args {
	($set:expr, $arg:ident, $cmd:ident, $subcmd:ident) => {
		if $set.is_empty() {
			$cmd.push($arg);
		}
		else {
			for z in &$set {
				if z.is_empty() { $cmd.push($arg.clone()); }
				else {
					$subcmd
						.get_mut(z)
						.ok_or_else(|| BashManError::InvalidSubCommand((*z).to_string()))?
						.3
						.push($arg.clone());
				}
			}
		}
	};
}



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
		toml::from_str(src)
			.map_err(|_| BashManError::ParseManifest)
	}
}

/// # Conversion.
///
/// Working around the static lifetimes is terrible, but this eventually gets
/// there!
impl<'a> Raw<'a> {
	/// # Parse.
	pub(super) fn parse(&'a self) -> Result<Command<'a>, BashManError> {
		// We can do this a light lighter without worrying about subcommands.
		if self.package.metadata.subcommands.is_empty() {
			return Ok(self.parse_single());
		}

		let mut subcmds: IndexMap<&str, (&str, &str, &str, Vec::<DataKind<'a>>)> = IndexMap::new();

		for y in &self.package.metadata.subcommands {
			// Command is required.
			if y.cmd.is_empty() {
				return Err(BashManError::MissingSubCommand);
			}

			subcmds.insert(
				y.cmd,
				(
					y.name.unwrap_or(y.cmd),
					y.description,
					y.cmd,
					Vec::new(),
				)
			);
		}

		let mut out_args: Vec<DataKind<'_>> = Vec::new();

		// Switches.
		for y in &self.package.metadata.switches {
			if let Some(flag) = DataFlag::new(y.long, y.short, y.description) {
				let arg = DataKind::Switch(flag);

				clone_args!(y.subcommands, arg, out_args, subcmds);
			}
		}

		// Options.
		for y in &self.package.metadata.options {
			if let Some(flag) = DataFlag::new(y.long, y.short, y.description) {
				let arg = DataKind::Option(DataOption::new(
					flag,
					y.label.unwrap_or("<VAL>"),
					y.path,
				));

				clone_args!(y.subcommands, arg, out_args, subcmds);
			}
		}

		// Arguments.
		for y in &self.package.metadata.arguments {
			if ! y.description.is_empty() {
				let arg = DataKind::Arg(DataItem::new(
					y.label.unwrap_or("<VALUES>"),
					y.description
				));

				clone_args!(y.subcommands, arg, out_args, subcmds);
			}
		}

		// Drain the subcommands into args.
		out_args.extend(
			subcmds.drain(..).map(|(_, v)| {
				DataKind::SubCommand(Command {
					name: v.0,
					description: v.1,
					parent: Some(self.command()),
					bin: v.2,
					version: self.version(),
					data: v.3,
					more: Vec::new(),
				})
			})
		);

		// Finally return the whole thing!
		Ok(Command {
			name: self.name(),
			description: self.description(),
			parent: None,
			bin: self.command(),
			version: self.version(),
			data: out_args,
			more: self.sections().unwrap_or_default(),
		})
	}

	/// # Parse Single.
	///
	/// This parses without subcommand support.
	fn parse_single(&'a self) -> Command<'a> {
		let mut out_args: Vec<DataKind<'_>> = Vec::new();

		// Switches.
		for y in &self.package.metadata.switches {
			if let Some(flag) = DataFlag::new(y.long, y.short, y.description) {
				out_args.push(DataKind::Switch(flag));
			}
		}

		// Options.
		for y in &self.package.metadata.options {
			if let Some(flag) = DataFlag::new(y.long, y.short, y.description) {
				out_args.push(
					DataKind::Option(DataOption::new(
						flag,
						y.label.unwrap_or("<VAL>"),
						y.path,
					))
				);
			}
		}

		// Arguments.
		for y in &self.package.metadata.arguments {
			if ! y.description.is_empty() {
				out_args.push(
					DataKind::Arg(DataItem::new(
						y.label.unwrap_or("<VALUES>"),
						y.description
					))
				);
			}
		}

		// Finally return the whole thing!
		Command {
			name: self.name(),
			description: self.description(),
			parent: None,
			bin: self.command(),
			version: self.version(),
			data: out_args,
			more: Vec::new(),
		}
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
			std::fs::canonicalize(path)
				.map_err(|_| BashManError::InvalidBashDir)
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
			std::fs::canonicalize(path)
				.map_err(|_| BashManError::InvalidManDir)
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
		let mut out = Vec::new();

		for y in &self.package.metadata.sections {
			if let Some(section) = More::new(y.name, y.inside, &y.lines, &y.items) {
				out.push(section);
			}
		}

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
	name: &'a str,
	version: &'a str,
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
	name: Option<&'a str>,

	#[serde(rename = "bash-dir")]
	bash_dir: Option<&'a str>,

	#[serde(rename = "man-dir")]
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



#[derive(Debug, Clone, Deserialize)]
/// # Raw Subcommand.
///
/// This is what is found under "package.metadata.bashman.subcommands".
struct RawSubCmd<'a> {
	name: Option<&'a str>,
	cmd: &'a str,
	description: &'a str,
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Switch.
///
/// This is what is found under "package.metadata.bashman.switches".
struct RawSwitch<'a> {
	short: Option<&'a str>,
	long: Option<&'a str>,
	description: &'a str,

	#[serde(default)]
	subcommands: Vec<&'a str>,
}



#[derive(Debug, Clone, Deserialize)]
/// Raw Option.
///
/// This is what is found under "package.metadata.bashman.options".
struct RawOption<'a> {
	short: Option<&'a str>,
	long: Option<&'a str>,
	description: &'a str,
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
	label: Option<&'a str>,
	description: &'a str,

	#[serde(default)]
	subcommands: Vec<&'a str>,
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Section.
///
/// This is what is found under "package.metadata.bashman.subcommands".
struct RawSection<'a> {
	name: &'a str,

	#[serde(default)]
	inside: bool,

	#[serde(default)]
	lines: Vec<&'a str>,

	#[serde(default)]
	items: Vec<[&'a str; 2]>
}
