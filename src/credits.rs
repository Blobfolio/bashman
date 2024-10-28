/*!
# Cargo BashMan: Crate Credits.
*/

use adbyss_psl::Domain;
use cargo_metadata::{
	CargoOpt,
	DependencyKind,
	DepKindInfo,
	Metadata,
	MetadataCommand,
	Node,
	NodeDep,
	Package,
	PackageId,
};
use crate::{
	BashManError,
	Manifest,
};
use oxford_join::{
	OxfordJoin,
	OxfordJoinFmt,
};
use std::{
	cmp::Ordering,
	fmt,
	collections::{
		BTreeSet,
		HashMap,
		HashSet,
	},
	path::{
		Path,
		PathBuf,
	},
};
use trimothy::TrimMut;
use utc2k::Utc2k;



/// # Table Header.
const HEADER: &str = "| Package | Version | Author(s) | License | Context |\n| ---- | ---- | ---- | ---- | ---- |\n";



/// # Crate Credits.
///
/// This struct is used to write the crate credits to a markdown file.
///
/// Most of the magic is accomplished via the `Display` impl, but
/// `Credits::write` is what the `main.rs` actually calls to save the contents
/// to a file.
pub(super) struct CreditsWriter<'a> {
	#[expect(dead_code, reason = "We'll want this eventually.")]
	/// # Cargo File.
	src: &'a Path,

	/// # Output File.
	dst: PathBuf,

	/// # Package Name.
	name: &'a str,

	/// # Package Version.
	version: &'a str,

	/// # Dependencies.
	dependencies: Vec<Dependency>,
}

impl<'a> fmt::Display for CreditsWriter<'a> {
	/// # Write Credits!
	///
	/// This method writes a markdown table entry for the dependency.
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		writeln!(
			f,
			"# Project Dependencies
    Package:   {}
    Version:   {}
    Generated: {} UTC
",
			self.name,
			self.version,
			Utc2k::now(),
		)?;

		// There may not be any dependencies.
		if self.dependencies.is_empty() {
			return f.write_str("This project has no dependencies.\n");
		}

		f.write_str(HEADER)?;
		for dep in &self.dependencies { <Dependency as fmt::Display>::fmt(dep, f)?; }

		Ok(())
	}
}

impl<'a> TryFrom<&'a Manifest> for CreditsWriter<'a> {
	type Error = BashManError;

	fn try_from(man: &'a Manifest) -> Result<Self, Self::Error> {
		let src = man.src();
		let dst = man.dir_credits()?.join("CREDITS.md");
		let cmd = man.main_cmd().ok_or(BashManError::Credits)?;
		let name = cmd.bin();

		// Fetch the dependencies with all features enabled, but if that
		// doesn't work, try again with just the defaults.
		let mut dependencies: Vec<Dependency> = get_dependencies(src, Some(CargoOpt::AllFeatures))
			.or_else(|_| get_dependencies(src, None))?
			.into_iter()
			.collect();

		// Remove ourselves if included.
		if let Some(pos) = dependencies.iter().position(|d| d.name == name) {
			dependencies.remove(pos);
		}

		// There doesn't seem to be an easy way tell whether or not a given
		// dependency is feature-dependent, so let's repeat the process with
		// all features disabled to compare and contrast.
		if ! dependencies.is_empty() {
			if let Ok(required) = get_dependencies(src, Some(CargoOpt::NoDefaultFeatures)) {
				for dep in &mut dependencies {
					if ! required.contains(dep) {
						dep.flags |= Dependency::FLAG_OPTIONAL;
					}
				}
			}
		}

		// Done!
		Ok(Self {
			src,
			dst,
			name,
			version: cmd.version(),
			dependencies,
		})
	}
}

impl<'a> CreditsWriter<'a> {
	/// # Write Credits!
	///
	/// This method is called by `main.rs` to generate and save the crate
	/// credits.
	///
	/// The shared `buf` is used to help reduce allocations across the various
	/// writes the program will make.
	///
	/// Errors will be bubbled up if encountered, otherwise the output path
	/// is returned.
	pub(super) fn write(self, buf: &mut String) -> Result<PathBuf, BashManError> {
		use std::fmt::Write;

		// Reset the buffer and write our completions into it.
		buf.truncate(0);
		write!(buf, "{self}").map_err(|_| BashManError::Credits)?;

		write_atomic::write_file(&self.dst, buf.as_bytes())
			.map_err(|_| BashManError::Write(self.dst.to_string_lossy().into_owned()))
			.map(|()| self.dst)
	}
}




#[derive(Debug)]
/// # Dependency.
struct Dependency {
	/// # Name.
	name: String,

	/// # Version.
	version: String,

	/// # Author(s).
	authors: Vec<String>,

	/// # License.
	license: String,

	/// # URL.
	link: Option<String>,

	/// # Flags.
	flags: u8,
}

impl Eq for Dependency {}

impl fmt::Display for Dependency {
	/// # Write Credits!
	///
	/// This method writes a markdown table entry for the dependency.
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if let Some(link) = self.link.as_deref() {
			writeln!(
				f,
				"| [{}]({}) | {} | {} | {} | {} |",
				self.name,
				link,
				self.version,
				OxfordJoinFmt::and(self.authors.as_slice()),
				self.license,
				self.context(),
			)
		}
		else {
			writeln!(
				f,
				"| {} | {} | {} | {} | {} |",
				self.name,
				self.version,
				OxfordJoinFmt::and(self.authors.as_slice()),
				self.license,
				self.context(),
			)
		}
	}
}

impl From<Package> for Dependency {
	fn from(mut src: Package) -> Self {
		strip_markdown(&mut src.name);
		let mut version = src.version.to_string();
		strip_markdown(&mut version);
		nice_authors(&mut src.authors);

		Self {
			name: src.name,
			version,
			authors: src.authors,
			license: src.license.map_or_else(String::new, |l| nice_license(&l)),
			link: src.repository,
			flags: 0,
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

impl Dependency {
	/// # Flag: Optional.
	const FLAG_OPTIONAL: u8 = 0b0001;

	/// # Flag: Used at Runtime.
	const FLAG_RUNTIME: u8 =  0b0010;

	/// # Flag: Used at Build.
	const FLAG_BUILD: u8 =    0b0100;

	/// # Flag: Runtime and Build.
	const FLAG_KINDS: u8 = Self::FLAG_BUILD | Self::FLAG_RUNTIME;

	/// # Context.
	const fn context(&self) -> &'static str {
		let optional = Self::FLAG_OPTIONAL == self.flags & Self::FLAG_OPTIONAL;
		let build = Self::FLAG_BUILD == self.flags & Self::FLAG_KINDS;
		match (optional, build) {
			(true, true) => "optional, build",
			(true, false) => "optional",
			(false, true) => "build",
			(false, false) => "",
		}
	}
}



/// # Get Dependencies.
///
/// Fetch, parse, and filter the dependencies.
fn get_dependencies(src: &Path, features: Option<CargoOpt>) -> Result<BTreeSet<Dependency>, BashManError> {
	let mut cmd = MetadataCommand::new();
	cmd.manifest_path(src);
	if let Some(features) = features { cmd.features(features); }
	let metadata = cmd.exec().map_err(|_| BashManError::Credits)?;

	parse_dependencies(metadata)
}

/// # Normalize Authors.
fn nice_authors(raw: &mut Vec<String>) {
	for x in raw.iter_mut() {
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

/// # Parse Dependencies.
///
/// Traverse the root crate's dependencies and return them in a much simpler
/// form than the one provided by `cargo_metadata`.
fn parse_dependencies(metadata: Metadata) -> Result<BTreeSet<Dependency>, BashManError> {
	// Parse out all of the package IDs in the dependency tree, excluding dev-
	// and build-deps.
	let resolve = metadata.resolve.as_ref().ok_or(BashManError::Credits)?;

	// Pull dependencies by package.
	let raw: HashMap<&PackageId, &[NodeDep]> = resolve.nodes.iter()
		.map(|Node { id, deps, .. }| (id, deps.as_slice()))
		.collect();

	// Classify each dependency.
	let mut flags = HashMap::<&PackageId, u8>::with_capacity(raw.len());
	for NodeDep { pkg, dep_kinds, .. } in raw.values().copied().flatten() {
		let mut f = 0;
		for DepKindInfo { kind, target, .. } in dep_kinds {
			if target.is_none() {
				f |= match kind {
					DependencyKind::Normal => Dependency::FLAG_RUNTIME,
					DependencyKind::Build => Dependency::FLAG_BUILD,
					_ => 0,
				};
			}
		}
		*(flags.entry(pkg).or_insert(0)) |= f;
	}

	// Build a list of all unique, normal dependencies.
	let mut deps: HashSet<&PackageId> = HashSet::new();
	let mut stack: Vec<_> = resolve.root.as_ref()
		.map_or_else(
			|| metadata.workspace_members.iter().collect(),
			|root| vec![root]
		);


	// Drain and repopulate the queue until we've reached the end.
	while let Some(package_id) = stack.pop() {
		if deps.insert(package_id) {
			if let Some(d) = raw.get(package_id).copied() {
				for NodeDep { pkg, .. } in d {
					if flags.get(pkg).map_or(false, |&f| 0 != f & Dependency::FLAG_KINDS) {
						stack.push(pkg);
					}
				}
			}
		}
	}

	// Boil down the "found" dependencies into a structure we can use.
	Ok(
		metadata.packages.into_iter()
			.filter_map(|p|
				if deps.contains(&p.id) {
					let f = flags.get(&p.id).copied().unwrap_or(0);
					let mut d = Dependency::from(p);
					d.flags = f;
					Some(d)
				}
				else { None }
			)
			.collect()
	)
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
		let mut raw: String = " H(E)L[L]O <W>O|RLD |".to_owned();
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
