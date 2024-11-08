/*!
# Cargo BashMan: Parsing Helpers.
*/

use adbyss_psl::Domain;
use crate::{
	BashManError,
	TargetTriple,
};
use semver::Version;
use serde::{
	Deserialize,
	Deserializer,
};
use std::{
	borrow::Cow,
	collections::HashSet,
	ffi::OsStr,
	path::Path,
	process::{
		Command,
		Output,
		Stdio,
	},
	sync::OnceLock,
};
use trimothy::{
	NormalizeWhitespace,
	TrimMut,
};



#[derive(Debug, Clone, Copy)]
/// # Cargo Metadata.
///
/// This struct is used to configure and execute a call to `cargo metadata`.
pub(super) struct CargoMetadata<'a> {
	/// # Manifest Path.
	path: &'a Path,

	/// # Target Triple.
	target: Option<TargetTriple>,

	/// # Flags.
	features: bool,
}

impl<'a> CargoMetadata<'a> {
	/// # New.
	pub(super) const fn new(path: &'a Path, target: Option<TargetTriple>) -> Self {
		Self {
			path,
			target,
			features: false,
		}
	}

	/// # With Features.
	///
	/// If `false`, will be called with `--no-default-features`; if `true`,
	/// `--all-features`.
	pub(super) const fn with_features(self, features: bool) -> Self {
		Self { features, ..self }
	}

	/// # Exec.
	pub(super) fn exec(&self) -> Result<Vec<u8>, BashManError> {
		// Populate the command arguments.
		let mut cmd = cargo_cmd();
		cmd.args([
			"metadata",
			"--quiet",
			"--color", "never",
			"--format-version", "1",
			if self.features { "--all-features" } else { "--no-default-features" },
			"--manifest-path",
		]);
		cmd.arg(self.path.as_os_str());
		if let Some(target) = self.target {
			cmd.args(["--filter-platform", target.as_str()]);
		}

		// Run it and see what happens!
		let Output { status, stdout, .. } = cmd
			.stdin(Stdio::null())
			.stdout(Stdio::piped())
			.stderr(Stdio::null())
			.output()
			.map_err(|_| BashManError::Cargo)?;

		if status.success() && stdout.starts_with(br#"{"packages":["#) { Ok(stdout) }
		else { Err(BashManError::Cargo) }
	}

	/// # Exec Tree.
	///
	/// Cargo tree is better at finding the dependencies we care about than we
	/// are. If we can use it to get a list, we might as well!
	pub(super) fn exec_tree<'b>(&self, packages: &'b [super::cargo::RawPackage]) -> Option<HashSet<&'b str>> {
		if packages.is_empty() { return None; }

		// Populate the command arguments.
		let mut cmd = cargo_cmd();
		cmd.args([
			"tree",
			"--quiet",
			"--color", "never",
			"--edges", "normal,build",
			"--prefix", "none",
			if self.features { "--all-features" } else { "--no-default-features" },
			"--target", self.target.map_or("all", TargetTriple::as_str),
			"--manifest-path",
		]);
		cmd.arg(self.path.as_os_str());

		let raw = cmd
			.stdin(Stdio::null())
			.stdout(Stdio::piped())
			.stderr(Stdio::null())
			.output()
			.ok()
			.and_then(|o|
				if o.status.success() { String::from_utf8(o.stdout).ok() }
				else { None }
			)?;

		// Find the package/version pairs.
		let name_version: HashSet<(&str, Version)> = raw.lines()
			.filter_map(|line| {
				let mut parts = line.split_whitespace();
				let name = parts.next()?;
				let version = parts.next()
					.and_then(|s| s.strip_prefix("v"))
					.and_then(|s| s.parse::<Version>().ok())?;
				Some((name, version))
			})
			.collect();

		// Now map those to package IDs from cargo metadata!
		let mut out = HashSet::with_capacity(packages.len());
		for p in packages {
			if name_version.contains(&(p.name.as_str(), p.version.clone())) {
				out.insert(p.id);
			}
		}

		// Return it if we got it!
		if out.is_empty() { None }
		else { Some(out) }
	}
}



#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: Authors.
pub(super) fn deserialize_authors<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where D: Deserializer<'de> {
	if let Ok(mut out) = <Vec<String>>::deserialize(deserializer) {
		out.retain_mut(|line| {
			nice_author(line);
			! line.is_empty()
		});
		return Ok(out);
	}

	Ok(Vec::new())
}

#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: Package License.
///
/// Note this removes problematic characters but does not strictly enforce SPDX
/// formatting requirements or license names.
pub(super) fn deserialize_license<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where D: Deserializer<'de> {
	Ok(
		<String>::deserialize(deserializer).ok()
			.and_then(|mut out| {
				out.retain(|c| ! matches!(c, '[' | ']' | '<' | '>' | '|'));

				// Slash separators are deprecated.
				while let Some(pos) = out.find('/') { out.replace_range(pos..=pos, " OR "); }

				// Normalize and return if non-empty.
				normalize_string(&mut out);
				if out.is_empty() { None }
				else { Some(out) }
			})
	)
}

/// # Deserialize: Non-Empty String, Normalized.
///
/// This will return an error if a string is present but empty.
pub(super) fn deserialize_nonempty_str_normalized<'de, D>(deserializer: D) -> Result<String, D::Error>
where D: Deserializer<'de> {
	let mut out = <String>::deserialize(deserializer)?;
	normalize_string(&mut out);
	if out.is_empty() { Err(serde::de::Error::custom("value cannot be empty")) }
	else { Ok(out) }
}

#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: Optional Non-Empty String.
///
/// This will return `None` if the string is empty.
pub(super) fn deserialize_nonempty_opt_str<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where D: Deserializer<'de> {
	Ok(
		<String>::deserialize(deserializer).ok()
			.and_then(|mut x| {
				x.trim_mut();
				if x.is_empty() { None }
				else { Some(x) }
			})
	)
}

#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: Optional Non-Empty String, Normalized.
///
/// This will return `None` if the string is empty, normalizing whitespace and
/// control characters along the way.
pub(super) fn deserialize_nonempty_opt_str_normalized<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where D: Deserializer<'de> {
	Ok(
		<String>::deserialize(deserializer).ok()
			.and_then(|mut x| {
				normalize_string(&mut x);
				if x.is_empty() { None }
				else { Some(x) }
			})
	)
}

/// # Normalize String.
///
/// Compact whitespace and strip control characters.
///
/// This proceeds under the assumption that most normalization can be achieved
/// "inline" via `retain`, but if substitution is required it will rebuild the
/// string char-by-char.
pub(super) fn normalize_string(raw: &mut String) {
	let mut ws = true;
	let mut rebuild = false;
	raw.retain(|c: char|
		if c.is_whitespace() {
			if ws { false }
			else {
				ws = true;
				if c != ' ' { rebuild = true; }
				true
			}
		}
		else if c.is_control() { false }
		else {
			ws = false;
			true
		}
	);

	// We encountered something requiring more than a strip; rebuild!
	if rebuild { *raw = raw.normalized_whitespace().collect(); }
	// Just trim the end and we're good to go!
	else { raw.trim_end_mut(); }
}



/// # Return Cargo Command.
///
/// This instantiates a new (argumentless) command set to the `$CARGO`
/// environmental variable or simply "cargo".
fn cargo_cmd() -> Command {
	/// # Cargo Executable Path.
	static CARGO: OnceLock<Cow<OsStr>> = OnceLock::new();

	// Start the command.
	Command::new(CARGO.get_or_init(|| {
		let out = std::env::var_os("CARGO").unwrap_or_default();
		if out.is_empty() { Cow::Borrowed(OsStr::new("cargo")) }
		else { Cow::Owned(out) }
	}))
}

/// # Nice Author Line.
///
/// Sanitize an author line, which should either look like "Name" or
/// "Name <Email>". If the latter, this will reformat it as a markdown link
/// for the benefit of our credits generation.
fn nice_author(raw: &mut String) {
	// Check for an email address.
	if let Some((start, end)) = raw.find('<').zip(raw.rfind('>')) {
		if start < end {
			// Chop off the email bit.
			raw.truncate(end);
			let email = raw.split_off(start + 1);
			raw.truncate(start);

			if let Some(email) = nice_email(email) {
				// Pretty up the name part.
				raw.retain(|c| ! matches!(c, '[' | ']' | '<' | '>' | '(' | ')' | '|'));
				normalize_string(raw);

				// We have an email but not a name.
				if raw.is_empty() {
					raw.push('[');
					raw.push_str(&email);
					raw.push_str("](mailto:");
					raw.push_str(&email);
					raw.push(')');
					return;
				}

				// Add the email back.
				raw.insert(0, '[');
				raw.push_str("](mailto:");
				raw.push_str(&email);
				raw.push(')');
				return;
			}
		}
	}

	// It stands alone.
	raw.retain(|c| ! matches!(c, '[' | ']' | '<' | '>' | '(' | ')' | '|'));
	normalize_string(raw);
}

/// # Validate email.
///
/// It's unclear if the Cargo author metadata is pre-sanitized. Just in case,
/// this method performs semi-informed validation against suspected email
/// addresses, making sure the user portion is lowercase alphanumeric (with `.`,
/// `+`, `-`, and `_` allowed), and the host is ASCII with a valid public
/// suffix. (The host domain itself may or may not exist, but that's fine.)
///
/// If any of the above conditions fail, `None` is returned, otherwise a fresh
/// owned `String` is returned.
fn nice_email(mut raw: String) -> Option<String> {
	// We need an at sign!
	raw.trim_mut();
	let at = raw.find('@')?;
	if raw.len() <= at + 1 { return None; }

	// We also need a user portion consisting of only ASCII alphanumeric or the
	// limited special characters we support.
	raw.make_ascii_lowercase();
	let user = raw[..at].as_bytes();
	if user.is_empty() || ! user.iter().copied().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'+' | b'-' | b'_')) {
		return None;
	}

	// Split off and validate/clean the host.
	let host = Domain::try_from(raw.split_off(at + 1)).ok()?;

	// Add it back and return!
	raw.push_str(host.as_str());
	Some(raw)
}



#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn t_nice_author() {
		let mut author = String::new();
		for (raw, expected) in [
			(" <", ""),
			("Josh  <USER@♥.com>", "[Josh](mailto:user@xn--g6h.com)"),
			("<USER@♥.com>", "[user@xn--g6h.com](mailto:user@xn--g6h.com)"),
			("The\tConsortium", "The Consortium"),
			("Björk <localhost>", "Björk"),
		] {
			raw.clone_into(&mut author);
			nice_author(&mut author);
			assert_eq!(author, expected);
		}
	}

	#[test]
	fn t_nice_email() {
		assert_eq!(
			nice_email("  JoSh@BloBfolio.com ".to_owned()),
			Some("josh@blobfolio.com".to_owned())
		);

		assert_eq!(nice_email("  JoSh@BloBfolio.x ".to_owned()), None);

		assert_eq!(
			nice_email("USER@♥.com".to_owned()),
			Some("user@xn--g6h.com".to_owned())
		);
	}

	#[test]
	fn t_normalize_string() {
		let mut buf = String::new();

		for (raw, expected) in [
			("Björk", "Björk"),
			(" Björk\t\n", "Björk"),
			("hello\tB\0j\x1börk", "hello Björk"),
			(" \0 ", ""),
		] {
			raw.clone_into(&mut buf);
			normalize_string(&mut buf);
			assert_eq!(buf, expected);
		}
	}
}
