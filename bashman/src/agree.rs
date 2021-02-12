/*!
# FYI Menu: Agree
*/

use crate::BashManError;
use libdeflater::{
	CompressionLvl,
	Compressor,
};
use std::{
	ffi::OsStr,
	io::Write,
	os::unix::ffi::OsStrExt,
	path::{
		Path,
		PathBuf,
	},
};
use tendril::{
	StrTendril,
	format_tendril,
};



#[derive(Debug, Clone, Hash, Eq, PartialEq)]
/// # Agreement Kind.
///
/// This enum provides a more or less consistent interface for dealing with the
/// disparate argument/item types making up an application.
///
/// With the exception of [`AgreeKind::SubCommand`], each type has a
/// corresponding initialization method. The intention is you should never have
/// to import the underlying struct directly.
///
/// For example, to register a new [`AgreeKind::Switch`], just call
/// [`AgreeKind::switch`].
///
/// The enum additionally passes through the underyling structs' builder
/// methods. For example, you can add keys to options and switches using
/// [`AgreeKind::with_short`] or [`AgreeKind::with_long`], and for paragraphs,
/// you can add additional lines using [`AgreeKind::with_line`].
pub enum AgreeKind {
	/// # Switch.
	///
	/// This is a flag (true/false) with a short and/or long key and
	/// description.
	Switch(AgreeSwitch),

	/// # Option.
	///
	/// This is an option (that takes a value) with a short and/or long key and
	/// description. The value can be open-ended or path-based.
	Option(AgreeOption),

	/// # Argument.
	///
	/// A trailing argument with a label and description.
	Arg(AgreeItem),

	/// # Subcommand.
	///
	/// This is a recursive [`Agree`], complete with its own description,
	/// flags, etc.
	///
	/// When calling [`Agree::write_man`], separate manuals will be written for
	/// each subcommand, following a "{bin}-{subcommand}.1" naming scheme.
	///
	/// Take a look at the `man` example in this crate, and also the `fyi`
	/// bin's own `build.rs` for sample construction.
	///
	/// ## Safety
	///
	/// There is support for ONE LEVEL of subcommands. That is, the main
	/// [`Agree`] struct can have any number of subcommands among its
	/// arguments, however those subcommands CANNOT have their own
	/// sub-subcommands. Undefined things will happen if 2+ levels are
	/// included.
	SubCommand(Agree),

	/// # Miscellaneous K/V Item.
	///
	/// This is a miscellaneous key/value pair that can be used for custom MAN
	/// sections. See also [`AgreeSection`].
	Item(AgreeItem),

	/// # Paragraph.
	///
	/// This is a text block with one or more lines.
	Paragraph(AgreeParagraph),
}

impl AgreeKind {
	/// # New Switch.
	///
	/// This is a convenience method to register a new [`AgreeKind::Switch`].
	pub fn switch<S>(description: S) -> Self
	where S: Into<StrTendril> {
		Self::Switch(AgreeSwitch::new(description))
	}

	/// # New Option.
	///
	/// This is a convenience method to register a new [`AgreeKind::Option`].
	pub fn option<S>(value: S, description: S, path: bool) -> Self
	where S: Into<StrTendril> {
		Self::Option(AgreeOption::new(value, description, path))
	}

	/// # New Argument.
	///
	/// This is a convenience method to register a new [`AgreeKind::Arg`].
	pub fn arg<S>(name: S, description: S) -> Self
	where S: Into<StrTendril> {
		Self::Arg(AgreeItem::new(name, description))
	}

	/// # New K/V Item.
	///
	/// This is a convenience method to register a new [`AgreeKind::Item`].
	pub fn item<S>(name: S, description: S) -> Self
	where S: Into<StrTendril> {
		Self::Item(AgreeItem::new(name, description))
	}

	/// # New Argument.
	///
	/// This is a convenience method to register a new [`AgreeKind::Paragraph`].
	pub fn paragraph<S>(line: S) -> Self
	where S: Into<StrTendril> {
		Self::Paragraph(AgreeParagraph::new(line))
	}

	/// # With Line.
	///
	/// This is a convenience method that passes through to the underlying
	/// data's `with_line()` method, if any.
	///
	/// This can be used to force a line break between bits of text. Otherwise
	/// if you jam everything into one "line", it will just wrap as needed.
	///
	/// This has no effect unless the type is [`AgreeKind::Paragraph`].
	pub fn with_line<S>(self, line: S) -> Self
	where S: Into<StrTendril> {
		if let Self::Paragraph(s) = self {
			Self::Paragraph(s.with_line(line))
		}
		else { self }
	}

	/// # With Long.
	///
	/// This is a convenience method that passes through to the underlying
	/// data's `with_long()` method, if any.
	///
	/// Specify a long key, e.g. `--help`.
	///
	/// This has no effect unless the type is [`AgreeKind::Switch`] or
	/// [`AgreeKind::Option`].
	pub fn with_long<S>(self, key: S) -> Self
	where S: Into<StrTendril> {
		match self {
			Self::Switch(s) => Self::Switch(s.with_long(key)),
			Self::Option(s) => Self::Option(s.with_long(key)),
			_ => self,
		}
	}

	/// # With Short.
	///
	/// This is a convenience method that passes through to the underlying
	/// data's `with_short()` method, if any.
	///
	/// Specify a short key, e.g. `-h`.
	///
	/// This has no effect unless the type is [`AgreeKind::Switch`] or
	/// [`AgreeKind::Option`].
	pub fn with_short<S>(self, key: S) -> Self
	where S: Into<StrTendril> {
		match self {
			Self::Switch(s) => Self::Switch(s.with_short(key)),
			Self::Option(s) => Self::Option(s.with_short(key)),
			_ => self,
		}
	}

	/// # BASH Helper.
	///
	/// This formats the BASH flag/option conditions, if any, for the
	/// completion script. This partial value is incorporated into the full
	/// output produced by [`Agree::bash`].
	fn bash(&self) -> StrTendril {
		match self {
			Self::Switch(s) => bash_long_short_conds(
				s.short.as_deref(),
				s.long.as_deref(),
			),
			Self::Option(s) => bash_long_short_conds(
				s.short.as_deref(),
				s.long.as_deref(),
			),
			Self::SubCommand(s) => format_tendril!("\topts+=(\"{}\")\n", &s.bin),
			_ => StrTendril::new(),
		}
	}

	/// # Return if Arg.
	const fn if_arg(&self) -> Option<&AgreeItem> {
		if let Self::Arg(s) = self { Some(s) }
		else { None }
	}

	/// # Return if (Path) Option.
	const fn if_path_option(&self) -> Option<&AgreeOption> {
		if let Self::Option(s) = self {
			if s.path { Some(s) }
			else { None }
		}
		else { None }
	}

	/// # Return if Subcommand.
	const fn if_subcommand(&self) -> Option<&Agree> {
		if let Self::SubCommand(s) = self { Some(s) }
		else { None }
	}

	/// # MAN Helper.
	///
	/// This formats the MAN line(s) for the underlying data. This partial
	/// value is incorporated into the full output produced by [`Agree::man`].
	fn man(&self, indent: bool) -> StrTendril {
		match self {
			Self::Switch(i) => {
				let mut out: StrTendril = self.man_tagline();
				if out.is_empty() {
					format_tendril!(".TP\n{}", i.description)
				}
				else {
					out.push_char('\n');
					out.push_tendril(&i.description);
					out
				}
			},
			Self::Option(i) => {
				let mut out: StrTendril = self.man_tagline();
				if out.is_empty() {
					format_tendril!(".TP\n{}", i.description)
				}
				else {
					out.push_char('\n');
					out.push_tendril(&i.description);
					out
				}
			},
			Self::Arg(i) | Self::Item(i) => {
				let mut out: StrTendril = self.man_tagline();
				if out.is_empty() {
					format_tendril!(".TP\n{}", i.description)
				}
				else {
					out.push_char('\n');
					out.push_tendril(&i.description);
					out
				}
			},
			Self::Paragraph(i) => {
				if indent {
					format_tendril!(
						".TP\n{}",
						crate::tendril::join(&i.p, &StrTendril::from("\n.RE\n")),
					)
				}
				else {
					crate::tendril::join(&i.p, &StrTendril::from("\n.RE\n"))
				}
			},
			Self::SubCommand(s) => format_tendril!(
				"{}\n{}",
				self.man_tagline(),
				s.description,
			),
		}
	}

	/// # MAN Helper (Tagline).
	///
	/// This formats the key/value line for the MAN output. This gets
	/// incorporated into [`AgreeKind::man`], which gets incorporated into
	/// [`Agree::man`] to produce the full output.
	fn man_tagline(&self) -> StrTendril {
		match self {
			Self::Switch(s) => man_tagline(s.short.as_ref(), s.long.as_ref(), None),
			Self::Option(o) => man_tagline(o.short.as_ref(), o.long.as_ref(), Some(&o.value)),
			Self::Arg(k) | Self::Item(k) => man_tagline(None, None, Some(&k.name)),
			Self::SubCommand(s) => man_tagline(None, None, Some(&s.bin)),
			_ => StrTendril::new(),
		}
	}
}



#[derive(Debug, Clone, Hash, Eq, PartialEq)]
/// # Switch.
///
/// This holds the internal data for [`AgreeKind::Switch`]. It is public
/// because [`AgreeKind`] is public, but is not meant to be interacted with
/// directly. You should be using the passthrough methods provided by
/// [`AgreeKind`] instead.
pub struct AgreeSwitch {
	short: Option<StrTendril>,
	long: Option<StrTendril>,
	description: StrTendril,
}

impl AgreeSwitch {
	/// # New.
	pub fn new<S>(description: S) -> Self
	where S: Into<StrTendril> {
		Self {
			short: None,
			long: None,
			description: description.into(),
		}
	}

	/// # With Long.
	///
	/// Specify a long key, e.g. `--help`.
	pub fn with_long<S>(mut self, key: S) -> Self
	where S: Into<StrTendril> {
		self.long = Some(key.into());
		self
	}

	/// # With Short.
	///
	/// Specify a short key, e.g. `-h`.
	pub fn with_short<S>(mut self, key: S) -> Self
	where S: Into<StrTendril> {
		self.short = Some(key.into());
		self
	}
}



#[derive(Debug, Clone, Hash, Eq, PartialEq)]
/// # Option.
///
/// This holds the internal data for [`AgreeKind::Option`]. It is public
/// because [`AgreeKind`] is public, but is not meant to be interacted with
/// directly. You should be using the passthrough methods provided by
/// [`AgreeKind`] instead.
pub struct AgreeOption {
	short: Option<StrTendril>,
	long: Option<StrTendril>,
	value: StrTendril,
	path: bool,
	description: StrTendril,
}

impl AgreeOption {
	/// # New.
	///
	/// The `path` flag indicates whether or not this option expects some sort
	/// of file system path for its value. If `true`, the BASH completion will
	/// reveal files and folders in the current directory.
	pub fn new<S>(value: S, description: S, path: bool) -> Self
	where S: Into<StrTendril> {
		Self {
			short: None,
			long: None,
			value: value.into(),
			path,
			description: description.into(),
		}
	}

	#[must_use]
	/// # With Long.
	///
	/// Specify a long key, e.g. `--help`.
	pub fn with_long<S>(mut self, key: S) -> Self
	where S: Into<StrTendril> {
		self.long = Some(key.into());
		self
	}

	#[must_use]
	/// # With Short.
	///
	/// Specify a short key, e.g. `-h`.
	pub fn with_short<S>(mut self, key: S) -> Self
	where S: Into<StrTendril> {
		self.short = Some(key.into());
		self
	}
}



#[derive(Debug, Clone, Hash, Eq, PartialEq)]
/// # Item.
///
/// This holds the internal data for [`AgreeKind::Arg`] and [`AgreeKind::Item`].
/// It is public because [`AgreeKind`] is public, but is not meant to be
/// interacted with directly. You should be using the passthrough methods
/// provided by [`AgreeKind`] instead.
pub struct AgreeItem {
	name: StrTendril,
	description: StrTendril,
}

impl AgreeItem {
	/// # New.
	pub fn new<S>(name: S, description: S) -> Self
	where S: Into<StrTendril> {
		Self {
			name: name.into(),
			description: description.into(),
		}
	}
}



#[derive(Debug, Clone, Hash, Eq, PartialEq)]
/// # Paragraph.
///
/// This holds the internal data for [`AgreeKind::Paragraph`]. It is public
/// because [`AgreeKind`] is public, but is not meant to be interacted with
/// directly. You should be using the passthrough methods provided by
/// [`AgreeKind`] instead.
pub struct AgreeParagraph {
	p: Vec<StrTendril>,
}

impl Default for AgreeParagraph {
	fn default() -> Self {
		Self { p: Vec::new() }
	}
}

impl AgreeParagraph {
	/// # New.
	pub fn new<S>(line: S) -> Self
	where S: Into<StrTendril> {
		Self {
			p: vec![line.into()],
		}
	}

	/// # With Line.
	///
	/// This can be used to force a line break between bits of text. Otherwise
	/// if you jam everything into one "line", it will just wrap as needed.
	pub fn with_line<S>(mut self, line: S) -> Self
	where S: Into<StrTendril> {
		self.p.push(line.into());
		self
	}

	#[must_use]
	/// # Is Empty.
	pub fn is_empty(&self) -> bool { self.p.is_empty() }

	#[must_use]
	/// # Length.
	pub fn len(&self) -> usize { self.p.len() }
}



#[derive(Debug, Clone, Hash, Eq, PartialEq)]
/// # Section.
///
/// This struct represents a section of the MAN page. It can be used to add any
/// arbitrary content you want (on top of the default NAME/DESCRIPTION/USAGE
/// bits.)
pub struct AgreeSection {
	name: StrTendril,
	indent: bool,
	items: Vec<AgreeKind>
}

impl AgreeSection {
	/// # New.
	pub fn new<S>(name: S, indent: bool) -> Self
	where S: Into<StrTendril> {
		let mut name: StrTendril = name.into().to_uppercase().into();
		crate::tendril::trim(&mut name);
		if indent {
			if ! name.ends_with(':') {
				name.push_char(':');
			}
		}
		else if name.ends_with(':') {
			name.pop_back(1);
		}

		Self {
			name,
			indent,
			items: Vec::new(),
		}
	}

	#[must_use]
	/// # With Item.
	///
	/// Attach any sort of [`AgreeKind`] data to the list. Mixing and matching
	/// might look weird in a single section, but any type will do.
	pub fn with_item(mut self, item: AgreeKind) -> Self {
		self.items.push(item);
		self
	}

	/// # Push Item.
	///
	/// Attach any sort of [`AgreeKind`] data to the list. Mixing and matching
	/// might look weird in a single section, but any type will do.
	pub fn push_item(&mut self, item: AgreeKind) {
		self.items.push(item);
	}

	#[must_use]
	/// # Is Empty.
	///
	/// This returns `true` if no items are attached to the section.
	pub fn is_empty(&self) -> bool { self.items.is_empty() }

	#[must_use]
	/// # Length.
	///
	/// This returns the number of items attached to the section.
	pub fn len(&self) -> usize { self.items.len() }

	/// # MAN Helper.
	///
	/// This generates the MAN code for the section, which is incorporated by
	/// [`Agree::man`] to produce the full document.
	fn man(&self) -> StrTendril {
		// Start with the header.
		let mut out: StrTendril = format_tendril!(
			"{} {}",
			if self.indent { ".SS" } else { ".SH" },
			&self.name,
		);

		// Add the items one at a time.
		self.items.iter()
			.map(|i| i.man(self.indent))
			.for_each(|x| {
				out.push_char('\n');
				out.push_slice(&x);
			});

		// Done!
		out
	}
}



#[derive(Debug, Clone, Hash, Eq, PartialEq)]
/// # App Details.
///
/// [`Agree`] is a very crude, simple library to generate BASH completions
/// and/or MAN pages for apps.
///
/// The main idea is to toss a call to this in `build.rs`, keeping the
/// overhead out of the runtime application entirely.
///
/// It is constructed using builder patterns ([`Agree::with_arg`],
/// [`Agree::with_section`], etc.). Once set up, you can either obtain the
/// BASH completions and MAN page as a string ([`Agree::bash`] and
/// [`Agree::man`] respectively), or write them straight to files ([`Agree::write_bash`]
/// and [`Agree::write_man`] respectively).
///
/// The write methods are probably what you want.
///
/// Take a look at the crate examples or FYI's own `build.rs` for construction
/// and usage samples.
///
/// ## Safety
///
/// There is support for ONE LEVEL of subcommands. That is, the main [`Agree`]
/// struct can have any number of subcommands among its arguments, however
/// those subcommands CANNOT have their own sub-subcommands. Undefined things
/// will happen if 2+ levels are included.
pub struct Agree {
	name: StrTendril,
	bin: StrTendril,
	version: StrTendril,
	description: StrTendril,
	args: Vec<AgreeKind>,
	other: Vec<AgreeSection>,
}

impl Agree {
	/// # New.
	pub fn new<S>(name: S, description: S, bin: S, version: S) -> Self
	where S: Into<StrTendril> {
		Self {
			name: name.into(),
			bin: bin.into(),
			version: version.into(),
			description: description.into(),
			args: Vec::new(),
			other: Vec::new(),
		}
	}

	#[must_use]
	/// # With Arg.
	///
	/// Use this builder pattern method to attach every flag, option,
	/// trailing arg, and subcommand supported by your program.
	///
	/// When building manuals, these will automatically be separated out into
	/// appropriate sections for you.
	pub fn with_arg(mut self, arg: AgreeKind) -> Self {
		self.args.push(arg);
		self
	}

	/// # Push Arg.
	pub fn push_arg(&mut self, arg: AgreeKind) {
		self.args.push(arg);
	}

	#[must_use]
	/// # With Section.
	///
	/// Use this builder pattern method to attach arbitrary MAN sections to
	/// the app. Any sections you add here will appear after the default ones.
	pub fn with_section(mut self, section: AgreeSection) -> Self {
		self.other.push(section);
		self
	}

	/// # Push Section.
	///
	/// Use this builder pattern method to attach arbitrary MAN sections to
	/// the app. Any sections you add here will appear after the default ones.
	pub fn push_section(&mut self, section: AgreeSection) {
		self.other.push(section);
	}

	#[must_use]
	/// # BASH Completions.
	///
	/// Generate and return the code for a BASH completion script as a string.
	/// You can alternatively use [`Agree::write_bash`] to save this to a file
	/// instead.
	///
	/// The completions are set up such that suggestions will only appear once.
	/// That is, if you have a help flag and the line already includes `-h`, it
	/// will not suggest you add `--help`.
	///
	/// Completions are subcommand-aware. You can have different options for
	/// different subcommands, and/or options available only to the top-level
	/// bin.
	pub fn bash(&self) -> StrTendril {
		// Start by building all the subcommand code. We'll handle things
		// differently depending on whether or not the resulting string is
		// empty.
		let mut out: StrTendril = self.args.iter()
			.filter_map(|x| x.if_subcommand().and_then(|y| {
				let tmp = y.bash_completions(&self.bin);
				if tmp.is_empty() { None }
				else { Some(tmp) }
			}))
			.fold(StrTendril::new(), |mut a, b| {
				a.push_tendril(&b);
				a
			});

		// If this is empty, just add our app and call it quits.
		if out.is_empty() {
			return format_tendril!(
				"{}complete -F {} -o bashdefault -o default {}\n",
				self.bash_completions(""),
				&self.bash_fname(""),
				&self.bin
			);
		}

		// Add the app method.
		out.push_tendril(&self.bash_completions(""));

		// Add the function chooser.
		out.push_tendril(&self.bash_subcommands());

		// Done!
		out
	}

	#[must_use]
	/// # MAN Page.
	///
	/// Generate and return the code for a MAN page as a string. You can
	/// alternatively use [`Agree::write_man`] to save this to a file instead.
	///
	/// This automatically generates sections for `NAME`, `DESCRIPTION`, and
	/// `USAGE`, and if applicable, `FLAGS`, `OPTIONS`, trailing args, and
	/// `SUBCOMMANDS`.
	///
	/// If custom sections have been added, those will be printed after the
	/// above default sections.
	///
	/// Note: this will only return the manual for the top-level app. If there
	/// are subcommands, those pages will be ignored. To obtain those, call
	/// [`Agree::write_man`] instead.
	pub fn man(&self) -> StrTendril {
		self.subman("")
	}

	#[must_use]
	/// # Version.
	pub fn version(&self) -> &str {
		&self.version
	}

	/// # Write BASH Completions!
	///
	/// This will write the BASH completion script to the directory of your
	/// choosing, using the file name "{bin}.bash".
	pub fn write_bash<P>(&self, dir: P) -> Result<(), BashManError>
	where P: AsRef<Path> {
		let mut path = std::fs::canonicalize(dir.as_ref())
			.ok()
			.filter(|x| x.is_dir())
			.ok_or(BashManError::InvalidBashDir)?;

		path.push([&self.bin, ".bash"].concat());
		write_to(&path, self.bash().as_bytes(), false)
			.map_err(|_| BashManError::WriteBash(path))
	}

	/// # Write MAN Page!
	///
	/// This will write the MAN page(s) to the directory of your choosing,
	/// using the file name "{bin}.1" for the top-level app, and
	/// "{bin}-{subcommand}.1" for any specified subcommands.
	///
	/// This method will also write Gzipped copies of any manuals produced in
	/// case you want to use them for distribution (reducing the file size a
	/// bit).
	///
	/// You should only push one copy of each manual to `/usr/share/man/man1`,
	/// either the "{bin}.1" or "{bin}.1.gz" version, not both. ;)
	pub fn write_man<P>(&self, dir: P) -> Result<(), BashManError>
	where P: AsRef<Path> {
		let mut path = std::fs::canonicalize(dir.as_ref())
			.ok()
			.filter(|x| x.is_dir())
			.ok_or(BashManError::InvalidManDir)?;

		// The main file.
		path.push([&self.bin, ".1"].concat());
		write_to(&path, self.man().as_bytes(), true)
			.map_err(|_| BashManError::WriteMan(path.clone()))?;

		// Write subcommand pages.
		for (bin, man) in self.args.iter()
			.filter_map(|x| x.if_subcommand()
				.map(|x| (x.bin.clone(), x.subman(&self.bin)))
			)
		{
			path.pop();
			path.push([&self.bin, "-", &bin, ".1"].concat());
			write_to(&path, man.as_bytes(), true)
				.map_err(|_| BashManError::WriteMan(path.clone()))?;
		}

		Ok(())
	}

	/// # BASH Helper (Function Name).
	///
	/// This generates a unique-ish function name for use in the BASH
	/// completion script.
	fn bash_fname(&self, parent: &str) -> StrTendril {
		[
			"_basher__",
			parent,
			"_",
			&self.bin,
		].concat()
			.chars()
			.filter_map(|x| match x {
				'a'..='z' | '0'..='9' => Some(x),
				'A'..='Z' => Some(x.to_ascii_lowercase()),
				'-' | '_' | ' ' => Some('_'),
				_ => None,
			})
			.collect::<StrTendril>()
	}

	/// # BASH Helper (Completions).
	///
	/// This generates the completions for a given app or subcommand. The
	/// output is combined with other code to produce the final script returned
	/// by the main [`Agree::bash`] method.
	fn bash_completions(&self, parent: &str) -> StrTendril {
		format_tendril!(
			r#"{}() {{
	local cur prev opts
	COMPREPLY=()
	cur="${{COMP_WORDS[COMP_CWORD]}}"
	prev="${{COMP_WORDS[COMP_CWORD-1]}}"
	opts=()

{}
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
			&self.bash_fname(parent),
			&self.args.iter()
				.fold(StrTendril::new(), |mut a, b| {
					let txt: StrTendril = b.bash();
					if ! txt.is_empty() {
						a.push_tendril(&txt);
					}

					a
				}),
			&self.bash_paths(),
		)
	}

	/// # BASH Helper (Path Options).
	///
	/// This produces the file/directory-listing portion of the BASH completion
	/// script for cases where the last option entered expects a path. It is
	/// integrated into the main [`Agree::bash`] output.
	fn bash_paths(&self) -> StrTendril {
		let keys: Vec<&str> = self.args.iter()
			.filter_map(|o| o.if_path_option().and_then(|x| x.short.as_deref()))
			.chain(
				self.args.iter()
					.filter_map(|o| o.if_path_option().and_then(|x| x.long.as_deref()))
			)
			.collect();

		if keys.is_empty() { StrTendril::new() }
		else {
			format_tendril!(
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
	fn bash_subcommands(&self) -> StrTendril {
		let (cmd, chooser): (StrTendril, StrTendril) = std::iter::once((self.bin.clone(), self.bash_fname("")))
			.chain(
				self.args.iter()
					.filter_map(|x| x.if_subcommand()
						.map(|y| (y.bin.clone(), y.bash_fname(&self.bin)))
					)
			)
			.fold(
				(StrTendril::new(), StrTendril::new()),
				|(mut a, mut b), (c, d)| {
					a.push_tendril(&format_tendril!("\
						\t\t\t{})\n\
						\t\t\t\tcmd=\"{}\"\n\
						\t\t\t\t;;\n",
						&c, &c
					));
					b.push_tendril(&format_tendril!("\
						\t\t{})\n\
						\t\t\t{}\n\
						\t\t\t;;\n",
						&c,
						&d
					));

					(a, b)
				}
			);

		format_tendril!(
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
			fname=self.bash_fname(""),
			bname=self.bin,
			sub1=cmd,
			sub2=chooser
		)
	}

	/// # MAN Helper (Usage).
	///
	/// This generates an example command for the `USAGE` section, if any.
	fn man_usage(&self, parent: &str) -> StrTendril {
		let mut out: StrTendril = format_tendril!(
			"{} {}",
			parent,
			&self.bin
		);
		crate::tendril::trim(&mut out);

		if self.args.iter().any(|x| matches!(x, AgreeKind::SubCommand(_))) {
			out.push_slice(" [SUBCOMMAND]");
		}

		if self.args.iter().any(|x| matches!(x, AgreeKind::Switch(_))) {
			out.push_slice(" [FLAGS]");
		}

		if self.args.iter().any(|x| matches!(x, AgreeKind::Option(_))) {
			out.push_slice(" [OPTIONS]");
		}

		if let Some(s) = self.args.iter().find_map(AgreeKind::if_arg) {
			out.push_char(' ');
			out.push_tendril(&s.name);
		}

		out
	}

	/// # MAN Helper (Subcommands)
	///
	/// This produces the main manual content, varying based on whether or not
	/// this is for a subcommand. Its output is incorporated into the main
	/// [`Agree::man`] result.
	fn subman(&self, parent: &str) -> StrTendril {
		// Start with the header.
		let mut out: StrTendril = format_tendril!(
			r#".TH "{}" "1" "{}" "{} v{}" "User Commands""#,
			[&parent.to_uppercase(), " ", &self.name.to_uppercase()].concat().trim(),
			chrono::Local::now().format("%B %Y"),
			&self.name,
			&self.version,
		);

		// Add each section.
		let mut pre: Vec<AgreeSection> = vec![
			AgreeSection::new("NAME", false)
				.with_item(AgreeKind::paragraph(format_tendril!(
					"{} - Manual page for {} v{}.",
					&self.name,
					&self.bin,
					&self.version
				))),
				AgreeSection::new("DESCRIPTION", false)
					.with_item(AgreeKind::paragraph(self.description.clone())),
				AgreeSection::new("USAGE:", true)
					.with_item(AgreeKind::paragraph(self.man_usage(parent))),
		];

		// Generated FLAGS Section.
		{
			let section = self.args.iter()
				.filter(|x| matches!(x, AgreeKind::Switch(_)))
				.cloned()
				.fold(
					AgreeSection::new("FLAGS:", true),
					AgreeSection::with_item
				);
			if ! section.is_empty() {
				pre.push(section);
			}
		}

		// Generated OPTIONS Section.
		{
			let section = self.args.iter()
				.filter(|x| matches!(x, AgreeKind::Option(_)))
				.cloned()
				.fold(
					AgreeSection::new("OPTIONS:", true),
					AgreeSection::with_item
				);
			if ! section.is_empty() {
				pre.push(section);
			}
		}

		// Generated ARGUMENTS Section.
		self.args.iter()
			.filter_map(AgreeKind::if_arg)
			.for_each(|x| {
				pre.push(
					AgreeSection::new(format_tendril!("{}:", x.name), true)
						.with_item(AgreeKind::paragraph(x.description.clone()))
				);
			});

		// Generated SUBCOMMANDS Section.
		{
			let section = self.args.iter()
				.filter(|x| matches!(x, AgreeKind::SubCommand(_)))
				.cloned()
				.fold(
					AgreeSection::new("SUBCOMMANDS:", true),
					AgreeSection::with_item
				);
			if ! section.is_empty() {
				pre.push(section);
			}
		}

		pre.iter()
			.chain(self.other.iter())
			.for_each(|x| {
				out.push_char('\n');
				out.push_slice(&x.man());
			});

		out.push_char('\n');
		StrTendril::from(unsafe {
			std::str::from_utf8_unchecked(
				&out.as_bytes()
					.iter()
					.fold(Vec::new(), |mut v, &b| {
						if b == b'-' {
							v.extend_from_slice(br"\-");
						}
						else {
							v.push(b);
						}
						v
					})
			)
		})
	}
}



/// # Bash Helper (Long/Short Conds)
fn bash_long_short_conds(short: Option<&str>, long: Option<&str>) -> StrTendril {
	match (short, long) {
		(Some(s), Some(l)) => format_tendril!(
			r#"	if [[ ! " ${{COMP_LINE}} " =~ " {short} " ]] && [[ ! " ${{COMP_LINE}} " =~ " {long} " ]]; then
		opts+=("{short}")
		opts+=("{long}")
	fi
"#,
			short=s,
			long=l
		),
		(None, Some(k)) | (Some(k), None) => format_tendril!(
			"\t[[ \" ${{COMP_LINE}} \" =~ \" {key} \" ]] || opts+=(\"{key}\")\n",
			key=k
		),
		(None, None) => StrTendril::new(),
	}
}

/// # Man Tagline.
///
/// This helper method generates an appropriate key/value line given what sorts
/// of keys and values exist for the given [`AgreeKind`] type.
fn man_tagline(short: Option<&StrTendril>, long: Option<&StrTendril>, value: Option<&StrTendril>) -> StrTendril {
	match (short, long, value) {
		// Option: long and short.
		(Some(s), Some(l), Some(v)) => format_tendril!(
			".TP\n\\fB{}\\fR, \\fB{}\\fR {}",
			s, l, v
		),
		// Option: long or short.
		(Some(k), None, Some(v)) | (None, Some(k), Some(v)) => format_tendril!(
			".TP\n\\fB{}\\fR {}",
			k, v
		),
		// Switch: long and short.
		(Some(s), Some(l), None) => format_tendril!(
			".TP\n\\fB{}\\fR, \\fB{}\\fR",
			s, l
		),
		// Switch: long or short.
		// Key/Value.
		(Some(k), None, None) | (None, Some(k), None) | (None, None, Some(k)) => format_tendril!(
			".TP\n\\fB{}\\fR",
			k
		),
		_ => StrTendril::new(),
	}
}

#[allow(trivial_casts)] // Triviality is required.
/// # Write File.
///
/// This writes data to a file, optionally recursing to save a `GZipped`
/// version (for MAN pages).
fn write_to(file: &PathBuf, data: &[u8], compress: bool) -> Result<(), ()> {
	std::fs::File::create(file)
		.and_then(|mut out| out.write_all(data).and_then(|_| out.flush()))
		.map_err(|_| ())?;

	// Save a compressed copy?
	if compress {
		let mut writer = Compressor::new(CompressionLvl::best());
		let mut buf: Vec<u8> = Vec::with_capacity(data.len());
		buf.resize(writer.gzip_compress_bound(data.len()), 0);

		// Trim any excess now that we know the final length.
		let len = writer.gzip_compress(data, &mut buf).map_err(|_| ())?;
		buf.truncate(len);

		// Toss ".gz" onto the original file path.
		let filegz: PathBuf = PathBuf::from(OsStr::from_bytes(&[
			unsafe { &*(file.as_os_str() as *const OsStr as *const [u8]) },
			b".gz",
		].concat()));

		// Recurse to write it!
		return write_to(&filegz, &buf, false);
	}

	Ok(())
}
