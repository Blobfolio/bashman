/*!
# Cargo BashMan: Manual Pages.
*/

use crate::{
	BashManError,
	Flag,
	Manifest,
	OptionFlag,
	Subcommand,
	TrailingArg,
};
use libdeflater::{
	CompressionLvl,
	Compressor,
};
use std::{
	borrow::Cow,
	fmt,
	path::{
		Path,
		PathBuf,
	},
};
use utc2k::Utc2k;



/// # Args Section Label.
const LABEL_ARGS: &str = "TRAILING:";

/// # Subcommands Section Label.
const LABEL_SUBCOMMANDS: &str = "SUBCOMMANDS:";



/// # Manual Page(s) Writer.
///
/// This struct is used to write manual page(s) for each (sub)command in a
/// `Manifest`.
///
/// The magic is largely handled through the `Display` impls of this and
/// supporting sub-structures, but `ManWriter::write` is what actually makes
/// the call and saves the file(s).
pub(super) struct ManWriter<'a> {
	/// # Output Directory.
	dir: PathBuf,

	/// # Man Pages.
	men: Vec<Man<'a>>,
}

impl<'a> TryFrom<&'a Manifest> for ManWriter<'a> {
	type Error = BashManError;

	fn try_from(src: &'a Manifest) -> Result<Self, Self::Error> {
		let dir = src.dir_man()?;
		let subcommands = src.subcommands();
		if subcommands.is_empty() { return Err(BashManError::Man); }

		// Build the individual `Man` instances, even if just one.
		let mut men = Vec::with_capacity(subcommands.len());
		for sub in subcommands {
			let mut entry = Man::from(sub);

			// Populate or remove the subcommand section if this is the main
			// command.
			if sub.is_main() {
				if let Some(pos) = entry.sections.iter().position(|s| s.label == LABEL_SUBCOMMANDS) {
					entry.sections[pos].data.extend(
						subcommands.iter().filter_map(|s|
							if s.is_main() { None }
							else { Some(SectionData::from(s)) }
						)
					);

					// Remove it.
					if entry.sections[pos].data.is_empty() { entry.sections.remove(pos); }
					// Keep it!
					else { entry.toc |= Man::HAS_SUBCOMMANDS; }
				}
			}

			men.push(entry);
		}

		Ok(Self { dir, men })
	}
}

impl<'a> ManWriter<'a> {
	/// # Write to File.
	///
	/// This method is called by `main.rs` to generate and save the manual
	/// page(s), including gzip copies.
	///
	/// The shared `buf` is used to help reduce allocations across the various
	/// writes the program will make.
	///
	/// Errors will be bubbled up if encountered, otherwise the output path(s)
	/// are returned.
	pub(super) fn write(self, buf: &mut String) -> Result<Vec<PathBuf>, BashManError> {
		use std::fmt::Write;

		let mut done = Vec::new(); // Output paths.
		let mut gz = Vec::new();   // Gzip buffer.

		// A page for every man!
		let Self { dir, men } = self;
		for man in men {
			// Generate and gzip.
			buf.truncate(0);
			write!(buf, "{man}").map_err(|_| BashManError::Man)?;
			gzip(buf.as_bytes(), &mut gz)?;

			// Figure out the flie names.
			let dst1 = output_file(&dir, man.parent_cmd, man.cmd);
			let mut dst2 = dst1.clone();
			dst2.as_mut_os_string().push(".gz");

			write_atomic::write_file(&dst1, buf.as_bytes())
				.and_then(|()| write_atomic::write_file(&dst2, &gz))
				.map_err(|_| BashManError::Man)?;

			done.push(dst1);
			done.push(dst2);
		}

		if done.is_empty() { Err(BashManError::Man) }
		else {
			if 2 < done.len() { done.sort_unstable(); }
			Ok(done)
		}
	}
}





/// # Manual Page (Individual).
///
/// This struct is used to write a _single_ manual page for a given
/// (sub)command. As with `ManWriter`, the magic is handled by its `Display`
/// impl.
struct Man<'a> {
	/// # Parent Nice Name.
	parent_name: Option<&'a str>,

	/// # Parent Command.
	parent_cmd: Option<&'a str>,

	/// # Nice Name.
	name: &'a str,

	/// # (Sub)command.
	cmd: &'a str,

	/// # Version.
	version: EscapeHyphens<'a>,

	/// # Description.
	description: EscapeHyphens<'a>,

	/// # Table of Contents.
	///
	/// This encodes the available sections with relevance to the USAGE line.
	toc: u8,

	/// # Sections.
	sections: Vec<Section<'a>>,
}

impl<'a> fmt::Display for Man<'a> {
	/// # Write Section.
	///
	/// This generates appropriate man code for the section.
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		// Start with the header.
		let now = Utc2k::now();
		let full_name = self.parent_name.map_or_else(
			|| self.name.to_uppercase(),
			|p| format!("{p} {}", self.name).to_uppercase(),
		).replace('"', "");
		let full_cmd = self.parent_cmd.map_or(
			Cow::Borrowed(self.cmd),
			|p| Cow::Owned(format!("{p} {}", self.cmd)),
		);

		writeln!(
			f,
			r#".TH "{}" "1" "{} {}" "{} v{}" "User Commands""#,
			EscapeHyphens(full_name.as_str()),
			now.month_name(),
			now.year(),
			EscapeHyphens(full_cmd.as_ref()),
			self.version,
		)?;

		// Name.
		writeln!(
			f,
			".SH NAME\n{} \\- Manual page for {} v{}.",
			EscapeHyphens(&self.name.to_uppercase()),
			EscapeHyphens(full_cmd.as_ref()),
			self.version,
		)?;

		// Description.
		writeln!(f, ".SH DESCRIPTION\n{}", self.description)?;

		// Usage.
		write!(
			f,
			".SS USAGE:\n.TP\n{}{}{}{}",
			EscapeHyphens(full_cmd.as_ref()),
			if Self::HAS_SUBCOMMANDS == self.toc & Self::HAS_SUBCOMMANDS { " [SUBCOMMAND]" } else { "" },
			if Self::HAS_FLAGS == self.toc & Self::HAS_FLAGS { " [FLAGS]" } else { "" },
			if Self::HAS_OPTIONS == self.toc & Self::HAS_OPTIONS { " [OPTIONS]" } else { "" },
		)?;
		if let Some(arg) = self.arg_label() { writeln!(f, " {arg}") }
		else { writeln!(f) }?;

		// Everything else!
		for line in &self.sections { <Section as fmt::Display>::fmt(line, f)? }

		Ok(())
	}
}

impl<'a> Man<'a> {
	/// # Has Flags?
	const HAS_FLAGS: u8 =       0b0001;

	/// # Has Options?
	const HAS_OPTIONS: u8 =     0b0010;

	/// # Has Args?
	const HAS_ARGS: u8 =        0b0100;

	/// # Has Subcommands?
	const HAS_SUBCOMMANDS: u8 = 0b1000;

	/// # Arg Label.
	///
	/// Return the value label used for trailing arguments, if any.
	fn arg_label(&self) -> Option<EscapeHyphens> {
		if Self::HAS_ARGS == self.toc & Self::HAS_ARGS {
			self.sections.iter().find_map(|s|
				if s.label == LABEL_ARGS {
					s.data.first().and_then(|d| d.label)
				}
				else { None }
			)
		}
		else { None }
	}
}

impl<'a> From<&'a Subcommand> for Man<'a> {
	fn from(src: &'a Subcommand) -> Self {
		let mut out = Self {
			parent_name: src.parent_nice_name(),
			parent_cmd: src.parent_bin(),
			name: src.nice_name(),
			cmd: src.bin(),
			version: EscapeHyphens(src.version()),
			description: EscapeHyphens(src.description()),
			toc: 0,
			sections: Vec::new(),
		};

		// Flags, options, args, then sections.
		let data = src.data();

		let tmp = data.flags();
		if ! tmp.is_empty() {
			out.toc |= Self::HAS_FLAGS;
			out.sections.push(Section {
				label: "FLAGS:",
				indent: true,
				data: tmp.iter().map(SectionData::from).collect(),
			});
		}

		let tmp = data.options();
		if ! tmp.is_empty() {
			out.toc |= Self::HAS_OPTIONS;
			out.sections.push(Section {
				label: "OPTIONS:",
				indent: true,
				data: tmp.iter().map(SectionData::from).collect(),
			});
		}

		if let Some(tmp) = data.args() {
			out.toc |= Self::HAS_ARGS;
			out.sections.push(Section {
				label: LABEL_ARGS,
				indent: true,
				data: vec![SectionData::from(tmp)],
			});
		}

		// Reserve a spot for subcommands if this is the primary command.
		// We'll populate or remove it later.
		if src.is_main() {
			out.sections.push(Section {
				label: LABEL_SUBCOMMANDS,
				indent: true,
				data: Vec::new(),
			});
		}

		// Sections require a touch more.
		for tmp in data.sections() {
			let mut label = tmp.name();
			let indent = tmp.inside();
			let mut inner = Vec::new();
			if let Some(lines) = tmp.lines() {
				inner.push(SectionData::from(lines));
			}
			if let Some(items) = tmp.items() {
				inner.extend(items.iter().map(SectionData::from));
			}

			// If this section isn't indented, we need to modify a few things.
			if ! indent {
				label = label.trim_end_matches(|c: char| c == ':' || c.is_whitespace());
				for v in &mut inner { v.indent = false; }
			}

			out.sections.push(Section { label, indent, data: inner });
		}

		out
	}
}



/// # Arbitrary Section.
///
/// This struct is used to generate an individual manual page section.
struct Section<'a> {
	/// # Label.
	label: &'a str,

	/// # Indent?
	indent: bool,

	/// # Data.
	data: Vec<SectionData<'a>>,
}

impl<'a> fmt::Display for Section<'a> {
	/// # Write Section.
	///
	/// This generates appropriate man code for the section.
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if self.indent { writeln!(f, ".SS {}", EscapeHyphens(self.label))?; }
		else { writeln!(f, ".SH {}", EscapeHyphens(self.label))?; }

		// Print the data.
		for line in &self.data { <SectionData as fmt::Display>::fmt(line, f)?; }

		Ok(())
	}
}



/// # Section Data.
///
/// This struct is used to hold/print arbitrary section data. It makes heavy
/// use of `Option` in order to accommodate keys, args, and custom stuff.
struct SectionData<'a> {
	/// # Short Key.
	short: Option<EscapeHyphens<'a>>,

	/// # Long Key.
	long: Option<EscapeHyphens<'a>>,

	/// # Label.
	label: Option<EscapeHyphens<'a>>,

	/// # Description.
	description: EscapeHyphens<'a>,

	/// # Indent?
	indent: bool,
}

impl<'a> fmt::Display for SectionData<'a> {
	/// # Write Entry.
	///
	/// This generates appropriate man code for a given data based on the
	/// available members.
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match (self.short, self.long, self.label) {
			// Everything!
			(Some(short), Some(long), Some(val)) => writeln!(
				f,
				".TP\n\\fB{short}\\fR, \\fB{long}\\fR {val}\n{}",
				self.description,
			),
			// Key and value.
			(Some(key), None, Some(val)) | (None, Some(key), Some(val)) => writeln!(
				f,
				".TP\n\\fB{key}\\fR {val}\n{}",
				self.description,
			),
			// Two keys.
			(Some(short), Some(long), None) => writeln!(
				f,
				".TP\n\\fB{short}\\fR, \\fB{long}\\fR\n{}",
				self.description,
			),
			// One thing.
			(Some(key), None, None) | (None, Some(key), None) | (None, None, Some(key)) => writeln!(
				f,
				".TP\n\\fB{key}\\fR\n{}",
				self.description,
			),
			// Just a paragraph.
			_ => {
				// Add indentation if necessary.
				if self.indent { f.write_str(".TP\n")?; }
				writeln!(f, "{}", self.description)
			},
		}
	}
}

impl<'a> From<&'a Flag> for SectionData<'a> {
	#[inline]
	fn from(src: &'a Flag) -> Self {
		Self {
			short: src.short().map(EscapeHyphens),
			long: src.long().map(EscapeHyphens),
			label: None,
			description: EscapeHyphens(src.description()),
			indent: true,
		}
	}
}

impl<'a> From<&'a OptionFlag> for SectionData<'a> {
	#[inline]
	fn from(src: &'a OptionFlag) -> Self {
		Self {
			short: src.short().map(EscapeHyphens),
			long: src.long().map(EscapeHyphens),
			label: Some(EscapeHyphens(src.label())),
			description: EscapeHyphens(src.description()),
			indent: true,
		}
	}
}

impl<'a> From<&'a [String; 2]> for SectionData<'a> {
	#[inline]
	fn from(src: &'a [String; 2]) -> Self {
		Self {
			short: None,
			long: Some(EscapeHyphens(src[0].as_str())),
			label: None,
			description: EscapeHyphens(src[1].as_str()),
			indent: true,
		}
	}
}

impl<'a> From<&'a str> for SectionData<'a> {
	#[inline]
	fn from(src: &'a str) -> Self {
		Self {
			short: None,
			long: None,
			label: None,
			description: EscapeHyphens(src),
			indent: true,
		}
	}
}

impl<'a> From<&'a Subcommand> for SectionData<'a> {
	#[inline]
	fn from(src: &'a Subcommand) -> Self {
		Self {
			short: None,
			long: Some(EscapeHyphens(src.bin())),
			label: None,
			description: EscapeHyphens(src.description()),
			indent: true,
		}
	}
}

impl<'a> From<&'a TrailingArg> for SectionData<'a> {
	#[inline]
	fn from(src: &'a TrailingArg) -> Self {
		Self {
			short: None,
			long: None,
			label: Some(EscapeHyphens(src.label())),
			description: EscapeHyphens(src.description()),
			indent: true,
		}
	}
}



#[derive(Debug, Clone, Copy)]
/// # Escape Hyphens.
struct EscapeHyphens<'a>(&'a str);

impl<'a> fmt::Display for EscapeHyphens<'a> {
	/// # Write Escaped.
	///
	/// MAN pages don't seem to like hyphens; this will escape any as they're
	/// encountered.
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		for part in self.0.split_inclusive('-') {
			if let Some(rest) = part.strip_suffix('-') {
				if ! rest.is_empty() { f.write_str(rest)?; }
				f.write_str(r"\-")?;
			}
			else if ! part.is_empty() { f.write_str(part)?; }
		}
		Ok(())
	}
}



/// # Gzip Encode.
fn gzip(src: &[u8], dst: &mut Vec<u8>) -> Result<(), BashManError> {
	let mut writer = Compressor::new(CompressionLvl::best());
	dst.resize(writer.gzip_compress_bound(src.len()), 0);
	let len = writer.gzip_compress(src, dst).map_err(|_| BashManError::Man)?;
	dst.truncate(len); // Trim the extra.
	Ok(())
}

/// # Output File Name.
fn output_file(dir: &Path, parent_cmd: Option<&str>, cmd: &str) -> PathBuf {
	parent_cmd.map_or_else(
		|| {
			let mut out = dir.join(cmd);
			out.as_mut_os_string().push(".1");
			out
		},
		|x| {
			let mut out = dir.join(x);
			let tmp = out.as_mut_os_string();
			tmp.push("-");
			tmp.push(cmd);
			tmp.push(".1");
			out
		}
	)
}



#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn t_manwriter() {
		let manifest = Manifest::from_test().expect("Manifest failed.");
		let writer = ManWriter::try_from(&manifest).expect("ManWriter failed.");
		assert_eq!(writer.men.len(), 1); // Just the one!

		// Test the page generates as expected (without saving anything).
		let mut expected = std::fs::read_to_string("skel/metadata.man")
			.expect("Missing skel/metadata.man");

		// Before we do that, though, we need to patch the date into our
		// reference output, as that always reflects the current time.
		let now = Utc2k::now();
		let pos = expected.find("MONTHNAME").expect("Missing MONTHNAME");
		expected.replace_range(pos + 10..pos + 14, &now.year().to_string());
		expected.replace_range(pos..pos + 9, now.month_name());

		// Test!
		assert_eq!(writer.men[0].to_string(), expected);
	}
}
