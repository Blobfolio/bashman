/*!
# `Cargo BashMan` — Dependency Credits

This is largely a trimmed-down version of `cargo_license`. Our needs are much
narrower than theirs.
*/

use cargo_metadata::{
	DependencyKind,
	DepKindInfo,
	MetadataCommand,
	Node,
	NodeDep,
	Package,
	PackageId,
};
use crate::BashManError;
use once_cell::sync::Lazy;
use oxford_join::OxfordJoin;
use regex::Regex;
use std::{
	collections::{
		HashMap,
		HashSet,
	},
	path::Path,
};




#[derive(Debug)]
/// # Dependency.
pub(super) struct Dependency {
	pub(super) name: String,
	pub(super) version: String,
	pub(super) authors: String,
	pub(super) license: String,
	pub(super) link: Option<String>,
}

impl From<Package> for Dependency {
	fn from(mut src: Package) -> Self {
		strip_markdown(&mut src.name);
		let mut version = src.version.to_string();
		strip_markdown(&mut version);

		Self {
			name: src.name,
			version,
			authors: nice_author(src.authors),
			license: src.license.map_or_else(String::new, |l| nice_license(&l)),
			link: src.repository,
		}
	}
}



/// # Get Dependencies.
pub(super) fn get_dependencies(src: &Path) -> Result<Vec<Dependency>, BashManError> {
	let metadata = {
		let mut cmd = MetadataCommand::new();
		cmd.manifest_path(&src);
		cmd.exec().map_err(|_| BashManError::InvalidManifest)?
	};

	// Parse out all of the package IDs in the dependency tree, excluding dev-
	// and build-deps.
	let deps = {
		let resolve = metadata.resolve.as_ref()
			.ok_or(BashManError::InvalidManifest)?;

		// Pull dependencies by package.
		let deps: HashMap<&PackageId, &Vec<NodeDep>> = resolve
			.nodes
			.iter()
			.map(|Node { id, deps, .. }| (id, deps))
			.collect();

		// Build a list of all unique, normal dependencies.
		let mut out: HashSet<&PackageId> = HashSet::new();
		let stack = &mut resolve.root.as_ref()
			.map_or_else(
				|| metadata.workspace_members.iter().collect(),
				|root| vec![root]
			);

		while let Some(package_id) = stack.pop() {
			if out.insert(package_id) {
				stack.extend(deps[package_id].iter().filter_map(
					|NodeDep { pkg, dep_kinds, .. }|
					if dep_kinds.iter().any(|DepKindInfo { kind, .. }| *kind == DependencyKind::Normal) {
						Some(pkg)
					}
					else { None }
				));
			}
		}

		out
	};

	// One final time around to pull the relevant package details for each
	// corresponding ID.
	let mut out: Vec<Dependency> = metadata.packages.into_iter()
        .filter(|p| deps.contains(&p.id))
        .map(Dependency::from)
        .collect();

    out.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));

	Ok(out)
}

/// # Normalize Licenses.
fn nice_license(raw: &str) -> String {
	let mut raw = raw.replace(" OR ", "/");
	strip_markdown(&mut raw);
	let mut list: Vec<&str> = raw.split('/').map(str::trim).collect();
	list.sort_unstable();
	list.dedup();
	list.oxford_or().into_owned()
}

/// # Normalize Authors.
fn nice_author(raw: Vec<String>) -> String {
	static RE1: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\[|\]|\||\(|\))").unwrap());
	static RE2: Lazy<Regex> = Lazy::new(|| Regex::new(r"(.+?) <([^>]+)>").unwrap());

	let list: Vec<String> = raw.into_iter()
		.map(|x| {
			let y = RE1.replace_all(&x, "");
			let z = RE2.replace_all(y.trim(), "[$1](mailto:$2)");
			if x == z { x }
			else { z.into_owned() }
		})
		.collect();
	list.oxford_and().into_owned()
}

/// # Lightly Sanitize.
///
/// Remove `[] <> () |` to help with later markdown display.
fn strip_markdown(raw: &mut String) {
	static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\[|\]|\||<|>|\(|\))").unwrap());

	let alt = RE.replace_all(raw.trim(), "");
	if raw != &alt {
		*raw = alt.into_owned();
	}
}
