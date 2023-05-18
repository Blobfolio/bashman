/*!
# `Cargo BashMan` — Parsed Data

This module contains the [`Command`] and related data structures produced from
[`Raw`]. See that module for more information on why all the windiness.
*/

use crate::BashManError;
use fyi_msg::Msg;
use libdeflater::{
	CompressionLvl,
	Compressor,
};
use std::{
	fs::File,
	io::Write,
	path::Path,
};
use utc2k::Utc2k;



const FLAG_ARGUMENTS: u8 =    0b0000_0001;
const FLAG_OPTIONS: u8 =      0b0000_0010;
const FLAG_PATH_OPTIONS: u8 = 0b0000_0110;
const FLAG_SUBCOMMANDS: u8 =  0b0000_1000;
const FLAG_SWITCHES: u8 =     0b0001_0000;



#[derive(Debug, Clone)]
/// # Command Metadata.
pub(super) struct Command<'a> {
	pub(crate) name: &'a str,
	pub(crate) parent: Option<&'a str>,
	pub(crate) bin: &'a str,
	pub(crate) version: &'a str,
	pub(crate) description: &'a str,
	pub(crate) data: Vec<DataKind<'a>>,
	pub(crate) more: Vec<More<'a>>,
	flags: u8,
	fname: Box<str>,
}

/// # Instantiation.
impl<'a> Command<'a> {
	/// # New.
	pub(crate) fn new(
		name: &'a str,
		parent: Option<&'a str>,
		bin: &'a str,
		version: &'a str,
		description: &'a str,
		data: Vec<DataKind<'a>>,
		more: Vec<More<'a>>,
	) -> Self {
		// One iter up front to see what kinds of content we have. This will
		// potentially save unnecessary work later on.
		let flags: u8 = data.iter().fold(0, |f, o| {
			match o {
				DataKind::SubCommand(_) => f | FLAG_SUBCOMMANDS,
				DataKind::Switch(_) => f | FLAG_SWITCHES,
				DataKind::Arg(_) => f | FLAG_ARGUMENTS,
				DataKind::Option(o) => {
					if o.path { f | FLAG_PATH_OPTIONS }
					else { f | FLAG_OPTIONS }
				},
				_ => f,
			}
		});

		Self {
			name,
			parent,
			bin,
			version,
			description,
			data,
			more,
			flags,
			fname: bash_subfname(parent.map(str::as_bytes).unwrap_or_default(), bin.as_bytes())
		}
	}
}

/// # Getters.
impl<'a> Command<'a> {
	#[must_use]
	/// # Bin (cmd).
	const fn bin(&self) -> &'a str { self.bin }

	#[must_use]
	/// # Description.
	const fn description(&self) -> &'a str { self.description }

	#[must_use]
	/// # Name.
	const fn name(&self) -> &'a str { self.name }

	#[must_use]
	/// # Version.
	const fn version(&self) -> &'a str { self.version }
}

/// # Bash.
impl<'a> Command<'a> {
	/// # Write Bash.
	pub(crate) fn write_bash(&self, path: &Path, buf: &mut Vec<u8>) -> Result<(), BashManError> {
		// No subcommands.
		if 0 == self.flags & FLAG_SUBCOMMANDS {
			self.bash_completions(buf)?;
			writeln!(
				buf,
				"complete -F {} -o bashdefault -o default {}",
				self.fname,
				self.bin
			)
				.map_err(|_| BashManError::WriteBash)?;
		}
		// Subcommands.
		else {
			self.data.iter()
				.try_for_each(|x| {
					if let DataKind::SubCommand(x) = x {
						x.bash_completions(buf)?;
					}

					Ok(())
				})?;

			self.bash_completions(buf)?;
			self.bash_subcommands(buf)?;
		}

		// We might be able to compress the output a bit by removing contiguous
		// line breaks.
		{
			let mut last: u8 = b'?';
			buf.retain(|&b|
				if b == b'\n' && b == last { false }
				else {
					last = b;
					true
				}
			);
		}

		// Write it to a file!
		let mut out_file = path.to_path_buf();
		out_file.push(self.bin.to_string() + ".bash");
		File::create(&out_file)
			.and_then(|mut f| f.write_all(buf).and_then(|_| f.flush()))
			.map_err(|_| BashManError::WriteBash)?;

		Msg::success(format!("BASH completions written to: {path:?}")).print();
		Ok(())
	}

	/// # BASH Helper (Completions).
	///
	/// This generates the completions for a given app or subcommand. The
	/// output is combined with other code to produce the final script returned
	/// by the main [`Agree::bash`] method.
	fn bash_completions(&self, buf: &mut Vec<u8>) -> Result<(), BashManError> {
		buf.extend_from_slice(self.fname.as_bytes());
		buf.extend_from_slice(br#"() {
	local cur prev opts
	COMPREPLY=()
	cur="${COMP_WORDS[COMP_CWORD]}"
	prev="${COMP_WORDS[COMP_CWORD-1]}"
	opts=()

"#);

		self.data.iter()
			.try_for_each(|x| {
				x.write_bash(buf)
			})?;

		buf.extend_from_slice(br#"
	opts=" ${opts[@]} "
	if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
		COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
		return 0
	fi

"#);

		if 0 != self.flags & FLAG_PATH_OPTIONS { self.bash_paths(buf)?; }

		buf.extend_from_slice(br#"
	COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
	return 0
}
"#);
		Ok(())
	}

	/// # BASH Helper (Path Options).
	///
	/// This produces the file/directory-listing portion of the BASH completion
	/// script for cases where the last option entered expects a path. It is
	/// integrated into the main [`Agree::bash`] output.
	fn bash_paths(&self, buf: &mut Vec<u8>) -> Result<(), BashManError> {
		let keys: Vec<&str> = self.data.iter()
			.filter_map(|o| o.and_path_option().and_then(|o| o.flag.short))
			.chain(
				self.data.iter()
					.filter_map(|o| o.and_path_option().and_then(|o| o.flag.long))
			)
			.collect();

		if ! keys.is_empty() {
			write!(
				buf,
				r#"	case "${{prev}}" in
		{})
			if [ -z "$( declare -f _filedir )" ]; then
				COMPREPLY=( $( compgen -f "${{cur}}" ) )
			else
				COMPREPLY=( $( _filedir ) )
			fi
			return 0
			;;
		*)
			COMPREPLY=()
			;;
	esac
"#,
				&keys.join("|")
			)
				.map_err(|_| BashManError::WriteBash)?;
		}

		Ok(())
	}

	/// # BASH Helper (Subcommand Chooser).
	///
	/// This generates an additional method for applications with subcommands
	/// to allow per-command suggestions. The output is incorporated into the
	/// value returned by [`Agree::bash`].
	fn bash_subcommands(&self, buf: &mut Vec<u8>) -> Result<(), BashManError> {
		use std::fmt::Write;
		let (cmd, chooser) = std::iter::once((self.bin, &self.fname))
			.chain(
				self.data.iter()
					.filter_map(|x|
						if let DataKind::SubCommand(c) = x {
							Some((c.bin, &c.fname))
						}
						else { None }
					)
			)
			.fold(
				(String::new(), String::new()),
				|(mut a, mut b), (c, d)| {
					let _res = writeln!(a, "\
						\t\t\t{})\n\
						\t\t\t\tcmd=\"{}\"\n\
						\t\t\t\t;;",
						&c, &c
					);
					let _res = writeln!(b, "\
						\t\t{})\n\
						\t\t\t{}\n\
						\t\t\t;;",
						&c,
						&d
					);

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
			fname=self.fname,
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
	pub(crate) fn write_man(&self, path: &Path, buf: &mut Vec<u8>) -> Result<(), BashManError> {
		// Main manual first.
		self.man(buf)?;
		man_escape(buf);

		let mut out_file = path.to_path_buf();
		out_file.push(self.bin.to_string() + ".1");
		self._write_man(&out_file, buf)?;

		// All the subcommands.
		if 0 != self.flags & FLAG_SUBCOMMANDS {
			self.data.iter().try_for_each(|o| {
				if let DataKind::SubCommand(o) = o {
					buf.truncate(0);
					o.man(buf)?;
					man_escape(buf);

					out_file.pop();
					out_file.push(format!(
						"{}-{}.1",
						self.bin,
						o.bin
					));

					o._write_man(&out_file, buf)?;
				}

				Ok(())
			})?;
		}

		Msg::success(format!("Man page(s) written to: {path:?}")).print();
		Ok(())
	}

	/// # Write For Real.
	fn _write_man(&self, path: &Path, data: &[u8]) -> Result<(), BashManError> {
		// Write plain.
		File::create(path)
			.and_then(|mut f| f.write_all(data).and_then(|_| f.flush()))
			.map_err(|_| BashManError::WriteSubMan(Box::from(self.bin)))?;

		// Write compressed.
		let mut writer = Compressor::new(CompressionLvl::best());
		let mut buf: Vec<u8> = vec![0; writer.gzip_compress_bound(data.len())];

		// Trim any excess now that we know the final length.
		let len = writer.gzip_compress(data, &mut buf)
			.map_err(|_| BashManError::WriteSubMan(Box::from(self.bin)))?;
		buf.truncate(len);

		// Toss ".gz" onto the original file path and write again!
		let mut dst = path.to_path_buf();
		dst.as_mut_os_string().push(".gz");
		File::create(dst)
			.and_then(|mut f| f.write_all(&buf).and_then(|_| f.flush()))
			.map_err(|_| BashManError::WriteSubMan(Box::from(self.bin)))
	}

	/// # Manual!
	fn man(&self, buf: &mut Vec<u8>) -> Result<(), BashManError> {
		let now = utc2k::Utc2k::now();

		// Start with the header.
		match self.parent {
			Some(p) => write!(
				buf,
				r#".TH "{} {}" "1" "{} {}" "{} v{}" "User Commands""#,
				p.to_uppercase(),
				self.name().to_uppercase(),
				now.month_name(),
				now.year(),
				self.name(),
				self.version(),
			),
			None => write!(
				buf,
				r#".TH "{}" "1" "{} {}" "{} v{}" "User Commands""#,
				self.name().to_uppercase(),
				now.month_name(),
				now.year(),
				self.name(),
				self.version(),
			),
		}
			.map_err(|_| BashManError::WriteSubMan(Box::from(self.bin)))?;

		// Helper: Generic section writer.
		macro_rules! write_section {
			($label:expr, $indent:expr, $data:expr) => {
				More { label: $label, indent: $indent, data: $data }.man(buf)?;
			};
			($label:literal, $arr:ident) => {
				if ! $arr.is_empty() { write_section!($label, true, $arr); }
			};
		}

		// Name.
		write_section!(
			"NAME",
			false,
			vec![DataKind::Paragraph(vec![&format!(
				"{} - Manual page for {} v{}.",
				self.name(),
				self.bin,
				self.version()
			)])]
		);

		// Description.
		write_section!(
			"DESCRIPTION",
			false,
			vec![DataKind::Paragraph(vec![self.description()])]
		);

		// Usage.
		write_section!(
			"USAGE:",
			true,
			vec![DataKind::Paragraph(vec![&self.man_usage()])]
		);

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
				subs.push(DataKind::Item(DataItem {
					label: s.bin(),
					description: s.description(),
				}));
			},
			_ => {},
		});

		// Now print each section.
		write_section!("FLAGS:", flags);
		write_section!("OPTIONS:", opts);

		args.into_iter().try_for_each(|(label, data)| {
			More {
				label: &label,
				indent: true,
				data: vec![data],
			}.man(buf)
		})?;

		write_section!("SUBCOMMANDS:", subs);

		// Random sections.
		self.more.iter().try_for_each(|x| x.man(buf))?;

		Ok(())
	}

	/// # Man usage.
	fn man_usage(&self) -> String {
		let mut out: String = self.parent.map_or_else(
			|| self.bin.to_string(),
			|p| format!("{p} {}", self.bin),
		);

		if 0 != self.flags & FLAG_SUBCOMMANDS {
			out.push_str(" [SUBCOMMAND]");
		}

		if 0 != self.flags & FLAG_SWITCHES {
			out.push_str(" [FLAGS]");
		}

		if 0 != self.flags & FLAG_OPTIONS {
			out.push_str(" [OPTIONS]");
		}

		if 0 != self.flags & FLAG_ARGUMENTS {
			if let Some(s) = self.data.iter().find_map(|o| match o {
				DataKind::Arg(s) => Some(s.label),
				_ => None,
			}) {
				out.push(' ');
				out.push_str(s);
			}
		}

		out
	}
}

/// # Credits.
impl<'a> Command<'a> {
	/// # Write Bash.
	pub(crate) fn write_credits(
		&self,
		manifest: &Path,
		features: Option<&str>,
		dir: &Path,
		buf: &mut Vec<u8>,
	) -> Result<(), BashManError> {
		// Get the dependencies.
		let raw = crate::credits::get_dependencies(manifest, features)?;

		// Write the header.
		buf.extend_from_slice(format!(
			"# Project Dependencies\n    Package:   {}\n    Version:   {}\n    Generated: {} UTC\n",
			self.bin,
			self.version,
			Utc2k::now(),
		).as_bytes());

		// No dependencies.
		if raw.len() < 2 {
			buf.extend_from_slice(b"\nThis package has no dependencies.\n");
		}
		// Some dependencies.
		else {
			buf.extend_from_slice(b"\n| Package | Version | Author(s) | License |\n| ---- | ---- | ---- | ---- |\n");

			for x in raw {
				if x.name == self.bin { continue; }
				if let Some(link) = x.link {
					writeln!(
						buf,
						"| [{}]({}) | {} | {} | {} |",
						x.name,
						link,
						x.version,
						x.authors,
						x.license,
					).map_err(|_| BashManError::WriteCredits)?;
				}
				else {
					writeln!(
						buf,
						"| {} | {} | {} | {} |",
						x.name,
						x.version,
						x.authors,
						x.license,
					).map_err(|_| BashManError::WriteCredits)?;
				}
			}
		}

		let mut out = dir.to_path_buf();
		out.push("CREDITS.md");

		// Write plain.
		File::create(&out)
			.and_then(|mut f| f.write_all(buf).and_then(|_| f.flush()))
			.map_err(|_| BashManError::WriteCredits)?;

		Msg::success(format!("Credits written to: {out:?}")).print();
		Ok(())
	}
}



#[derive(Debug, Clone)]
/// # Misc Metadata Section.
pub(super) struct More<'a> {
	pub(crate) label: &'a str,
	pub(crate) indent: bool,
	pub(crate) data: Vec<DataKind<'a>>,
}

/// # Manual.
impl<'a> More<'a> {
	/// # Manual.
	fn man(&self, buf: &mut Vec<u8>) -> Result<(), BashManError> {
		// Start with the header.
		if self.indent {
			buf.extend_from_slice(b"\n.SS ");
		}
		else {
			buf.extend_from_slice(b"\n.SH ");
		}

		match (self.indent, self.label.ends_with(':')) {
			// Indented sections need a trailing colon.
			(true, false) => {
				buf.extend_from_slice(self.label.as_bytes());
				buf.push(b':');
			},
			// Unindented sections should not have a trailing colon.
			(false, true) => {
				buf.extend_from_slice(self.label[..self.label.len() - 1].as_bytes());
			},
			// The label is fine as is.
			_ => {
				buf.extend_from_slice(self.label.as_bytes());
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
pub(super) enum DataKind<'a> {
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
				s.duplicate,
			),
			Self::Option(s) => bash_long_short_conds(
				buf,
				s.flag.short,
				s.flag.long,
				s.flag.duplicate,
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
		macro_rules! push_desc {
			($desc:expr) => {
				if self.man_tagline(buf)? {
					buf.push(b'\n');
					buf.extend_from_slice($desc.as_bytes());
				}
				else {
					buf.extend_from_slice(b"\n.TP\n");
					buf.extend_from_slice($desc.as_bytes());
				}
			};
		}
		match self {
			Self::Switch(i) => {
				push_desc!(i.description);
			},
			Self::Option(i) => {
				push_desc!(i.flag.description);
			},
			Self::Arg(i) | Self::Item(i) => {
				push_desc!(i.description);
			},
			Self::Paragraph(i) => {
				if indent {
					buf.extend_from_slice(b"\n.TP\n");
				}
				else {
					buf.push(b'\n');
				}
				buf.extend_from_slice(i.join("\n.RE\n").as_bytes());
			},
			Self::SubCommand(_) => {},
		}

		Ok(())
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
pub(super) struct DataFlag<'a> {
	pub(crate) short: Option<&'a str>,
	pub(crate) long: Option<&'a str>,
	pub(crate) description: &'a str,
	pub(crate) duplicate: bool,
}



#[derive(Debug, Copy, Clone)]
/// # Misc Item.
pub(super) struct DataItem<'a> {
	pub(crate) label: &'a str,
	pub(crate) description: &'a str,
}



#[derive(Debug, Copy, Clone)]
/// # Option.
pub(super) struct DataOption<'a> {
	pub(crate) flag: DataFlag<'a>,
	pub(crate) label: &'a str,
	pub(crate) path: bool,
}



#[inline]
/// # Bash Char.
const fn bash_bytes(b: u8) -> u8 {
	match b {
		b'a'..=b'z' | b'0'..=b'9' => b,
		b'A'..=b'Z' => b | 32_u8,
		_ => b'_',
	}
}

/// # Bash (Sub)Function Name.
fn bash_subfname(parent: &[u8], bin: &[u8]) -> Box<str> {
	let mut v: Vec<u8> = Vec::with_capacity(10 + parent.len() + bin.len());
	v.extend_from_slice(b"_basher__");
	if ! parent.is_empty() {
		v.extend(parent.iter().map(|&b| bash_bytes(b)));
	}
	v.push(b'_');
	v.extend(bin.iter().map(|&b| bash_bytes(b)));

	String::from_utf8_lossy(&v).into_owned().into_boxed_str()
}

/// # Bash Helper (Long/Short Conds)
fn bash_long_short_conds(
	buf: &mut Vec<u8>,
	short: Option<&str>,
	long: Option<&str>,
	duplicate: bool,
) -> Result<(), BashManError> {
	match (short, long) {
		(Some(s), Some(l)) =>
			if duplicate {
				writeln!(buf, "\topts+=(\"{s}\")\n\topts+=(\"{l}\")")
			}
			else {
				write!(
					buf,
					r#"	if [[ ! " ${{COMP_LINE}} " =~ " {s} " ]] && [[ ! " ${{COMP_LINE}} " =~ " {l} " ]]; then
		opts+=("{s}")
		opts+=("{l}")
	fi
"#,
				)
			}
			.map_err(|_| BashManError::WriteBash),
		(None, Some(k)) | (Some(k), None) =>
			if duplicate { writeln!(buf, "\topts+=(\"{k}\")") }
			else {
				writeln!(
					buf,
					"\t[[ \" ${{COMP_LINE}} \" =~ \" {k} \" ]] || opts+=(\"{k}\")",
				)
			}
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
			write!(buf, "\n.TP\n\\fB{s}\\fR, \\fB{l}\\fR {v}")
				.map_err(|_| BashManError::WriteMan)?;
			Ok(true)
		},
		// Option: long or short.
		(Some(k), None, Some(v)) | (None, Some(k), Some(v)) => {
			write!(buf, "\n.TP\n\\fB{k}\\fR {v}")
				.map_err(|_| BashManError::WriteMan)?;
			Ok(true)
		},
		// Switch: long and short.
		(Some(s), Some(l), None) => {
			write!(buf, "\n.TP\n\\fB{s}\\fR, \\fB{l}\\fR")
				.map_err(|_| BashManError::WriteMan)?;
			Ok(true)
		},
		// Switch: long or short.
		// Key/Value.
		(Some(k), None, None) | (None, Some(k), None) | (None, None, Some(k)) => {
			write!(buf, "\n.TP\n\\fB{k}\\fR")
				.map_err(|_| BashManError::WriteMan)?;
			Ok(true)
		},
		_ => Ok(false),
	}
}
