/*!
# Cargo Bashman: Build
*/

use argyle::KeyWordsBuilder;
use std::{
	borrow::Cow,
	collections::BTreeSet,
	ffi::OsStr,
	fs::File,
	io::Error,
	path::PathBuf,
	process::{
		Command,
		Stdio,
	},
};



/// # Build!
pub fn main() {
	println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");

	build_cli();
	build_targets();
}

/// # Build CLI Arguments.
fn build_cli() {
	let mut builder = KeyWordsBuilder::default();
	builder.push_keys([
		"-h", "--help",
		"--no-bash",
		"--no-credits",
		"--no-man",
		"--print-targets",
		"-V", "--version",
	]);
	builder.push_keys_with_values([
		"-m", "--manifest-path",
		"-t", "--target",
	]);
	builder.save(out_path("argyle.rs"));
}

/// # Build Targets.
///
/// This method generates an enum and supporting code matching all of the
/// target triples supported by rustc.
///
/// It's a bit much, but this way we can detect and alert users to invalid
/// <TARGET> values passed via CLI without having to pass through cargo's
/// illegible error response.
///
/// This does, however, mean that for any given environment, the supported
/// targets will be the _intersection_ of ours and theirs. Not ideal, but an
/// acceptable tradeoff, I think.
fn build_targets() {
	use std::fmt::Write;

	let raw = Command::new({
		let out = std::env::var_os("RUSTC").unwrap_or_default();
		if out.is_empty() { Cow::Borrowed(OsStr::new("rustc")) }
		else { Cow::Owned(out) }
	})
		.args(["--print", "target-list"])
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.output()
		.and_then(|o|
			if o.status.success() {
				String::from_utf8(o.stdout).map_err(Error::other)
			}
			else {
				Err(Error::other(String::from_utf8_lossy(&o.stderr)))
			}
		);

	let raw = match raw {
		Ok(raw) => raw,
		Err(e) => panic!("Unable to obtain target triples from rustc: {e}"),
	};

	let all: BTreeSet<&str> = raw.lines()
		.filter_map(|line| {
			let line = line.trim();
			if line.is_empty() { None }
			else { Some(line) }
		})
		.collect();

	// Codegen time!
	let mut out = String::with_capacity(32_768); // Probably about right.

	// Define the enum.
	out.push_str("#[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// # Target Triples.
pub(crate) enum TargetTriple {
");

	for (k, v) in all.iter().enumerate() {
		writeln!(&mut out, "\t/// # {v}\n\tT{k:03},").unwrap();
	}

	// Close that off and add a TryFrom impl.
	out.push_str("}

impl TryFrom<String> for TargetTriple {
	type Error = BashManError;

	fn try_from(mut src: String) -> Result<Self, Self::Error> {
		src.make_ascii_lowercase();
		match src.trim() {
");

	for (k, v) in all.iter().enumerate() {
		writeln!(&mut out, "\t\t\t{:?} => Ok(Self::T{k:03}),", v.to_ascii_lowercase()).unwrap();
	}

	// Close it off and start a new impl to format as a string.
	out.push_str("\t\t\t_ => Err(BashManError::Target),
		}
	}
}

impl fmt::Display for TargetTriple {
	#[inline]
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		<str as fmt::Display>::fmt(self.as_str(), f)
	}
}

impl TargetTriple {
	/// # As String Slice.
	pub(crate) const fn as_str(self) -> &'static str {
		match self {
");

	for (k, v) in all.iter().enumerate() {
		writeln!(&mut out, "\t\t\tSelf::T{k:03} => {v:?},").unwrap();
	}

	// Close it off and start a code for an iterator.
	out.push_str("\t\t}
	}

	/// # Target Triple Iterator.
	const fn all() -> TargetTripleIter { TargetTripleIter(0) }
}



/// # Target Triple Iterator.
struct TargetTripleIter(u16);

impl Iterator for TargetTripleIter {
	type Item = TargetTriple;

	fn next(&mut self) -> Option<Self::Item> {
		let pos = self.0;
		self.0 += 1;
		match pos {
");

	// Note: transmute would be more economical here, but this crate forbids
	// unsafe_code. Hopefully the compiler will do that all on its own.
	let len = all.len();
	for k in 0..len {
		writeln!(&mut out, "\t\t\t{k} => Some(TargetTriple::T{k:03}),").unwrap();
	}

	// Close it off and add ExactSizeIterator bits.
	writeln!(
		&mut out,
		"\t\t\t_ => None,
		}}
	}}

	fn size_hint(&self) -> (usize, Option<usize>) {{
		let len = self.len();
		(len, Some(len))
	}}
}}

impl ExactSizeIterator for TargetTripleIter {{
	#[inline]
	fn len(&self) -> usize {{ usize::from({len}_u16.saturating_sub(self.0)) }}
}}"
	).unwrap();

	// Save it!
	File::create(out_path("target-triples.rs")).and_then(|mut f| {
		use std::io::Write;
		f.write_all(out.as_bytes()).and_then(|_| f.flush())
	})
	.expect("Unable to save target-triples.rs");
}

/// # Output Path.
///
/// Append the sub-path to OUT_DIR and return it.
fn out_path(stub: &str) -> PathBuf {
	std::fs::canonicalize(std::env::var("OUT_DIR").expect("Missing OUT_DIR."))
		.expect("Missing OUT_DIR.")
		.join(stub)
}
