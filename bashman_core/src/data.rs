/*!
# `Cargo BashMan` — Parsed Data

This is the cleaned-up version of the TOML data.
*/

use crate::BashManError;
use libdeflater::{
	CompressionLvl,
	Compressor,
};
use smartstring::{
	LazyCompact,
	SmartString,
};
use std::{
	ffi::OsStr,
	io::Write,
	os::unix::ffi::OsStrExt,
	path::PathBuf,
};



/// Create a `SmartString` through string formatting.
macro_rules! format_smartstring {
    ($($arg:tt)*) => (format_ss(format_args!($($arg)*)))
}

/// Helper for [`format_smartstring!`].
#[must_use]
#[inline]
fn format_ss(args: std::fmt::Arguments) -> SmartString<LazyCompact> {
	use std::fmt::Write;
	let mut output = SmartString::<LazyCompact>::new();
	let _ = write!(&mut output, "{}", args);
	output
}



#[derive(Debug, Clone)]
/// # Command Metadata.
pub struct Command<'a> {
	pub(crate) name: &'a str,
	pub(crate) parent: Option<&'a str>,
	pub(crate) bin: &'a str,
	pub(crate) version: &'a str,
	pub(crate) description: &'a str,
	pub(crate) data: Vec<DataKind<'a>>,
	pub(crate) more: Vec<More<'a>>,
}

/// # Getters.
impl<'a> Command<'a> {
	#[must_use]
	/// # Bin (cmd).
	pub const fn bin(&'a self) -> &'a str { self.bin }

	#[must_use]
	/// # Description.
	pub const fn description(&'a self) -> &'a str { self.description }

	#[must_use]
	/// # Name.
	pub const fn name(&'a self) -> &'a str { self.name }

	#[must_use]
	/// # Version.
	pub const fn version(&'a self) -> &'a str { self.version }

	#[must_use]
	/// # Has Subcommands?
	fn has_subcommands(&self) -> bool {
		self.parent.is_none() &&
		self.data.iter().any(|o| matches!(o, DataKind::SubCommand(_)))
	}
}

/// # Bash.
impl<'a> Command<'a> {
	/// # Write Bash.
	pub fn write_bash(&self, path: &PathBuf) -> Result<(), BashManError> {
		if ! path.is_dir() {
			return Err(BashManError::WriteBash);
		}

		let mut out: Vec<u8> = Vec::new();

		// Has subcommands.
		if self.has_subcommands() {
			self.data.iter()
				.try_for_each(|x| {
					if let DataKind::SubCommand(x) = x {
						x.bash_completions(&mut out)?;
					}

					Ok(())
				})?;

			self.bash_completions(&mut out)?;
			self.bash_subcommands(&mut out)?;
		}
		// It is cleaner if it is singular.
		else {
			self.bash_completions(&mut out)?;
			writeln!(
				&mut out,
				"complete -F {} -o bashdefault -o default {}",
				self.bash_fname(),
				self.bin
			)
				.map_err(|_| BashManError::WriteBash)?;
		}

		// Write it to a file!
		let mut out_file = path.clone();
		out_file.push(self.bin.to_string() + ".bash");
		std::fs::File::create(&out_file)
			.and_then(|mut f| f.write_all(&out).and_then(|_| f.flush()))
			.map_err(|_| BashManError::WriteBash)?;

		fyi_msg::success!(format_smartstring!(
			"BASH completions written to: {:?}", path
		));

		Ok(())
	}

	/// # BASH Helper (Completions).
	///
	/// This generates the completions for a given app or subcommand. The
	/// output is combined with other code to produce the final script returned
	/// by the main [`Agree::bash`] method.
	fn bash_completions(&self, buf: &mut Vec<u8>) -> Result<(), BashManError> {
		write!(
			buf,
			r#"{}() {{
	local cur prev opts
	COMPREPLY=()
	cur="${{COMP_WORDS[COMP_CWORD]}}"
	prev="${{COMP_WORDS[COMP_CWORD-1]}}"
	opts=()

"#,
			self.bash_fname()
		)
		.map_err(|_| BashManError::WriteBash)?;

		self.data.iter()
			.try_for_each(|x| {
				x.write_bash(buf)
			})?;

		write!(
			buf,
			r#"
	opts=" ${{opts[@]}} "
	if [[ ${{cur}} == -* || ${{COMP_CWORD}} -eq 1 ]] ; then
		COMPREPLY=( $(compgen -W "${{opts}}" -- "${{cur}}") )
		return 0
	fi

{}
	COMPREPLY=( $(compgen -W "${{opts}}" -- "${{cur}}") )
	return 0
}}
"#,
			self.bash_paths()
		)
		.map_err(|_| BashManError::WriteBash)
	}

	/// # BASH Helper (Function Name).
	///
	/// This generates a unique-ish function name for use in the BASH
	/// completion script.
	fn bash_fname(&self) -> SmartString<LazyCompact> {
		format_smartstring!(
			"_basher__{}_{}",
			self.parent.unwrap_or_default(),
			self.bin
		)
			.chars()
			.filter_map(|x| match x {
				'a'..='z' | '0'..='9' => Some(x),
				'A'..='Z' => Some(x.to_ascii_lowercase()),
				'-' | '_' | ' ' => Some('_'),
				_ => None,
			})
			.collect()
	}

	/// # BASH Helper (Path Options).
	///
	/// This produces the file/directory-listing portion of the BASH completion
	/// script for cases where the last option entered expects a path. It is
	/// integrated into the main [`Agree::bash`] output.
	fn bash_paths(&self) -> SmartString<LazyCompact> {
		let keys: Vec<&str> = self.data.iter()
			.filter_map(|o| o.and_path_option().and_then(|o| o.flag.short))
			.chain(
				self.data.iter()
					.filter_map(|o| o.and_path_option().and_then(|o| o.flag.short))
			)
			.collect();

		if keys.is_empty() { SmartString::<LazyCompact>::new() }
		else {
			format_smartstring!(
				r#"	case "${{prev}}" in
		{})
			COMPREPLY=( $( compgen -f "${{cur}}" ) )
			return 0
			;;
		*)
			COMPREPLY=()
			;;
	esac
"#,
				&keys.join("|")
			)
		}
	}

	/// # BASH Helper (Subcommand Chooser).
	///
	/// This generates an additional method for applications with subcommands
	/// to allow per-command suggestions. The output is incorporated into the
	/// value returned by [`Agree::bash`].
	fn bash_subcommands(&self, buf: &mut Vec<u8>) -> Result<(), BashManError> {
		let (cmd, chooser): (SmartString<LazyCompact>, SmartString<LazyCompact>) = std::iter::once((self.bin, self.bash_fname()))
			.chain(
				self.data.iter()
					.filter_map(|x|
						if let DataKind::SubCommand(c) = x {
							Some((c.bin, c.bash_fname()))
						}
						else { None }
					)
			)
			.fold(
				(SmartString::<LazyCompact>::new(), SmartString::<LazyCompact>::new()),
				|(mut a, mut b), (c, d)| {
					a.push_str(&format_smartstring!("\
						\t\t\t{})\n\
						\t\t\t\tcmd=\"{}\"\n\
						\t\t\t\t;;\n",
						&c, &c
					));
					b.push_str(&format_smartstring!("\
						\t\t{})\n\
						\t\t\t{}\n\
						\t\t\t;;\n",
						&c,
						&d
					));

					(a, b)
				}
			);

		write!(
			buf,
			r#"subcmd_{fname}() {{
	local i cmd
	COMPREPLY=()
	cmd=""

	for i in ${{COMP_WORDS[@]}}; do
		case "${{i}}" in
{sub1}
			*)
				;;
		esac
	done

	echo "$cmd"
}}

chooser_{fname}() {{
	local i cmd
	COMPREPLY=()
	cmd="$( subcmd_{fname} )"

	case "${{cmd}}" in
{sub2}
		*)
			;;
	esac
}}

complete -F chooser_{fname} -o bashdefault -o default {bname}
"#,
			fname=self.bash_fname(),
			bname=self.bin,
			sub1=cmd,
			sub2=chooser
		)
			.map_err(|_| BashManError::WriteBash)
	}
}

/// # Manuals.
impl<'a> Command<'a> {
	/// # Write Manuals.
	pub fn write_man(&self, path: &PathBuf) -> Result<(), BashManError> {
		if ! path.is_dir() {
			return Err(BashManError::WriteSubMan(self.bin.to_string()));
		}

		// Main manual first.
		let mut out: Vec<u8> = Vec::new();
		self.man(&mut out)?;
		man_escape(&mut out);

		let mut out_file = path.clone();
		out_file.push(self.bin.to_string() + ".1");
		self._write_man(&out_file, &out)?;

		// All the subcommands.
		self.data.iter().try_for_each(|o| {
			if let DataKind::SubCommand(o) = o {
				out.truncate(0);
				o.man(&mut out)?;
				man_escape(&mut out);

				out_file.pop();
				out_file.push(format!(
					"{}-{}.1",
					self.bin,
					o.bin
				));

				o._write_man(&out_file, &out)?;
			}

			Ok(())
		})?;

		fyi_msg::success!(format_smartstring!(
			"MAN page(s) written to: {:?}", path
		));

		Ok(())
	}

	#[allow(trivial_casts)]
	/// # Write For Real.
	fn _write_man(&self, path: &PathBuf, data: &[u8]) -> Result<(), BashManError> {
		// Write plain.
		std::fs::File::create(&path)
			.and_then(|mut f| f.write_all(data).and_then(|_| f.flush()))
			.map_err(|_| BashManError::WriteSubMan(self.bin.to_string()))?;

		// Write compressed.
		let mut writer = Compressor::new(CompressionLvl::best());
		let mut buf: Vec<u8> = Vec::with_capacity(data.len());
		buf.resize(writer.gzip_compress_bound(data.len()), 0);

		// Trim any excess now that we know the final length.
		let len = writer.gzip_compress(data, &mut buf)
			.map_err(|_| BashManError::WriteSubMan(self.bin.to_string()))?;
		buf.truncate(len);

		// Toss ".gz" onto the original file path and write again!
		std::fs::File::create(OsStr::from_bytes(&[
			unsafe { &*(path.as_os_str() as *const OsStr as *const [u8]) },
			b".gz",
		].concat()))
			.and_then(|mut f| f.write_all(&buf).and_then(|_| f.flush()))
			.map_err(|_| BashManError::WriteSubMan(self.bin.to_string()))
	}

	/// # Manual!
	fn man(&self, buf: &mut Vec<u8>) -> Result<(), BashManError> {
		// Start with the header.
		write!(
			buf,
			r#".TH "{}" "1" "{}" "{} v{}" "User Commands""#,
			match self.parent {
				Some(p) => format!(
					"{} {}",
					p.to_uppercase(),
					self.name().to_uppercase()
				),
				None => self.name().to_uppercase(),
			},
			chrono::Local::now().format("%B %Y"),
			self.name(),
			self.version(),
		)
			.map_err(|_| BashManError::WriteSubMan(self.bin.to_string()))?;

		// Start with general sections.
		More {
			label: "NAME",
			indent: false,
			data: vec![DataKind::Paragraph(vec![&format!(
				"{} - Manual page for {} v{}.",
				self.name(),
				self.bin,
				self.version()
			)])],
		}.man(buf)?;

		More {
			label: "DESCRIPTION",
			indent: false,
			data: vec![DataKind::Paragraph(vec![self.description()])],
		}.man(buf)?;

		More {
			label: "USAGE:",
			indent: true,
			data: vec![DataKind::Paragraph(vec![&self.man_usage()])],
		}.man(buf)?;

		// Handle the generated sections.
		let mut flags: Vec<DataKind> = Vec::new();
		let mut opts: Vec<DataKind> = Vec::new();
		let mut args: Vec<(String, DataKind)> = Vec::new();
		let mut subs: Vec<DataKind> = Vec::new();

		// First pass: collect.
		self.data.iter().for_each(|o| match o {
			DataKind::Switch(_) => {
				flags.push(o.clone());
			},
			DataKind::Option(_) => {
				opts.push(o.clone());
			},
			DataKind::Arg(a) => {
				args.push((
					a.label.to_uppercase() + ":",
					DataKind::Paragraph(vec![a.description])
				));
			},
			DataKind::SubCommand(s) => {
				subs.push(DataKind::Item(DataItem::new(
					s.bin(),
					s.description(),
				)));
			},
			_ => {},
		});

		// Now print each section.
		if ! flags.is_empty() {
			More {
				label: "FLAGS:",
				indent: true,
				data: flags,
			}.man(buf)?;
		}

		if ! opts.is_empty() {
			More {
				label: "OPTIONS:",
				indent: true,
				data: opts,
			}.man(buf)?;
		}

		args.drain(..).try_for_each(|(label, data)| {
			More {
				label: &label,
				indent: true,
				data: vec![data],
			}.man(buf)
		})?;

		if ! subs.is_empty() {
			More {
				label: "SUBCOMMANDS:",
				indent: true,
				data: subs,
			}.man(buf)?;
		}

		// Random sections.
		self.more.iter().try_for_each(|x| x.man(buf))?;

		Ok(())
	}

	/// # Man usage.
	fn man_usage(&self) -> String {
		let mut out: String =
			match self.parent {
				Some(p) => format!("{} {}", p, self.bin),
				None => self.bin.to_string(),
			};

		if self.data.iter().any(|x| matches!(x, DataKind::SubCommand(_))) {
			out.push_str(" [SUBCOMMAND]");
		}

		if self.data.iter().any(|x| matches!(x, DataKind::Switch(_))) {
			out.push_str(" [FLAGS]");
		}

		if self.data.iter().any(|x| matches!(x, DataKind::Option(_))) {
			out.push_str(" [OPTIONS]");
		}

		if let Some(s) = self.data.iter().find_map(|o| match o {
			DataKind::Arg(s) => Some(s),
			_ => None,
		}) {
			out.push(' ');
			out.push_str(s.label);
		}

		out
	}
}



#[derive(Debug, Clone)]
/// # Misc Metadata Section.
pub struct More<'a> {
	label: &'a str,
	indent: bool,
	data: Vec<DataKind<'a>>,
}

impl<'a> More<'a> {
	#[must_use]
	/// # New.
	pub fn new(
		label: &'a str,
		indent: bool,
		lines: &'a [&'a str],
		items: &'a [[&'a str; 2]],
	) -> Option<Self> {
		let mut data = Vec::new();

		// Handle lines.
		let lines: Vec<&'a str> = lines.iter()
			.filter(|y| ! y.is_empty())
			.cloned()
			.collect();
		if ! lines.is_empty() {
			data.push(DataKind::Paragraph(lines));
		}

		// Handle items.
		items.iter()
			.filter(|[a, b]| ! a.is_empty() && ! b.is_empty())
			.for_each(|[a, b]| {
				data.push(DataKind::Item(DataItem::new(a, b)));
			});

		// Return it!
		if data.is_empty() || label.is_empty() { None }
		else {
			Some(Self { label, indent, data })
		}
	}
}

/// # Manual.
impl<'a> More<'a> {
	/// # Manual.
	fn man(&self, buf: &mut Vec<u8>) -> Result<(), BashManError> {
		// Start with the header.
		write!(buf, "\n{} ", if self.indent { ".SS" } else { ".SH" })
			.map_err(|_| BashManError::WriteMan)?;

		match (self.indent, self.label.ends_with(':')) {
			// Indented sections need a trailing colon.
			(true, false) => {
				write!(buf, "{}:", self.label)
					.map_err(|_| BashManError::WriteMan)?;
			},
			// Unindented sections should not have a trailing colon.
			(false, true) => {
				write!(buf, "{}", &self.label[..self.label.len() - 1])
					.map_err(|_| BashManError::WriteMan)?;
			},
			// The label is fine as is.
			_ => {
				write!(buf, "{}", self.label)
					.map_err(|_| BashManError::WriteMan)?;
			}
		}

		// Write each item.
		self.data.iter()
			.try_for_each(|x| {
				x.man(buf, self.indent)
			})?;

		Ok(())
	}
}



#[derive(Debug, Clone)]
/// # Data Kind.
pub enum DataKind<'a> {
	/// # Trailing argument.
	Arg(DataItem<'a>),
	/// # Misc Item.
	Item(DataItem<'a>),
	/// # Option.
	Option(DataOption<'a>),
	/// # Paragraph.
	Paragraph(Vec<&'a str>),
	/// # Subcommand.
	SubCommand(Command<'a>),
	/// # Switch.
	Switch(DataFlag<'a>),
}

/// # Bash.
impl<'a> DataKind<'a> {
	fn write_bash(&self, buf: &mut Vec<u8>) -> Result<(), BashManError> {
		match self {
			Self::Switch(s) => bash_long_short_conds(
				buf,
				s.short,
				s.long,
			),
			Self::Option(s) => bash_long_short_conds(
				buf,
				s.flag.short,
				s.flag.long,
			),
			Self::SubCommand(s) => writeln!(buf, "\topts+=(\"{}\")", s.bin)
				.map_err(|_| BashManError::WriteBash),
			_ => Ok(()),
		}
	}
}

/// # Manual.
impl<'a> DataKind<'a> {
	/// # Manual.
	fn man(&self, buf: &mut Vec<u8>, indent: bool) -> Result<(), BashManError> {
		match self {
			Self::Switch(i) => {
				let res = self.man_tagline(buf)?;
				if res {
					write!(buf, "\n{}", i.description)
						.map_err(|_| BashManError::WriteMan)
				}
				else {
					write!(buf, "\n.TP\n{}", i.description)
						.map_err(|_| BashManError::WriteMan)
				}
			},
			Self::Option(i) => {
				let res = self.man_tagline(buf)?;
				if res {
					write!(buf, "\n{}", i.flag.description)
						.map_err(|_| BashManError::WriteMan)
				}
				else {
					write!(buf, "\n.TP\n{}", i.flag.description)
						.map_err(|_| BashManError::WriteMan)
				}
			},
			Self::Arg(i) | Self::Item(i) => {
				let res = self.man_tagline(buf)?;
				if res {
					write!(buf, "\n{}", i.description)
						.map_err(|_| BashManError::WriteMan)
				}
				else {
					write!(buf, "\n.TP\n{}", i.description)
						.map_err(|_| BashManError::WriteMan)
				}
			},
			Self::Paragraph(i) => {
				if indent {
					write!(buf, "\n.TP\n{}", i.join("\n.RE\n"))
						.map_err(|_| BashManError::WriteMan)
				}
				else {
					write!(buf, "\n{}", i.join("\n.RE\n"))
						.map_err(|_| BashManError::WriteMan)
				}
			},
			_ => Ok(()),
		}
	}

	/// # Manual Tagline.
	fn man_tagline(&self, buf: &mut Vec<u8>) -> Result<bool, BashManError> {
		match self {
			Self::Switch(s) => man_tagline(buf, s.short, s.long, None),
			Self::Option(o) => man_tagline(buf, o.flag.short, o.flag.long, Some(o.label)),
			Self::Arg(k) | Self::Item(k) => man_tagline(buf, None, None, Some(k.label)),
			_ => Ok(false),
		}
	}
}

/// # Misc.
impl<'a> DataKind<'a> {
	/// # And Path Option.
	const fn and_path_option(&'a self) -> Option<&'a DataOption<'a>> {
		if let Self::Option(s) = self {
			if s.path { return Some(s); }
		}

		None
	}
}



#[derive(Debug, Copy, Clone)]
/// # Flag.
pub struct DataFlag<'a> {
	short: Option<&'a str>,
	long: Option<&'a str>,
	description: &'a str,
}

impl<'a> DataFlag<'a> {
	#[must_use]
	/// # New.
	pub fn new(
		long: Option<&'a str>,
		short: Option<&'a str>,
		description: &'a str
	) -> Option<Self> {
		let out = Self {
			long: long.filter(|x| ! x.is_empty()),
			short: short.filter(|x| ! x.is_empty()),
			description
		};

		if out.long.is_some() || out.short.is_some() { Some(out) }
		else { None }
	}
}



#[derive(Debug, Copy, Clone)]
/// # Misc Item.
pub struct DataItem<'a> {
	label: &'a str,
	description: &'a str,
}

impl<'a> DataItem<'a> {
	#[must_use]
	/// # New.
	pub const fn new(label: &'a str, description: &'a str) -> Self {
		Self { label, description }
	}
}



#[derive(Debug, Copy, Clone)]
/// # Option.
pub struct DataOption<'a> {
	flag: DataFlag<'a>,
	label: &'a str,
	path: bool,
}

impl<'a> DataOption<'a> {
	#[must_use]
	/// # New.
	pub const fn new(flag: DataFlag<'a>, label: &'a str, path: bool) -> Self {
		Self {
			flag,
			label,
			path,
		}
	}
}



/// # Bash Helper (Long/Short Conds)
fn bash_long_short_conds(
	buf: &mut Vec<u8>,
	short: Option<&str>,
	long: Option<&str>
) -> Result<(), BashManError> {
	match (short, long) {
		(Some(s), Some(l)) => write!(
			buf,
			r#"	if [[ ! " ${{COMP_LINE}} " =~ " {short} " ]] && [[ ! " ${{COMP_LINE}} " =~ " {long} " ]]; then
		opts+=("{short}")
		opts+=("{long}")
	fi
"#,
			short=s,
			long=l
		)
			.map_err(|_| BashManError::WriteBash),
		(None, Some(k)) | (Some(k), None) => writeln!(
			buf,
			"\t[[ \" ${{COMP_LINE}} \" =~ \" {key} \" ]] || opts+=(\"{key}\")",
			key=k
		)
			.map_err(|_| BashManError::WriteBash),
		(None, None) => Ok(()),
	}
}

/// # Escape Manual.
fn man_escape(src: &mut Vec<u8>) {
	let mut idx: usize = 0;
	let mut len: usize = src.len();

	while idx < len {
		if src[idx] == b'-' {
			src.insert(idx, b'\\');
			idx += 2;
			len += 1;
		}
		else { idx += 1; }
	}

	src.push(b'\n');
}

/// # Man Tagline.
///
/// This helper method generates an appropriate key/value line given what sorts
/// of keys and values exist for the given [`AgreeKind`] type.
fn man_tagline(
	buf: &mut Vec<u8>,
	short: Option<&str>,
	long: Option<&str>,
	value: Option<&str>
) -> Result<bool, BashManError> {
	match (short, long, value) {
		// Option: long and short.
		(Some(s), Some(l), Some(v)) => {
			write!(buf, "\n.TP\n\\fB{}\\fR, \\fB{}\\fR {}", s, l, v)
				.map_err(|_| BashManError::WriteMan)?;
			Ok(true)
		},
		// Option: long or short.
		(Some(k), None, Some(v)) | (None, Some(k), Some(v)) => {
			write!(buf, "\n.TP\n\\fB{}\\fR {}", k, v)
				.map_err(|_| BashManError::WriteMan)?;
			Ok(true)
		},
		// Switch: long and short.
		(Some(s), Some(l), None) => {
			write!(buf, "\n.TP\n\\fB{}\\fR, \\fB{}\\fR", s, l)
				.map_err(|_| BashManError::WriteMan)?;
			Ok(true)
		},
		// Switch: long or short.
		// Key/Value.
		(Some(k), None, None) | (None, Some(k), None) | (None, None, Some(k)) => {
			write!(buf, "\n.TP\n\\fB{}\\fR", k)
				.map_err(|_| BashManError::WriteMan)?;
			Ok(true)
		},
		_ => Ok(false),
	}
}
