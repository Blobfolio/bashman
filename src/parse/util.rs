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
				if out.chars().any(|c| c.is_ascii_alphabetic()) {
					esc_markdown(&mut out);

					// Slash separators are deprecated.
					while let Some(pos) = out.find('/') { out.replace_range(pos..=pos, " OR "); }

					// Normalize and return if non-empty.
					normalize_string(&mut out);
					if out.is_empty() { None }
					else { Some(out) }
				}
				else { None }
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

/// # Escape Entities.
///
/// This method HTML-encodes entities with (possible) markdown properties,
/// namely for the benefit of the credits page.
fn esc_markdown(raw: &mut String) {
	// This kinda sucks. Haha.
	let mut end = raw.len();
	while let Some(pos) = raw[..end].rfind(['#', '*', '<', '>', '[', ']', '^', '_', '`', '|', '~']) {
		let entity = match raw.as_bytes()[pos] {
			b'#' => "&#35;",
			b'*' => "&#42;",
			b'<' => "&lt;",
			b'>' => "&gt;",
			b'[' => "&#91;",
			b']' => "&#93;",
			b'^' => "&#94;",
			b'_' => "&#95;",
			b'`' => "&#96;",
			b'|' => "&#124;",
			_ => "&#126;", // ~
		};
		raw.replace_range(pos..=pos, entity);
		end = pos;
	}
}

/// # Nice Author Line.
///
/// Sanitize an author line, which should either look like "Name" or
/// "Name <Email>". If the latter, this will reformat it as a markdown link
/// for the benefit of our credits generation.
fn nice_author(raw: &mut String) {
	/// # HTML Escape Email.
	///
	/// The email standard allows some wild shit that might need to be
	/// entity-encoded for HTML/Markdown.
	fn esc_email(local: &str, host: &str) -> String {
		let mut out = String::with_capacity(local.len() + 1 + host.len());

		// Only the local part needs this attention.
		for c in local.chars() {
			match c {
				'#' => { out.push_str("&#35;"); },
				'%' => { out.push_str("&#37;"); },
				'&' => { out.push_str("&#38;"); },
				'*' => { out.push_str("&#42;"); },
				'+' => { out.push_str("&#43;"); },
				'/' => { out.push_str("&#47;"); },
				'=' => { out.push_str("&#61;"); },
				'?' => { out.push_str("&#63;"); },
				'^' => { out.push_str("&#94;"); },
				'_' => { out.push_str("&#95;"); },
				'`' => { out.push_str("&#96;"); },
				'|' => { out.push_str("&#124;"); },
				'~' => { out.push_str("&#126;"); },

				// Regular characters!
				c => { out.push(c); },
			}
		}

		// Domains are much cleaner.
		out.push('@');
		out.push_str(host);

		out
	}

	raw.trim_mut();

	// Check for an email address.
	if let Some((start, end)) = raw.find('<').zip(raw.rfind('>')) {
		if start < end {
			// Pull out the email.
			raw.truncate(end);
			let email = Domain::email_parts(&raw[start + 1..])
				.map(|(local, host)| esc_email(&local, &host));
			raw.truncate(start);

			if let Some(email) = email {
				// Pretty up the name part.
				esc_markdown(raw);
				normalize_string(raw);

				// We have an email but not a name.
				if raw.is_empty() {
					raw.push('<');
					raw.push_str(&email);
					raw.push('>');
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
	if raw.chars().any(|c| c.is_ascii_alphabetic()) {
		esc_markdown(raw);
		normalize_string(raw);
	}
	else { raw.truncate(0); }
}



#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn t_esc_markdown() {
		let mut buf = String::new();
		for (raw, expected) in [
			("I`m #1!", "I&#96;m &#35;1!"),
			("### Headline", "&#35;&#35;&#35; Headline"),
			("((<_>))", "((&lt;&#95;&gt;))"),
			("#[On]* | *[Off]#", "&#35;&#91;On&#93;&#42; &#124; &#42;&#91;Off&#93;&#35;"),
			("Up^", "Up&#94;"),
			("Crook`d~~", "Crook&#96;d&#126;&#126;"),
			("hello world", "hello world"),
		] {
			raw.clone_into(&mut buf);
			esc_markdown(&mut buf);
			assert_eq!(buf, expected);
		}
	}

	#[test]
	fn t_nice_author() {
		let mut author = String::new();
		for (raw, expected) in [
			(" <", ""),
			("Josh  <USER@♥.com>", "[Josh](mailto:user@xn--g6h.com)"),
			("<USER@♥.com>", "<user@xn--g6h.com>"),
			("The\tConsortium", "The Consortium"),
			("Björk <localhost>", "Björk"),
		] {
			raw.clone_into(&mut author);
			nice_author(&mut author);
			assert_eq!(author, expected);
		}
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
