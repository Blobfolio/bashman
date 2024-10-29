/*!
# Cargo BashMan: Raw Cargo Metadata JSON Parsing.
*/

use adbyss_psl::Domain;
use crate::{
	BashManError,
	Dependency,
	PackageName,
};
use semver::Version;
use serde::{
	Deserialize,
	Deserializer,
};
use std::{
	borrow::Cow,
	collections::{
		BTreeSet,
		hash_map::Entry,
		HashMap,
		HashSet,
	},
	ffi::OsStr,
	path::Path,
	process::{
		Command,
		Output,
		Stdio,
	},
	sync::OnceLock,
};
use super::util;
use trimothy::TrimMut;
use url::Url;



#[inline]
/// # Fetch Dependencies.
///
/// Run `cargo metadata` and parse the results into a sorted and deduped list
/// of dependencies.
pub(super) fn fetch_dependencies<P: AsRef<Path>>(src: P, features: bool)
-> Result<BTreeSet<Dependency>, BashManError> {
	let raw = cargo_exec(src, features)?;
	from_json(&raw)
}



#[derive(Debug, Deserialize)]
/// # Raw Top-Level Structure.
struct Raw<'a> {
	#[serde(borrow)]
	/// # Detailed Packages.
	packages: Vec<RawPackage<'a>>,

	#[serde(with = "RawNodes")]
	#[serde(borrow)]
	/// # Resolved Tree.
	resolve: Vec<RawNode<'a>>,
}



#[derive(Deserialize)]
/// # Raw Nodes (Wrapper).
///
/// We only care about one of the components in `resolve`; this removes a level
/// of complexity.
struct RawNodes<T> {
	/// # Nodes.
	nodes: T,
}

impl<T> RawNodes<T> {
	/// # Deserialize.
	fn deserialize<'de, D>(deserializer: D) -> Result<T, D::Error>
	where T: Deserialize<'de>, D: Deserializer<'de> {
		let wrapper = <Self as Deserialize>::deserialize(deserializer)?;
		Ok(wrapper.nodes)
	}
}



#[derive(Debug, Deserialize)]
/// # Raw Node.
///
/// Nodes are like Package-Lite, presumably to cut down on the JSON size.
struct RawNode<'a> {
	#[serde(borrow)]
	/// # ID.
	id: &'a str,

	#[serde(default)]
	#[serde(borrow)]
	/// # Dependency Details.
	deps: Vec<RawNodeDep<'a>>,
}



#[derive(Debug, Clone, Copy, Deserialize)]
/// # Raw Node Dependency.
///
/// The node dependencies are similarly lightweight, containing only an ID
/// and the unique context combinations.
struct RawNodeDep<'a> {
	#[serde(rename = "pkg")]
	#[serde(borrow)]
	/// # ID.
	id: &'a str,

	#[serde(default = "default_depkinds")]
	#[serde(deserialize_with = "deserialize_depkinds")]
	/// # Dependency Kinds.
	dep_kinds: u8,
}



#[derive(Debug, Clone, Copy, Deserialize)]
/// # Raw Node Dependency Kind.
///
/// This holds the different combinations of kind/target for a given
/// dependency's dependency.
struct RawNodeDepKind {
	#[serde(default)]
	#[serde(rename = "kind")]
	#[serde(deserialize_with = "deserialize_kind")]
	/// # Development?
	dev: bool,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_target")]
	/// # Target.
	target: bool,
}

impl RawNodeDepKind {
	/// # Into Flag.
	///
	/// Convert the kind/target into the corresponding context flag used by
	/// our `Dependency` struct.
	const fn as_flag(self) -> u8 {
		if self.dev { Dependency::FLAG_DEV }
		else if self.target { Dependency::FLAG_RUNTIME | Dependency::FLAG_TARGET }
		else { Dependency::FLAG_RUNTIME | Dependency::FLAG_ANY }
	}
}



#[derive(Debug, Deserialize)]
/// # Raw Package.
///
/// This is almost but not quite the same as our `Dependency` struct.
struct RawPackage<'a> {
	/// # Name.
	name: PackageName,

	/// # Version.
	version: Version,

	#[serde(borrow)]
	/// # ID.
	id: &'a str,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_license")]
	/// # License.
	license: String,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_authors")]
	/// # Author(s).
	authors: Vec<String>,

	#[serde(default)]
	/// # Repository URL.
	repository: Option<Url>,
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

/// # Execute Cargo Metadata.
///
/// Run `cargo metadata` and return the results (raw JSON) or an error.
fn cargo_exec<P: AsRef<Path>>(src: P, features: bool)
-> Result<Vec<u8>, BashManError> {
	let src: &Path = src.as_ref();
	let Output { stdout, .. } = cargo_cmd()
		.args([
			"metadata",
			"--quiet",
			"--color", "never",
			"--format-version", "1",
			if features { "--all-features" } else { "--no-default-features" },
			"--manifest-path",
		])
		.arg(src)
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::null())
		.output()
		.map_err(|_| BashManError::Cargo)?;

	if stdout.starts_with(br#"{"packages":["#) { Ok(stdout) }
	else { Err(BashManError::Cargo) }
}

/// # Default Dependency Kinds.
const fn default_depkinds() -> u8 { Dependency::FLAG_RUNTIME }

#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: Authors.
fn deserialize_authors<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
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
/// # Deserialize: Dependency Kinds.
fn deserialize_depkinds<'de, D>(deserializer: D) -> Result<u8, D::Error>
where D: Deserializer<'de> {
	if let Ok(out) = <Vec<RawNodeDepKind>>::deserialize(deserializer) {
		let out = out.into_iter().fold(0_u8, |acc, v| acc | v.as_flag());
		if out != 0 { return Ok(out); }
	}

	Ok(Dependency::FLAG_RUNTIME)
}

#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: Dev Kind?
fn deserialize_kind<'de, D>(deserializer: D) -> Result<bool, D::Error>
where D: Deserializer<'de> {
	Ok(
		matches!(<Cow<str>>::deserialize(deserializer).ok().as_deref(), Some("dev"))
	)
}

#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: License.
fn deserialize_license<'de, D>(deserializer: D) -> Result<String, D::Error>
where D: Deserializer<'de> {
	if let Ok(mut out) = <String>::deserialize(deserializer) {
		out.retain(|c| ! matches!(c, '[' | ']' | '<' | '>' | '(' | ')' | '|'));

		// Replace joiners with simple semi-colons.
		while let Some(pos) = out.find(" AND ") { out.replace_range(pos..pos + 5, "; "); }
		while let Some(pos) = out.find(" OR ") { out.replace_range(pos..pos + 4, "; "); }
		while let Some(pos) = out.find('/') { out.replace_range(pos..=pos, "; "); }

		util::normalize_string(&mut out);
		return Ok(out);
	}

	Ok(String::new())
}

#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: Target.
fn deserialize_target<'de, D>(deserializer: D) -> Result<bool, D::Error>
where D: Deserializer<'de> {
	Ok(
		<Cow<str>>::deserialize(deserializer).ok()
		.map_or(
			false,
			|o| ! o.trim().is_empty()
		)
	)
}

/// # Parse Dependencies.
///
/// Parse the raw JSON output from a `cargo metadata` command and return
/// the relevant dependencies, deduped and sorted.
///
/// This is called by `Manifest::dependencies` twice, with and without
/// features enabled to classify required/optional dependencies.
fn from_json(raw: &[u8]) -> Result<BTreeSet<Dependency>, BashManError> {
	let Raw { packages, resolve } = serde_json::from_slice(raw)
		.map_err(|e| BashManError::ParseCargoMetadata(e.to_string()))?;

	// First let's figure out the contexts for each sub-dependency (build,
	// target-specific, etc.). This requires looping loops of loops. Haha.
	let mut flags = HashMap::<&str, u8>::with_capacity(packages.len());
	for deps in resolve.iter().flat_map(|r| r.deps.iter()) {
		match flags.entry(deps.id) {
			Entry::Occupied(mut e) => { *e.get_mut() |= deps.dep_kinds; },
			Entry::Vacant(e) => { e.insert(deps.dep_kinds); },
		}
	}

	// Now build a list of all of the _used_ IDs (since the output contains
	// potentially irrelevant shit).
	let mut used: HashSet<&str> = resolve.iter().map(|n| n.id).collect();

	// We aren't interested in development-only packages, so let's strip
	// anything that isn't also used for build/runtime.
	for (id, flag) in &mut flags {
		if Dependency::FLAG_DEV == *flag & Dependency::FLAG_CONTEXT {
			used.remove(id);
		}

		// The dev flag has served its purpose and can be removed.
		*flag &= ! Dependency::FLAG_DEV;
	}

	// All that's left to do is compile an authoritative set of the used
	// dependencies in a friendly format.
	let out: BTreeSet<Dependency> = packages.into_iter()
		.filter_map(|p|
			if used.contains(p.id) {
				// Figure out the context flags.
				let mut context = flags.get(p.id).copied().unwrap_or(0);
				if 0 == context & Dependency::FLAG_CONTEXT {
					context |= Dependency::FLAG_RUNTIME;
				}
				if 0 == context & Dependency::FLAG_PLATFORM {
					context |= Dependency::FLAG_ANY;
				}

				Some(Dependency {
					name: String::from(p.name),
					version: p.version,
					license: if p.license.is_empty() { None } else { Some(p.license) },
					authors: p.authors,
					url: p.repository.map(String::from),
					context,
				})
			}
			else { None }
		)
		.collect();

	Ok(out)
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
				util::normalize_string(raw);

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
	util::normalize_string(raw);
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
mod tests {
	use super::*;

	#[test]
	fn t_parse_raw() {
		let raw = std::fs::read("skel/metadata.json")
			.expect("Missing skel/metadata.json");
		let raw = from_json(&raw).expect("Failed to marse metadata.json");

		// For now let's just count the results.
		assert_eq!(raw.len(), 87);
	}

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
}
