/*!
# Cargo BashMan
*/

#![forbid(unsafe_code)]

#![deny(
	clippy::allow_attributes_without_reason,
	clippy::correctness,
	unreachable_pub,
)]

#![warn(
	clippy::complexity,
	clippy::nursery,
	clippy::pedantic,
	clippy::perf,
	clippy::style,

	clippy::allow_attributes,
	clippy::clone_on_ref_ptr,
	clippy::create_dir,
	clippy::filetype_is_file,
	clippy::format_push_string,
	clippy::get_unwrap,
	clippy::impl_trait_in_params,
	clippy::lossy_float_literal,
	clippy::missing_assert_message,
	clippy::missing_docs_in_private_items,
	clippy::needless_raw_strings,
	clippy::panic_in_result_fn,
	clippy::pub_without_shorthand,
	clippy::rest_pat_in_fully_bound_structs,
	clippy::semicolon_inside_block,
	clippy::str_to_string,
	clippy::string_to_string,
	clippy::todo,
	clippy::undocumented_unsafe_blocks,
	clippy::unneeded_field_pattern,
	clippy::unseparated_literal_suffix,
	clippy::unwrap_in_result,

	macro_use_extern_crate,
	missing_copy_implementations,
	missing_docs,
	non_ascii_idents,
	trivial_casts,
	trivial_numeric_casts,
	unused_crate_dependencies,
	unused_extern_crates,
	unused_import_braces,
)]

#![expect(clippy::doc_markdown, reason = "BashMan makes this annoying.")]
#![expect(clippy::redundant_pub_crate, reason = "Unresolvable.")]



mod bash;
mod credits;
mod err;
mod man;
mod parse;



use argyle::Argument;
use bash::BashWriter;
use credits::CreditsWriter;
use dactyl::NiceElapsed;
use err::BashManError;
use fyi_msg::Msg;
use man::ManWriter;
use oxford_join::{
	JoinFmt,
	OxfordJoinFmt,
};
use parse::{
	Flag,
	keyword::KeyWord,
	Manifest,
	OptionFlag,
	pkg::{
		Dependency,
		PackageName,
	},
	Subcommand,
	target::TargetTriple,
	TrailingArg,
};
use std::{
	borrow::Cow,
	fmt,
	path::{
		Path,
		PathBuf,
	},
	sync::LazyLock,
	time::Instant,
};



/// # Enable BASH completions.
const FLAG_BASH: u8 =    0b0001;

/// # Enable CREDITS.md.
const FLAG_CREDITS: u8 = 0b0010;

/// # Enable MAN page(s).
const FLAG_MAN: u8 =     0b0100;

/// # All Flags.
const FLAG_ALL: u8 =     0b0111;

/// # CWD.
static CWD: LazyLock<Option<PathBuf>> = LazyLock::new(||
	std::env::current_dir()
		.and_then(std::fs::canonicalize)
		.ok()
		.filter(|p| p.is_dir())
);



/// # Main.
fn main() {
	match _main() {
		Ok(()) => {},
		Err(BashManError::Target) => {
			Msg::error("Target must be one of the following:")
				.eprint();
			eprintln!("\x1b[2m-----\x1b[0m");
			println!("{}", BashManError::Target);
			std::process::exit(1);
		}
		Err(e @ (
			BashManError::PrintHelp |
			BashManError::PrintTargets |
			BashManError::PrintVersion
		)) => { println!("{e}"); },
		Err(e) => { Msg::error(e.to_string()).die(1); },
	}
}

#[inline]
/// # Actual main.
fn _main() -> Result<(), BashManError> {
	/// # Skipped Bash.
	const SKIPPED_BASH: u8 = 0b0001;

	/// # Skipped Man.
	const SKIPPED_MAN: u8 =  0b0010;

	// Keep track of the time.
	let now = Instant::now();

	// Parse CLI arguments.
	let args = argyle::args()
		.with_keywords(include!(concat!(env!("OUT_DIR"), "/argyle.rs")));

	let mut flags: u8 = FLAG_ALL;
	let mut manifest = None;
	let mut target = None;
	for arg in args {
		match arg {
			Argument::Key("--no-bash") => { flags &= ! FLAG_BASH; },
			Argument::Key("--no-credits") => { flags &= ! FLAG_CREDITS; },
			Argument::Key("--no-man") => { flags &= ! FLAG_MAN; },

			Argument::Key("-h" | "--help") => return Err(BashManError::PrintHelp),
			Argument::Key("--print-targets") => return Err(BashManError::PrintTargets),
			Argument::Key("-V" | "--version") => return Err(BashManError::PrintVersion),

			Argument::KeyWithValue("-m" | "--manifest-path", s) => {
				manifest.replace(PathBuf::from(s));
			},
			Argument::KeyWithValue("-t" | "--target", s) => {
				target.replace(TargetTriple::try_from(s)?);
			},

			// Nothing else is expected.
			Argument::Other(s) => if s.starts_with('-') {
				return Err(BashManError::InvalidCli(s))
			},
			Argument::InvalidUtf8(s) => return Err(BashManError::InvalidCli(s.to_string_lossy().into_owned())),
			_ => {},
		}
	}

	// Nothing to do?
	if 0 == flags & FLAG_ALL { return Err(BashManError::Noop); }

	// If no manifest path was provided, assume there's one in the current
	// working directory.
	let manifest = Manifest::from_file(match manifest {
		Some(m) => m,
		None => CWD.as_ref()
			.ok_or_else(|| BashManError::Dir("working", "./".to_owned()))?
			.join("Cargo.toml"),
	}, target)?;

	// Set up a shared buffer for whatever we'll be writing to help reduce
	// allocations.
	let mut buf = String::with_capacity(1024);

	let mut bad = Vec::with_capacity(3);
	let mut skipped = 0_u8;
	let mut good = Vec::with_capacity(3);
	let mut files = Vec::new();

	// Bash Completions.
	if FLAG_BASH == flags & FLAG_BASH {
		match BashWriter::try_from(&manifest).and_then(|w| w.write(&mut buf)) {
			Ok(p) => {
				good.push("bash completions");
				files.push(p);
			},
			Err(BashManError::Noop) => { skipped |= SKIPPED_BASH; },
			Err(e) => { bad.push(e); }
		}
	}

	// Man Pages.
	if FLAG_MAN == flags & FLAG_MAN {
		match ManWriter::try_from(&manifest).and_then(|w| w.write(&mut buf)) {
			Ok(mut p) => {
				good.push("man page(s)");
				files.append(&mut p);
			},
			Err(BashManError::Noop) => { skipped |= SKIPPED_MAN; },
			Err(e) => { bad.push(e); }
		}
	}

	// Crate Credits.
	if FLAG_CREDITS == flags & FLAG_CREDITS {
		match CreditsWriter::new(&manifest).and_then(|w| w.write(&mut buf)) {
			Ok(p) => {
				good.push("credits");
				files.push(p);
			},
			Err(e) => { bad.push(e); }
		}
	}

	// Print the good.
	if ! good.is_empty() {
		files.sort_unstable();
		Msg::success(format!(
			"Generated {} in {}.\n  \x1b[2m{}\x1b[0m",
			OxfordJoinFmt::and(good.as_slice()),
			NiceElapsed::from(now),
			JoinFmt::new(
				files.iter().map(|x| RelativePath::from(x.as_path())),
				"\n  ",
			),
		)).eprint();
	}

	// Print the skipped.
	if skipped != 0 {
		Msg::custom("Skipped", 11, &format!(
			"{}; no corresponding bashman manifest sections found.",
			match skipped {
				SKIPPED_BASH => "Bash completions",
				SKIPPED_MAN => "Man page(s)",
				_ => "Bash completions and man page(s)",
			}
		))
			.with_newline(true)
			.eprint();
	}

	#[expect(clippy::option_if_let_else, reason = "Too messy.")]
	// Print the bad.
	if let Some(last) = bad.pop() {
		for b in bad { Msg::error(b.to_string()).eprint(); }
		Err(last)
	}
	else { Ok(()) }
}



/// # Relative Path.
///
/// Try to reformat a path as relative to the current working directory so that
/// it can be printed more compactly.
struct RelativePath<'a>(Cow<'a, str>);

impl<'a> From<&'a Path> for RelativePath<'a> {
	#[inline]
	fn from(src: &'a Path) -> Self { Self(src.to_string_lossy()) }
}

impl<'a> fmt::Display for RelativePath<'a> {
	#[inline]
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		/// # Strip Prefix.
		///
		/// This method ensures the prefix has a trailing slash, since path
		/// parts won't always have one.
		fn strip_prefix<'a>(prefix: &str, full: &'a str) -> Option<&'a str> {
			let rest = full.strip_prefix(prefix)?;
			if prefix.ends_with('/') { Some(rest) }
			else { rest.strip_prefix('/') }
		}

		// If the CWD failed, print it as is.
		let Some(cwd) = CWD.as_ref().map(|p| p.to_string_lossy()) else {
			return f.write_str(&self.0);
		};

		// If the path is fully under the entire CWD, chop and print!
		if let Some(rest) = strip_prefix(&cwd, &self.0) {
			// But only if it is actually smaller this way.
			if rest.len() + 2 < self.0.len() {
				f.write_str("./")?;
				return f.write_str(rest);
			}

			// Otherwise it was fine as-was.
			return f.write_str(&self.0);
		}

		// Run through the parts until we stop matching.
		let mut split = cwd.split_inclusive('/');
		let mut rel: &str = self.0.as_ref();
		let mut dotdot = 0;
		for next in split.by_ref() {
			if let Some(rest) = strip_prefix(next, rel) { rel = rest; }
			else {
				dotdot = 1;
				break;
			}
		}

		// Count up the remaining parts to figure out how many dot-dots are
		// needed to get from here to there.
		dotdot += split.count();

		// If the relative version is smaller and not too deep, use it!
		if dotdot < 5 && rel.len() + usize::max(dotdot * 3, 2) < self.0.len() {
			if dotdot == 0 { f.write_str("./")?; }
			else {
				for _ in 0..dotdot { f.write_str("../")?; }
			}
			f.write_str(rel)
		}
		// Otherwise print it as was.
		else { f.write_str(&self.0) }
	}
}
