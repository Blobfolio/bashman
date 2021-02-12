/*!
# `Cargo BashMan`
*/

#![warn(clippy::filetype_is_file)]
#![warn(clippy::integer_division)]
#![warn(clippy::needless_borrow)]
#![warn(clippy::nursery)]
#![warn(clippy::pedantic)]
#![warn(clippy::perf)]
#![warn(clippy::suboptimal_flops)]
#![warn(clippy::unneeded_field_pattern)]
#![warn(macro_use_extern_crate)]
#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(non_ascii_idents)]
#![warn(trivial_casts)]
#![warn(trivial_numeric_casts)]
#![warn(unreachable_pub)]
#![warn(unused_extern_crates)]
#![warn(unused_import_braces)]

#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::map_err_ignore)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]

pub mod agree;
mod error;

use indexmap::IndexMap;
use std::path::{
	Path,
	PathBuf,
};
use toml::Value;

pub use agree::{
	Agree,
	AgreeKind,
	AgreeSwitch,
	AgreeOption,
	AgreeItem,
	AgreeParagraph,
	AgreeSection,
};
pub use error::BashManError;
use smartstring::{
	SmartString,
	LazyCompact,
};



/// Create a `SmartString` through string formatting.
#[macro_export]
macro_rules! format_smartstring {
    ($($arg:tt)*) => ($crate::format_ss(format_args!($($arg)*)))
}

/// Helper for [`format_smartstring!`].
#[must_use]
#[inline]
pub fn format_ss(args: std::fmt::Arguments) -> SmartString<LazyCompact> {
	use std::fmt::Write;
	let mut output = SmartString::<LazyCompact>::new();
	let _ = write!(&mut output, "{}", args);
	output
}



#[derive(Debug, Clone, Eq, Hash, PartialEq)]
/// # Bash Man.
///
/// This is a wrapper around [`Agree`] containing the manifest path and the
/// output directories for BASH and MAN content.
///
/// This exists solely to help out the `cargo-bashman` binary. If for some
/// reason you want to use the library on its own, you should probably just
/// use [`Agree`] directly.
///
/// See the `README` for more information.
pub struct BashMan {
	agree: Agree,
	manifest: PathBuf,
	bash: PathBuf,
	man: PathBuf,
}

impl BashMan {
	#[allow(clippy::similar_names)] // Sorry not sorry.
	/// # New.
	///
	/// Start a new instance given a Cargo manifest path. This will parse the
	/// contents of that file to construct all the arguments, etc.
	///
	/// If it works, an instance is returned, otherwise an error message is
	/// returned as a string.
	pub fn new<P>(manifest: P) -> Result<Self, BashManError>
	where P: AsRef<Path> {
		// Clean up the manifest path.
		let manifest = std::fs::canonicalize(manifest)
			.map_err(|_| BashManError::InvalidManifest)?;

		// Parse the raw TOML.
		let raw = {
			let content = std::fs::read_to_string(&manifest)
				.map_err(|_| BashManError::InvalidManifest)?;

			content.parse::<Value>()
				.map_err(|_| BashManError::ParseManifest)?
		};

		// The main app section.
		let main = raw
			.get("package")
			.ok_or(BashManError::MissingPackage)?;

		// BashMan-specific data.
		let bm = main.get("metadata")
			.and_then(|s| s.get("bashman"))
			.ok_or(BashManError::MissingPackageMeta)?;

		// Extract some basic metadata.
		let cmd: &str = main.get("name")
			.and_then(Value::as_str)
			.unwrap_or_default();

		let dir = manifest.parent().unwrap().to_path_buf();

		let bash: PathBuf = resolve_path(bm.get("bash-dir"), &dir)
			.map_err(|_| BashManError::InvalidBashDir)?;

		let man: PathBuf = resolve_path(bm.get("man-dir"), &dir)
			.map_err(|_| BashManError::InvalidManDir)?;

		// We have enough to start an Agree!
		let mut agree: Agree = Agree::new(
			bm.get("name")
				.and_then(Value::as_str)
				.unwrap_or(cmd),
			main.get("description")
				.and_then(Value::as_str)
				.unwrap_or_default(),
			cmd,
			main.get("version")
				.and_then(Value::as_str)
				.unwrap_or_default(),
		);

		// Check for subcommands.
		let mut subcmd: IndexMap<SmartString<LazyCompact>, Agree> = IndexMap::new();
		resolve_subcommands(bm.get("subcommands"), &mut subcmd, agree.version());

		// Load up flags/options/args.
		resolve_switches(bm.get("switches"), &mut agree, &mut subcmd);
		resolve_options(bm.get("options"), &mut agree, &mut subcmd);
		resolve_args(bm.get("arguments"), &mut agree, &mut subcmd);

		// Add subcommands.
		agree = subcmd.drain(..).map(|(_k, v)| AgreeKind::SubCommand(v)).fold(agree, Agree::with_arg);

		resolve_sections(bm.get("sections"), &mut agree);

		// Pull a few generic things.
		Ok(Self {
			agree,
			manifest,
			bash,
			man,
		})
	}

	/// # Write.
	///
	/// Attempt to write the BASH completions and MAN page(s). If any problems
	/// arise with either, the program will print an error and exit with a
	/// status code of `1`.
	pub fn write(&self) -> Result<(), BashManError> {
		self._write()?;
		fyi_msg::success!(format!("BASH completions written to: {:?}", &self.bash));
		fyi_msg::success!(format!("MAN page(s) written to: {:?}", &self.man));
		Ok(())
	}

	/// # Write.
	///
	/// This does the actual writing, or rather, calls [`Agree::write_bash`]
	/// and [`Agree::write_man`] with the appropriate paths. Errors are bubbled
	/// up as applicable.
	fn _write(&self) -> Result<(), BashManError> {
		self.agree.write_bash(&self.bash)?;
		self.agree.write_man(&self.man)?;
		Ok(())
	}
}



/// # Load From Manifest.
///
/// This is a wrapper around [`BashMan::new`] that tries to interpret the
/// manifest path. If no path is provided, it will check for a `Cargo.toml`
/// file in the current directory.
///
/// If all goes well, the `BashMan` instance is returned, otherwise an error
/// is printed and the program exists with a status code of `1`.
pub fn load<P>(src: Option<P>) -> Result<BashMan, BashManError>
where P: AsRef<Path> {
	let src = src.map(|x| PathBuf::from(x.as_ref()))
		.or_else(|| Some(PathBuf::from("./Cargo.toml")))
		.and_then(|s| std::fs::canonicalize(s).ok())
		.ok_or(BashManError::MissingManifest)?;

	BashMan::new(src)
}

/// # Resolve Path.
///
/// This helper method interprets raw TOML values as paths. If they're not
/// absolute, they are realigned to be relative to the manifest directory.
fn resolve_path(path: Option<&Value>, dir: &PathBuf) -> Result<PathBuf, BashManError> {
	path.and_then(Value::as_str)
		.map_or_else(
			|| Ok(dir.clone()),
			|path|
				if path.starts_with('/') { std::fs::canonicalize(path) }
				else {
					let mut tmp: PathBuf = dir.clone();
					tmp.push(path);
					std::fs::canonicalize(tmp)
				}.map_err(|_| BashManError::InvalidPath(PathBuf::from(path)))
		)
}

/// # Resolve Subcommands.
///
/// This helper method interprets any raw "subcommands" TOML values. On this
/// pass, subcommands are parsed into [`Agree`] structs, but without
/// arguments. Those will reveal themselves later.
fn resolve_subcommands(
	subcommands: Option<&Value>,
	subcmd: &mut IndexMap<SmartString<LazyCompact>, Agree>,
	version: &str,
) {
	if let Some(x) = subcommands.and_then(Value::as_array) {
		x.iter().filter_map(Value::as_table).for_each(|y| {
			let cmd: SmartString<LazyCompact> = y.get("cmd")
				.and_then(Value::as_str)
				.unwrap_or_default()
				.into();

			if ! cmd.is_empty() {
				let agree = Agree::new(
					y.get("name")
						.and_then(Value::as_str)
						.unwrap_or(&cmd),
					y.get("description")
						.and_then(Value::as_str)
						.unwrap_or_default(),
					&cmd,
					version,
				);

				subcmd.insert(cmd, agree);
			}
		});
	}
}

/// # Resolve Switches.
///
/// This helper method interprets any raw "switches" TOML values, pushing them
/// to the main [`Agree`] struct and/or the associated subcommands.
fn resolve_switches(
	switches: Option<&Value>,
	cmd: &mut Agree,
	subcmd: &mut IndexMap<SmartString<LazyCompact>, Agree>
) {
	if let Some(x) = switches.and_then(Value::as_array) {
		x.iter().filter_map(Value::as_table).for_each(|y| {
			let mut switch: AgreeKind = AgreeKind::switch(y.get("description")
				.and_then(Value::as_str)
				.unwrap_or_default()
			);

			switch = resolve_short_long(switch, y);
			clone_args(y.get("subcommands"), switch, cmd, subcmd);
		});
	}
}

/// # Resolve Options.
///
/// This helper method interprets any raw "options" TOML values, pushing them
/// to the main [`Agree`] struct and/or the associated subcommands.
fn resolve_options(
	options: Option<&Value>,
	cmd: &mut Agree,
	subcmd: &mut IndexMap<SmartString<LazyCompact>, Agree>
) {
	if let Some(x) = options.and_then(Value::as_array) {
		x.iter().filter_map(Value::as_table).for_each(|y| {
			let mut option: AgreeKind = AgreeKind::option(
				y.get("label")
					.and_then(Value::as_str)
					.unwrap_or_default(),
				y.get("description")
					.and_then(Value::as_str)
					.unwrap_or_default(),
				y.get("path")
					.and_then(Value::as_bool)
					.unwrap_or_default(),
			);

			option = resolve_short_long(option, y);
			clone_args(y.get("subcommands"), option, cmd, subcmd);
		});
	}
}

/// # Resolve Args.
///
/// This helper method interprets any raw "arguments" TOML values, pushing them
/// to the main [`Agree`] struct and/or the associated subcommands.
fn resolve_args(
	args: Option<&Value>,
	cmd: &mut Agree,
	subcmd: &mut IndexMap<SmartString<LazyCompact>, Agree>
) {
	if let Some(x) = args.and_then(Value::as_array) {
		x.iter().filter_map(Value::as_table).for_each(|y| {
			let arg: AgreeKind = AgreeKind::arg(
				y.get("label")
					.and_then(Value::as_str)
					.unwrap_or_default(),
				y.get("description")
					.and_then(Value::as_str)
					.unwrap_or_default(),
			);

			clone_args(y.get("subcommands"), arg, cmd, subcmd);
		});
	}
}

/// # Resolve Short/Long.
///
/// This helper method interprets any raw "long" or "short" TOML values
/// associated with a switch or option, and appends them as necessary.
fn resolve_short_long(
	mut arg: AgreeKind,
	set: &toml::map::Map<String, Value>
) -> AgreeKind {
	if let Some(z) = set.get("long").and_then(Value::as_str) {
		arg = arg.with_long(z);
	}

	if let Some(z) = set.get("short").and_then(Value::as_str) {
		arg = arg.with_short(z);
	}

	arg
}

/// # Resolve Sections.
///
/// This helper method interprets any raw "sections" TOML values, pushing them
/// to the main [`Agree`] struct.
fn resolve_sections(
	sections: Option<&Value>,
	agree: &mut Agree,
) {
	if let Some(x) = sections.and_then(Value::as_array) {
		x.iter().filter_map(Value::as_table).for_each(|y| {
			let mut section = AgreeSection::new(
				y.get("name")
					.and_then(Value::as_str)
					.unwrap_or_default(),
				y.get("inside")
					.and_then(Value::as_bool)
					.unwrap_or_default(),
			);

			// Do we have lines?
			let lines = y.get("lines")
				.and_then(Value::as_array)
				.unwrap_or(&Vec::new())
				.iter()
				.filter_map(|z| Value::as_str(z))
				.fold(AgreeParagraph::default(), AgreeParagraph::with_line);
			if ! lines.is_empty() {
				section.push_item(AgreeKind::Paragraph(lines));
			}

			// Add any items.
			section = y.get("items")
				.and_then(Value::as_array)
				.unwrap_or(&Vec::new())
				.iter()
				.filter_map(|z|
					Value::as_array(z)
						.filter(|z| z.len() == 2)
						.and_then(|z| Value::as_str(&z[0]).zip(Value::as_str(&z[1])))
						.map(|(k, v)| AgreeKind::item(k, v))
				)
				.fold(section, AgreeSection::with_item);

			// Push the section if it isn't empty.
			if ! section.is_empty() {
				agree.push_section(section);
			}
		});
	}
}

/// # Copy Agree Kinds.
///
/// This helper pushes a given [`AgreeKind`] to one or more [`Agree`] structs.
fn clone_args(
	set: Option<&Value>,
	arg: AgreeKind,
	cmd: &mut Agree,
	subcmd: &mut IndexMap<SmartString<LazyCompact>, Agree>
) {
	if let Some(z) = set.and_then(Value::as_array).filter(|z| ! z.is_empty()) {
		z.iter().filter_map(Value::as_str).for_each(|sub| {
			if sub.is_empty() {
				cmd.push_arg(arg.clone());
			}
			else if subcmd.contains_key(sub) {
				subcmd.get_mut(sub).unwrap().push_arg(arg.clone());
			}
		});
	}
	else {
		cmd.push_arg(arg);
	}
}
