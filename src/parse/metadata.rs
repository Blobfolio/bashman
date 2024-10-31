/*!
# Cargo BashMan: Raw Cargo Metadata JSON Parsing.
*/

use crate::{
	BashManError,
	Dependency,
	PackageName,
	TargetTriple,
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
use url::Url;



#[inline]
/// # Fetch Dependencies.
///
/// Run `cargo metadata` and parse the results into a sorted and deduped list
/// of dependencies.
pub(super) fn fetch_dependencies<P: AsRef<Path>>(
	src: P,
	features: bool,
	target: Option<TargetTriple>,
) -> Result<(BTreeSet<Dependency>, bool), BashManError> {
	let raw = cargo_exec(src, features, target)?;
	from_json(&raw, target.is_some())
}



#[derive(Debug, Deserialize)]
/// # Raw Top-Level Structure.
struct Raw<'a> {
	#[serde(borrow)]
	/// # Detailed Packages.
	packages: Vec<RawPackage<'a>>,

	#[serde(borrow)]
	/// # Resolved Tree.
	resolve: RawNodes<'a>,
}



#[derive(Debug, Deserialize)]
/// # Raw Nodes (Wrapper).
///
/// This is mostly just a wrapper around the list of nodes, but it also lets
/// us know the ID of the main/root package.
struct RawNodes<'a> {
	#[serde(default)]
	#[serde(borrow)]
	/// # Nodes.
	nodes: Vec<RawNode<'a>>,

	#[serde(borrow)]
	/// # Root.
	root: &'a str,
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
	#[serde(deserialize_with = "deserialize_node_deps")]
	/// # Dependency Details.
	///
	/// Note that dev-only dependencies are pruned during deserialization.
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
	#[serde(deserialize_with = "util::deserialize_license")]
	/// # License.
	license: String,

	#[serde(default)]
	#[serde(deserialize_with = "util::deserialize_authors")]
	/// # Author(s).
	authors: Vec<String>,

	#[serde(default)]
	/// # Repository URL.
	repository: Option<Url>,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_features")]
	/// # Has Features?
	features: bool,
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
fn cargo_exec<P: AsRef<Path>>(src: P, features: bool, target: Option<TargetTriple>)
-> Result<Vec<u8>, BashManError> {
	// Populate the command arguments.
	let src: &Path = src.as_ref();
	let mut cmd = cargo_cmd();
	cmd.args([
		"metadata",
		"--quiet",
		"--color", "never",
		"--format-version", "1",
		if features { "--all-features" } else { "--no-default-features" },
		"--manifest-path",
	]);
	cmd.arg(src);
	if let Some(target) = target {
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
	else { Err(BashManError::Credits) }
}

/// # Default Dependency Kinds.
const fn default_depkinds() -> u8 { Dependency::FLAG_RUNTIME }



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
/// # Deserialize: Features.
///
/// We just want to know if there _are_ features.
fn deserialize_features<'de, D>(deserializer: D) -> Result<bool, D::Error>
where D: Deserializer<'de> {
	if let Ok(mut map) = <HashMap<String, Vec<Cow<str>>>>::deserialize(deserializer) {
		map.remove("default");
		return Ok(! map.is_empty());
	}

	Ok(false)
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
/// # Deserialize: Node Dependencies.
///
/// This method won't fail, but dev-only dependencies will be pruned before
/// return.
fn deserialize_node_deps<'de, D>(deserializer: D) -> Result<Vec<RawNodeDep<'de>>, D::Error>
where D: Deserializer<'de> {
	let mut out = <Vec<RawNodeDep>>::deserialize(deserializer).unwrap_or_default();
	out.retain(|nd| Dependency::FLAG_DEV != nd.dep_kinds & Dependency::FLAG_CONTEXT);
	Ok(out)
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
/// This is called by `Manifest::dependencies` up to two times depending on
/// whether or not the root has features (so we can figure out what is
/// optional.)
fn from_json(raw: &[u8], targeted: bool)
-> Result<(BTreeSet<Dependency>, bool), BashManError> {
	let Raw { packages, resolve } = serde_json::from_slice(raw)
		.map_err(|e| BashManError::ParseCargoMetadata(e.to_string()))?;

	// As we don't care about dev-only dependencies, we have to manually
	// traverse the tree to figure out what is actually used. Before we do
	// that, let's build a map of parent/child pairs to make all those lookups
	// less terrible.
	let nice_resolve: HashMap<&str, Vec<&str>> = resolve.nodes.iter()
		.map(|n| (n.id, n.deps.iter().map(|nd| nd.id).collect()))
		.collect();

	// The traversal itself is simple if stupid: starting with the root
	// package, enqueue its dependencies, then each of their dependencies, and
	// so on until we run out of fresh references.
	let mut used: HashSet<&str> = HashSet::with_capacity(packages.len());
	let mut queue = vec![resolve.root];
	while let Some(next) = queue.pop() {
		// To prevent infinite recursion, only enqueue a given project's
		// dependencies the first time it comes up.
		if used.insert(next) {
			if let Some(next) = nice_resolve.get(next) {
				queue.extend_from_slice(next);
			}
		}
	}

	// Dependencies can be shared and used in different contexts, so let's
	// quickly calculate the combined values for each individual package.
	let mut flags = HashMap::<&str, u8>::with_capacity(used.len());
	for dep in resolve.nodes.iter().filter(|r| used.contains(r.id)).flat_map(|r| r.deps.iter()) {
		match flags.entry(dep.id) {
			Entry::Occupied(mut e) => { *e.get_mut() |= dep.dep_kinds; },
			Entry::Vacant(e) => { e.insert(dep.dep_kinds); },
		}
	}

	// The root node isn't needed in the output; the easiest way to filter it
	// out is to pretend it wasn't used.
	used.remove(resolve.root);

	// The context flags aren't needed in the output either, and if we're doing
	// a targeted lookup there's no point noting target-specificness. For now,
	// let's just build the mask so it can be easily applied to the final
	// assignment.
	let antiflags =
		if targeted { Dependency::FLAG_CONTEXT | Dependency::FLAG_TARGET }
		else { Dependency::FLAG_CONTEXT };

	// All that's left to do is compile an authoritative set of the used
	// dependencies in a friendly format.
	let mut features = false;
	let out: BTreeSet<Dependency> = packages.into_iter()
		.filter_map(|p|
			if used.contains(p.id) {
				// Figure out the context flags.
				let mut context = flags.remove(p.id).map_or(0, |f| f & ! antiflags);
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
			else {
				// Before we throw away the root package, note whether or not
				// it had crate features.
				if p.id == resolve.root { features = p.features }
				None
			}
		)
		.collect();

	Ok((out, features))
}



#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn t_from_json() {
		let raw = std::fs::read("skel/metadata.json")
			.expect("Missing skel/metadata.json");
		let (raw1, feat1) = from_json(&raw, false).expect("Failed to marse metadata.json");
		let (raw2, feat2) = from_json(&raw, true).expect("Failed to marse metadata.json");

		// We don't have features.
		assert!(! feat1);
		assert!(! feat2);

		// For now let's just count the results.
		assert_eq!(raw1.len(), 86);
		assert_eq!(raw2.len(), 86);

		// And make sure the target-specific flags were conditionally applied.
		assert!(raw1.iter().any(|d| d.context().contains("target-specific")));
		assert!(raw2.iter().all(|d| ! d.context().contains("target-specific")));
	}

	#[test]
	/// # Node Dependency Kind Deserialization.
	///
	/// We're deviating quite a bit from the natural structure, so it's a good
	/// idea to verify the data gets crunched correctly.
	fn t_raw_dep_kind() {
		// No values.
		let kind: RawNodeDepKind = serde_json::from_str(r#"{"kind": null, "target": null}"#)
			.expect("Failed to deserialize RawNodeDepKind");
		assert!(! kind.dev);
		assert!(! kind.target);
		assert_eq!(kind.as_flag(), Dependency::FLAG_RUNTIME | Dependency::FLAG_ANY);

		// Build.
		let kind: RawNodeDepKind = serde_json::from_str(r#"{"kind": "build", "target": null}"#)
			.expect("Failed to deserialize RawNodeDepKind");
		assert!(! kind.dev);
		assert!(! kind.target);
		assert_eq!(kind.as_flag(), Dependency::FLAG_RUNTIME | Dependency::FLAG_ANY);

		// Build and Target.
		let kind: RawNodeDepKind = serde_json::from_str(r#"{"kind": "build", "target": "cfg(unix)"}"#)
			.expect("Failed to deserialize RawNodeDepKind");
		assert!(! kind.dev);
		assert!(kind.target);
		assert_eq!(kind.as_flag(), Dependency::FLAG_RUNTIME | Dependency::FLAG_TARGET);

		// Target.
		let kind: RawNodeDepKind = serde_json::from_str(r#"{"kind": null, "target": "cfg(target_os = \"hermit\")"}"#)
			.expect("Failed to deserialize RawNodeDepKind");
		assert!(! kind.dev);
		assert!(kind.target);
		assert_eq!(kind.as_flag(), Dependency::FLAG_RUNTIME | Dependency::FLAG_TARGET);

		// Dev.
		let kind: RawNodeDepKind = serde_json::from_str(r#"{"kind": "dev", "target": null}"#)
			.expect("Failed to deserialize RawNodeDepKind");
		assert!(kind.dev);
		assert!(! kind.target);
		assert_eq!(kind.as_flag(), Dependency::FLAG_DEV);

		// Dev and target.
		let kind: RawNodeDepKind = serde_json::from_str(r#"{"kind": "dev", "target": "cfg(target_os = \"wasi\")"}"#)
			.expect("Failed to deserialize RawNodeDepKind");
		assert!(kind.dev);
		assert!(kind.target);
		assert_eq!(kind.as_flag(), Dependency::FLAG_DEV); // Dev takes priority.
	}
}
