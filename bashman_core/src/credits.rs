/*!
# `Cargo BashMan` — Dependency Credits

This is largely a trimmed-down version of `cargo_license`. Our needs are much
narrower than theirs.
*/

use adbyss_psl::Domain;
use cargo_metadata::{
	CargoOpt,
	DependencyKind,
	DepKindInfo,
	MetadataCommand,
	Node,
	NodeDep,
	Package,
	PackageId,
};
use crate::BashManError;
use oxford_join::OxfordJoin;
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
pub(super) fn get_dependencies(src: &Path, features: Option<&str>) -> Result<Vec<Dependency>, BashManError> {
	let metadata = {
		let mut cmd = MetadataCommand::new();
		cmd.manifest_path(&src);
		if let Some(features) = features {
			let features: Vec<String> = features.split(',')
				.filter_map(|f| {
					let f = f.trim();
					if f.is_empty() { None }
					else { Some(f.to_owned()) }
				})
				.collect();
			if ! features.is_empty() {
				cmd.features(CargoOpt::SomeFeatures(features));
			}
		}
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
	for x in &mut raw {
		x.retain(|c| ! matches!(c, '[' | ']' | '(' | ')' | '|'));
		x.trim_mut();

		// Reformat author line as markdown.
		let bytes = x.as_bytes();
		if let Some(start) = bytes.iter().position(|b| b'<'.eq(b)) {
			if let Some(end) = bytes.iter().rposition(|b| b'>'.eq(b)).filter(|p| start.lt(p)) {
				match (nice_name(x[..start].trim()), nice_email(&x[start + 1..end])) {
					// [Name](mailto:email)
					(Some(n), Some(e)) => {
						x.truncate(0);
						x.push('[');
						x.push_str(&n);
						x.push_str("](mailto:");
						x.push_str(&e);
						x.push(')');
					},
					// Name
					(Some(mut n), None) => { std::mem::swap(x, &mut n); },
					// [email](mailto:email)
					(None, Some(e)) => {
						x.truncate(0);
						x.push('[');
						x.push_str(&e);
						x.push_str("](mailto:");
						x.push_str(&e);
						x.push(')');
					},
					// Empty.
					(None, None) => { x.truncate(0); }
				}
			}
			// Get rid of the brackets; they weren't used correctly.
			else {
				x.retain(|c| ! matches!(c, '<' | '>'));
				x.trim_mut();
			}
		}
		// There weren't any opening <, but there might be closing > we should
		// remove.
		else {
			x.retain(|c| c != '>');
			x.trim_mut();
		}
	}

	// One final thing: remove empties.
	raw.retain(|x| ! x.is_empty());

	// Done!
	raw.oxford_and().into_owned()
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
fn nice_email(raw: &str) -> Option<String> {
	let (user, host) = raw.trim_matches(|c: char| c.is_whitespace() || c == '<' || c == '>')
		.split_once('@')?;

	// We need both parts.
	if user.is_empty() || host.is_empty() { return None; }

	// Make sure the host is parseable.
	let host = Domain::new(host)?;

	// Let's start with the user.
	let mut out = String::with_capacity(user.len() + host.len() + 1);
	for c in user.chars() {
		match c {
			'a'..='z' | '0'..='9' | '.' | '+' | '-' | '_' => { out.push(c); },
			'A'..='Z' => out.push(c.to_ascii_lowercase()),
			_ => return None,
		}
	}

	// Add the @ and host.
	out.push('@');
	out.push_str(host.as_str());

	Some(out)
}

/// # Normalize Licenses.
fn nice_license(raw: &str) -> String {
	let mut raw = raw.replace(" OR ", "/");
	raw.retain(|c| ! matches!(c, '[' | ']' | '<' | '>' | '(' | ')' | '|'));

	let mut list: Vec<&str> = raw.split('/')
		.filter_map(|line| {
			let line = line.trim();
			if line.is_empty() { None }
			else { Some(line) }
		})
		.collect();
	list.sort_unstable();
	list.dedup();
	list.oxford_or().into_owned()
}

/// # Nice Name.
///
/// This performs some light cleaning and trimming and returns the result if it
/// is non-empty.
fn nice_name(raw: &str) -> Option<String> {
	// The name is unlikely to have < or >, but they should be stripped out if
	// present.
	let mut out: String = raw
		.chars()
		.filter(|c| '<'.ne(c) && '>'.ne(c))
		.collect();

	out.trim_mut();
	if out.is_empty() { None }
	else { Some(out) }
}

/// # Lightly Sanitize.
///
/// Remove `[] <> () |` to help with later markdown display.
fn strip_markdown(raw: &mut String) {
	raw.retain(|c| ! matches!(c, '[' | ']' | '<' | '>' | '(' | ')' | '|'));
	raw.trim_mut();
}



#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn t_strip_markdown() {
		let mut raw: String = r" H(E)L[L]O <W>O|RLD |".to_string();
		strip_markdown(&mut raw);
		assert_eq!(raw, "HELLO WORLD");
	}

	#[test]
	fn t_nice_email() {
		assert_eq!(
			nice_email(" < JoSh@BloBfolio.com> "),
			Some("josh@blobfolio.com".to_owned())
		);

		assert_eq!(nice_email(" < JoSh@BloBfolio.x> "), None);

		assert_eq!(
			nice_email("USER@♥.com"),
			Some("user@xn--g6h.com".to_owned())
		);
	}
}
