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
	cmp::Ordering,
	collections::{
		HashMap,
		HashSet,
	},
	path::Path,
};
use trimothy::TrimMut;



#[derive(Debug)]
/// # Dependency.
pub(super) struct Dependency {
	pub(super) name: String,
	pub(super) version: String,
	pub(super) authors: String,
	pub(super) license: String,
	pub(super) link: Option<String>,
}

impl Eq for Dependency {}

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

impl Ord for Dependency {
	#[inline]
	fn cmp(&self, other: &Self) -> Ordering { self.name.cmp(&other.name) }
}

impl PartialEq for Dependency {
	#[inline]
	fn eq(&self, other: &Self) -> bool { self.name == other.name }
}

impl PartialOrd for Dependency {
	#[inline]
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
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
		let resolve = metadata.resolve.as_ref().ok_or(BashManError::InvalidManifest)?;

		// Pull dependencies by package.
		let deps: HashMap<&PackageId, &[NodeDep]> = resolve.nodes.iter()
			.map(|Node { id, deps, .. }| (id, deps.as_slice()))
			.collect();

		// Build a list of all unique, normal dependencies.
		let mut out: HashSet<&PackageId> = HashSet::new();
		let mut stack: Vec<_> = resolve.root.as_ref()
			.map_or_else(
				|| metadata.workspace_members.iter().collect(),
				|root| vec![root]
			);

		while let Some(package_id) = stack.pop() {
			if out.insert(package_id) {
				if let Some(d) = deps.get(package_id).copied() {
					if ! d.is_empty() {
						for NodeDep { pkg, dep_kinds, .. } in d {
							if dep_kinds.iter().any(|DepKindInfo { kind, target, .. }| target.is_none() && *kind == DependencyKind::Normal) {
								stack.push(pkg);
							}
						}
					}
				}
			}
		}

		out
	};

	// One final time around to pull the relevant package details for each
	// corresponding ID.
	let mut out: Vec<Dependency> = metadata.packages.into_iter()
		.filter_map(|p|
			if deps.contains(&p.id) { Some(Dependency::from(p)) }
			else { None }
		)
		.collect();

	out.sort_unstable();

	Ok(out)
}



/// # Normalize Authors.
fn nice_author(mut raw: Vec<String>) -> String {
	static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(.+?) <([^>]+)>").unwrap());

	for x in &mut raw {
		x.trim_mut();
		x.retain(|c| ! matches!(c, '[' | ']' | '(' | ')' | '|'));

		let y = RE.replace_all(x, "[$1](mailto:$2)");
		if *x != y {
			*x = y.into_owned();
		}
	}

	raw.oxford_and().into_owned()
}

/// # Normalize Licenses.
fn nice_license(raw: &str) -> String {
	let mut raw = raw.replace(" OR ", "/");
	raw.retain(|c| ! matches!(c, '[' | ']' | '<' | '>' | '(' | ')' | '|'));

	let mut list: Vec<&str> = raw.split('/').map(str::trim).collect();
	list.sort_unstable();
	list.dedup();
	list.oxford_or().into_owned()
}

/// # Lightly Sanitize.
///
/// Remove `[] <> () |` to help with later markdown display.
fn strip_markdown(raw: &mut String) {
	raw.trim_mut();
	raw.retain(|c| ! matches!(c, '[' | ']' | '<' | '>' | '(' | ')' | '|'));
}



#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn t_strip_markdown() {
		let mut raw: String = r" H(E)L[L]O <W>O|RLD ".to_string();
		strip_markdown(&mut raw);
		assert_eq!(raw, "HELLO WORLD");
	}
}
