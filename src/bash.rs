/*!
# Cargo BashMan: Bash Completions.
*/

use crate::{
	BashManError,
	Flag,
	Manifest,
	OptionFlag,
};
use oxford_join::JoinFmt;
use std::{
	cmp::Ordering,
	fmt,
	path::PathBuf,
};



/// # Bash Completions.
///
/// This struct is used to write bash completions for the (sub)commands and/or
/// keyed arguments in a `Manifest`.
///
/// The magic is largely handled through the `Display` impls of this and
/// supporting sub-structures, but `BashWriter::write` is what actually makes
/// the call and saves the file.
pub(super) struct BashWriter<'a> {
	/// # Output Directory.
	dir: PathBuf,

	/// # Subcommands.
	subcommands: Vec<Subcommand<'a>>,
}

impl<'a> fmt::Display for BashWriter<'a> {
	/// # Write Completions!
	///
	/// This method outputs the _entire_ contents of the completions file. It
	/// is used by the `BashWriter::write`, though that method removes
	/// redundant line breaks from the result before saving it to disk.
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		// This should never fail, but if it does we have nothing to do.
		let Ok(main) = self.main_cmd() else { return Ok(()); };

		// We can save ourselves a lot of trouble if there is only a single
		// command to worry about!
		if self.subcommands.len() == 1 {
			<Subcommand as fmt::Display>::fmt(main, f)?;
			return writeln!(
				f,
				"complete -F {} -o bashdefault -o default {}",
				main.fname,
				main.bin,
			);
		}

		// Otherwise we need to start by writing the key methods for each of
		// the subcommands (ignoring the main one for the moment).
		for sub in &self.subcommands {
			if ! sub.main {
				<Subcommand as fmt::Display>::fmt(sub, f)?;
			}
		}

		// Now we need to do the same thing for the main command, passing it a
		// list of the subcommands since those are "keywords" in that top-level
		// context. (The generated method is otherwise identical to what the
		// subs got earlier.)
		main.write_completions(
			f,
			self.subcommands.iter().filter_map(|s|
				if s.main { None }
				else { Some(s.bin) }
			)
		)?;

		// To finish, we need to add two more methods to route the matching to
		// the right sub/command method (that we already generated).
		let fname = main.fname.as_str();
		let bname = main.bin;
		writeln!(
			f,
			r#"subcmd_{fname}() {{
	local i cmd
	COMPREPLY=()
	cmd=""

	for i in ${{COMP_WORDS[@]}}; do
		case "${{i}}" in
{}
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
{}
		*)
			;;
	esac
}}

complete -F chooser_{fname} -o bashdefault -o default {bname}"#,
			JoinFmt::new(self.subcommands.iter().map(SubcmdCase::from), ""),
			JoinFmt::new(self.subcommands.iter().map(ChooserCase::from), ""),
		)
	}
}

impl<'a> TryFrom<&'a Manifest> for BashWriter<'a> {
	type Error = BashManError;

	fn try_from(src: &'a Manifest) -> Result<Self, Self::Error> {
		let dir = src.dir_bash()?;
		let raw_subcommands = src.subcommands();
		let mut subcommands: Vec<_> = raw_subcommands.iter()
			.map(Subcommand::from)
			.collect();
		subcommands.sort_unstable();
		subcommands.dedup();

		// Assuming we didn't lose anything, we're good!
		if raw_subcommands.len() == subcommands.len() {
			Ok(Self { dir, subcommands })
		}
		else { Err(BashManError::Bash) }
	}
}

impl<'a> BashWriter<'a> {
	/// # Main Command.
	///
	/// We store the primary and subcommands together because they mostly work
	/// exactly the same, but not _always_.
	///
	/// This method finds and returns just the main entry for the times where
	/// that distinction matters.
	///
	/// If for some unlikely reason there isn't one, an error will be returned.
	fn main_cmd(&self) -> Result<&Subcommand<'_>, BashManError> {
		self.subcommands.iter()
			.find(|s| s.main)
			.ok_or(BashManError::Bash)
	}

	/// # Write to File.
	///
	/// This method is called by `main.rs` to generate and save the bash
	/// completions.
	///
	/// The shared `buf` is used to help reduce allocations across the various
	/// writes the program will make.
	///
	/// Errors will be bubbled up if encountered, otherwise the output path
	/// is returned.
	pub(super) fn write(self, buf: &mut String) -> Result<PathBuf, BashManError> {
		use std::fmt::Write;

		// We have an output directory but not a file name. Let's generate this
		// now because if we can't for whatever reason, there's no sense
		// continuing with the codegen.
		let mut bname = self.main_cmd()?.bin.to_owned();
		bname.push_str(".bash");

		// Reset the buffer and write our completions into it.
		buf.truncate(0);
		write!(buf, "{self}").map_err(|_| BashManError::Bash)?;

		// Strip double linebreaks before saving to a file. (Waste not, want
		// not!)
		let mut last = '\n';
		buf.retain(|c|
			if c == '\n' {
				if last == '\n' { false }
				else {
					last = '\n';
					true
				}
			}
			else {
				last = c;
				true
			}
		);

		// Save it!
		let out_file = self.dir.join(bname);
		write_atomic::write_file(&out_file, buf.as_bytes())
			.map_err(|_| BashManError::Write(out_file.to_string_lossy().into_owned()))
			.map(|()| out_file)
	}
}



#[derive(Debug, Clone, Copy)]
/// # chooser_XXX Case.
///
/// This is used to help format the case entries in the `chooser_XXX` bash
/// method, enabling us to leverage a `JoinFmt` to keep the damage confined to
/// a single `write!` pattern.
struct ChooserCase<'a>(&'a str, &'a str);

impl<'a> fmt::Display for ChooserCase<'a> {
	/// # Write the Case.
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		writeln!(f, "\
			\t\t{})\n\
			\t\t\t{}\n\
			\t\t\t;;",
			self.0,
			self.1,
		)
	}
}

impl<'a> From<&'a Subcommand<'a>> for ChooserCase<'a> {
	#[inline]
	fn from(src: &'a Subcommand<'a>) -> Self {
		Self(src.bin, src.fname.as_str())
	}
}



#[derive(Debug, Clone, Copy)]
/// # subcmd_XXX Case.
///
/// This is used to help format the case entries in the subcmd_XXX bash method,
/// enabling us to leverage a `JoinFmt` to keep the damage confined to a single
/// `write!` pattern.
struct SubcmdCase<'a>(&'a str);

impl<'a> fmt::Display for SubcmdCase<'a> {
	/// # Write Case.
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		writeln!(f, "\
			\t\t\t{})\n\
			\t\t\t\tcmd=\"{}\"\n\
			\t\t\t\t;;",
			self.0,
			self.0,
		)
	}
}

impl<'a> From<&'a Subcommand<'a>> for SubcmdCase<'a> {
	#[inline]
	fn from(src: &'a Subcommand<'a>) -> Self { Self(src.bin) }
}



#[derive(Debug, Clone)]
/// # Key Kind.
///
/// Only `Flag` and `OptionFlag` data components are relevant for bash
/// completions, and both work pretty much exactly the same. This enum lets us
/// group them neatly together.
struct Key<'a> {
	/// # Short Key.
	short: Option<&'a str>,

	/// # Long Key.
	long: Option<&'a str>,

	/// # Key Settings.
	flags: u8,
}

impl<'a> fmt::Display for Key<'a> {
	/// # Write Conditions.
	///
	/// This generates code to add the key(s) to the completion matcher for a
	/// given (sub)command chooser method.
	///
	/// This is called by other `Display` impls higher up the chain; it is not
	/// useful on its own.
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let duplicate = Self::FLAG_DUPLICATE == self.flags & Self::FLAG_DUPLICATE;
		match (self.short, self.long) {
			// Two keys.
			(Some(s), Some(l)) =>
				if duplicate {
					writeln!(f, "\topts+=(\"{s}\")\n\topts+=(\"{l}\")")
				}
				else {
					writeln!(
						f,
					r#"	if [[ ! " ${{COMP_LINE}} " =~ " {s} " ]] && [[ ! " ${{COMP_LINE}} " =~ " {l} " ]]; then
		opts+=("{s}")
		opts+=("{l}")
	fi"#,
					)
				},
			// One key.
			(Some(k), None) | (None, Some(k)) =>
				if duplicate { writeln!(f, "\topts+=(\"{k}\")") }
				else {
					writeln!(
						f,
						"\t[[ \" ${{COMP_LINE}} \" =~ \" {k} \" ]] || opts+=(\"{k}\")",
					)
				},
			// There should never be nothing, but whatever.
			(None, None) => Ok(()),
		}
	}
}

impl<'a> From<&'a Flag> for Key<'a> {
	#[inline]
	fn from(src: &'a Flag) -> Self {
		Self {
			short: src.short(),
			long: src.long(),
			flags: if src.duplicate() { Self::FLAG_DUPLICATE } else { 0 },
		}
	}
}

impl<'a> From<&'a OptionFlag> for Key<'a> {
	#[inline]
	fn from(src: &'a OptionFlag) -> Self {
		let mut flags = Self::FLAG_OPTION;
		if src.duplicate() { flags |= Self::FLAG_DUPLICATE; }
		if src.path() { flags |= Self::FLAG_PATH; }

		Self {
			short: src.short(),
			long: src.long(),
			flags,
		}
	}
}

impl<'a> Key<'a> {
	/// # Flag: Allow Duplicates?
	const FLAG_DUPLICATE: u8 = 0b0001;

	/// # Flag: Takes Value?
	const FLAG_OPTION: u8 =    0b0010;

	/// # Flag: Takes Path Value?
	const FLAG_PATH: u8 =      0b0110;
}


#[derive(Debug, Clone)]
/// # (Sub)command.
///
/// A Bash-specific wrapper around the few subcommand/data components we
/// care about for completion purposes.
///
/// Note the `fname` field is used for equality/sorting purposes.
///
/// Concision aside, this separation from the crate-level `Subcommand`
/// structure allows us to give it a bash-specific `Display` impl, simplifying
/// the task of generating the completion code.
struct Subcommand<'a> {
	/// # Primary Command?
	main: bool,

	/// # Command.
	bin: &'a str,

	/// # Data.
	data: Vec<Key<'a>>,

	/// # Bash Function Name.
	fname: String,
}

impl<'a> fmt::Display for Subcommand<'a> {
	#[inline]
	/// # Write Completion Method.
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.write_completions(f, [])
	}
}

impl<'a> From<&'a crate::Subcommand> for Subcommand<'a> {
	fn from(src: &'a crate::Subcommand) -> Self {
		let parent_bin = src.parent_bin();
		let bin = src.bin();

		// Tease out the key data (args and sections are irrelevant).
		let raw_data = src.data();
		let data: Vec<Key> = raw_data.flags().iter().map(Key::from)
			.chain(raw_data.options().iter().map(Key::from))
			.collect();

		// Generate a function name to hold the keyword lookups.
		let mut fname = String::with_capacity(10 + parent_bin.map_or(0, str::len) + bin.len());
		fname.push_str("_basher__");
		if let Some(p) = parent_bin {
			// Lowercase ASCII alphanumeric is fine; underscores for
			// substitution.
			fname.extend(p.chars().map(|c| match c {
				'a'..='z' | '0'..='9' => c,
				'A'..='Z' => c.to_ascii_lowercase(),
				_ => '_',
			}));
		}
		fname.push('_');
		fname.extend(bin.chars().map(|c| match c {
			'a'..='z' | '0'..='9' => c,
			'A'..='Z' => c.to_ascii_lowercase(),
			_ => '_',
		}));

		Self {
			main: parent_bin.is_none(),
			bin,
			data,
			fname
		}
	}
}

impl<'a> Eq for Subcommand<'a> {}

impl<'a> Ord for Subcommand<'a> {
	#[inline]
	fn cmp(&self, other: &Self) -> Ordering { self.fname.cmp(&other.fname) }
}

impl<'a> PartialEq for Subcommand<'a> {
	#[inline]
	fn eq(&self, other: &Self) -> bool { self.fname == other.fname }
}

impl<'a> PartialOrd for Subcommand<'a> {
	#[inline]
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl<'a> Subcommand<'a> {
	/// # Write Completion Method.
	///
	/// This method writes a command-specific completion method containing the
	/// relevant key(s) and/or subcommands.
	///
	/// This uses `Display` semantics because most of the time that's how it is
	/// used, but in multi-command contexts, `BashWriter` will call this
	/// directly on the main command so it can pass along the other subcommands
	/// for inclusion.
	fn write_completions<I: IntoIterator<Item=&'a str>>(
		&self,
		f: &mut fmt::Formatter<'_>,
		subcommands: I,
	) -> fmt::Result {
		// Write the function opener.
		f.write_str(&self.fname)?;
		f.write_str(r#"() {
	local cur prev opts
	COMPREPLY=()
	cur="${COMP_WORDS[COMP_CWORD]}"
	prev="${COMP_WORDS[COMP_CWORD-1]}"
	opts=()
"#)?;
		// Add the key conditionals.
		for key in &self.data { <Key as fmt::Display>::fmt(key, f)?; }

		// Add subcommands?
		if self.main {
			for sub in subcommands {
				writeln!(f, "\topts+=(\"{sub}\")")?;
			}
		}

		// Add some formatting/abort handling.
		f.write_str(r#"	opts=" ${opts[@]} "
	if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
		COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
		return 0
	fi
"#)?;

		// Add special matching for path-options, if any.
		let path_keys = self.path_keys();
		if ! path_keys.is_empty() {
			writeln!(
				f,
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
	esac"#,
				JoinFmt::new(path_keys.iter(), "|"),
			)?;
		}

		// Close off the method!
		f.write_str(r#"	COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
	return 0
}
"#)
	}

	/// # Keys Requiring Path Values.
	///
	/// Return a set of all of the option keys that expect path values, if any.
	fn path_keys(&self) -> Vec<&str> {
		let mut out = Vec::new();
		for key in &self.data {
			if Key::FLAG_PATH == key.flags & Key::FLAG_PATH {
				if let Some(k) = key.short { out.push(k); }
				if let Some(k) = key.long { out.push(k); }
			}
		}

		// Sort and dedup before returning.
		if 1 < out.len() {
			out.sort_unstable();
			out.dedup();
		}

		out
	}
}
