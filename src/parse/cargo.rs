/*!
# Cargo BashMan: Cargo Metadata Parsing.

This module contains various deserialization helpers for extracting the
relevant data from the JSON output of a `cargo metadata` command.
*/

use crate::{
	BashManError,
	Dependency,
	Flag,
	KeyWord,
	OptionFlag,
	PackageName,
	Subcommand,
	TargetTriple,
	TrailingArg,
};
use semver::Version;
use serde::{
	de,
	Deserialize,
	Deserializer,
};
use serde_json::value::RawValue;
use std::{
	borrow::Cow,
	collections::{
		BTreeMap,
		BTreeSet,
		hash_map::Entry,
		HashMap,
		HashSet,
	},
	cmp::Ordering,
	path::Path,
};
use super::{
	ManifestData,
	Section,
	util::{
		self,
		CargoMetadata,
	},
};
use trimothy::NormalizeWhitespace;
use url::Url;



/// # Fetch Manifest Data.
///
/// This executes and parses the raw JSON output from `cargo metadata` into
/// more easily-consumable structures.
/// # New.
pub(super) fn fetch(src: &Path, target: Option<TargetTriple>)
-> Result<(RawMainPackage, BTreeSet<Dependency>), BashManError> {
	let mut cargo = CargoMetadata::new(src, target).with_features(false);

	// Query without features first.
	let raw1 = cargo.exec()?;
	let (packages, resolve) = serde_json::from_slice::<Raw>(&raw1)
		.map_err(|e| BashManError::ParseCargoMetadata(e.to_string()))?
		.finalize(Some(cargo));

	// Build the dependency list (and find the main package).
	let flags = resolve.flags(target.is_some());
	let mut main = None;
	let mut deps = BTreeSet::<Dependency>::new();
	for p in packages {
		// Split out the main crate.
		if p.id == resolve.root { main.replace(p); }
		// Convert and keep used dependencies.
		else if resolve.nodes.contains_key(p.id) {
			let context = flags.get(p.id).copied().unwrap_or(0);
			let p = p.try_into_dependency(context)?;
			deps.insert(p);
		}
	}

	// We should have a main package by now.
	let RawPackage { id, name, version, description, features, metadata, .. } = main.ok_or_else(|| BashManError::ParseCargoMetadata(
		"unable to determine root package".to_owned()
	))?;
	let main = RawMainPackage::try_from_parts(name, &version, description, metadata)?;
	let features = features.map_or(false, deserialize_features);

	// If this crate has features, repeat the process to figure out if
	// there are any additional optional dependencies. If this fails for
	// whatever reason, we'll stick with what we have.
	if features {
		cargo = cargo.with_features(true);
		if let Ok(raw2) = cargo.exec() {
			if let Ok((packages, resolve)) = serde_json::from_slice::<Raw>(&raw2).map(|r| r.finalize(Some(cargo))) {
				// Build the dependency list (and find the main package).
				let flags = resolve.flags(target.is_some());
				for p in packages {
					if p.id != id && resolve.nodes.contains_key(p.id) {
						let context = flags.get(p.id)
							.copied()
							.unwrap_or(0) | Dependency::FLAG_OPTIONAL;
						if let Ok(d) = p.try_into_dependency(context) {
							deps.insert(d);
						}
					}
				}
			}
		}
	}

	// Finish deserializing the main package.
	Ok((main, deps))
}

#[cfg(test)]
/// # Dummy Fetch.
///
/// This is a testing version of `fetch` that parses a static (pre-generated)
/// dataset instead of running `cargo metadata`.
pub(super) fn fetch_test(target: Option<TargetTriple>)
-> Result<(RawMainPackage, BTreeSet<Dependency>), BashManError> {
	// Parse the static data.
	let raw1 = std::fs::read("skel/metadata.json")
		.map_err(|_| BashManError::Read("skel/metadata.json".to_owned()))?;
	let (packages, resolve) = serde_json::from_slice::<Raw>(&raw1)
		.map_err(|e| BashManError::ParseCargoMetadata(e.to_string()))?
		.finalize(None);

	// Build the dependency list (and find the main package).
	let flags = resolve.flags(target.is_some());
	let mut main = None;
	let mut deps = BTreeSet::<Dependency>::new();
	for p in packages {
		// Split out the main crate.
		if p.id == resolve.root { main.replace(p); }
		// Convert and keep used dependencies.
		else if resolve.nodes.contains_key(p.id) {
			let context = flags.get(p.id).copied().unwrap_or(0);
			let p = p.try_into_dependency(context)?;
			deps.insert(p);
		}
	}

	// We should have a main package by now.
	let RawPackage { name, version, description, features, metadata, .. } = main.ok_or_else(|| BashManError::ParseCargoMetadata(
		"unable to determine root package".to_owned()
	))?;
	let main = RawMainPackage::try_from_parts(name, &version, description, metadata)?;

	// We don't have features.
	assert!(! features.map_or(false, deserialize_features), "No features expected!");

	// Finish deserializing the main package.
	Ok((main, deps))
}



#[derive(Debug)]
/// # Main Package.
///
/// This is almost the same as `RawPackage`, but includes the the `bashman`
/// metadata, if any.
pub(super) struct RawMainPackage {
	/// # Bash Output Directory.
	pub(super) dir_bash: Option<String>,

	/// # Manual Output Directory.
	pub(super) dir_man: Option<String>,

	/// # Credits Output Directory.
	pub(super) dir_credits: Option<String>,

	/// # Subcommands.
	pub(super) subcommands: Vec<Subcommand>,

	/// # Extra Credits.
	pub(super) credits: Vec<Dependency>,
}

impl RawMainPackage {
	/// # From Raw Parts.
	///
	/// This method consumes the relevant parts of a `RawPackage` object and
	/// returns an owned `RawMainPackage`.
	///
	/// Note the distance between here and there is quite long… Haha.
	fn try_from_parts<'a>(
		name: PackageName,
		version: &Version,
		description: Option<&'a RawValue>,
		metadata: Option<&'a RawValue>,
	) -> Result<Self, BashManError> {
		// Deserialize deferred fields.
		let description = description
			.ok_or_else(|| BashManError::ParseCargoMetadata(
				"missing description for main package".to_owned()
			))
			.and_then(|raw|
				util::deserialize_nonempty_str_normalized(raw)
					.map_err(|e| BashManError::ParseCargoMetadata(e.to_string()))
			)?;

		let RawBashMan { nice_name, dir_bash, dir_man, dir_credits, subcommands, flags, options, args, sections, credits } = match metadata {
			Some(m) => deserialize_bashman(m)?.unwrap_or_default(),
			None => RawBashMan::default(),
		};

		// Build the subcommands.
		let mut subs = BTreeMap::<String, Subcommand>::new();
		let main = Subcommand {
			nice_name,
			name: KeyWord::from(name),
			description,
			version: version.to_string(),
			parent: None,
			data: ManifestData {
				sections: sections.into_iter().map(Section::from).collect(),
				..ManifestData::default()
			},
		};
		for raw in subcommands {
			let sub = raw.into_subcommand(
				main.version.clone(),
				Some((main.nice_name().to_owned(), main.name.clone())),
			);
			subs.insert(sub.name.as_str().to_owned(), sub);
		}
		subs.insert(String::new(), main);

		// Add Flags.
		for line in flags {
			let RawSwitch { short, long, description, duplicate, mut subcommands } = line;
			let flag = Flag { short, long, description, duplicate };
			if let Some(last) = subcommands.pop_last() {
				for s in subcommands {
					add_subcommand_flag(&mut subs, s, flag.clone())?;
				}
				add_subcommand_flag(&mut subs, last, flag)?;
			}
		}

		// Add Options.
		for line in options {
			let RawOption { short, long, description, label, path, duplicate, mut subcommands } = line;
			let option = OptionFlag {
				flag: Flag { short, long, description, duplicate },
				label: label.unwrap_or_else(|| "<VAL>".to_owned()),
				path,
			};
			if let Some(last) = subcommands.pop_last() {
				for s in subcommands {
					add_subcommand_option(&mut subs, s, option.clone())?;
				}
				add_subcommand_option(&mut subs, last, option)?;
			}
		}

		// Add Args.
		for line in args {
			let RawArg { label, description, mut subcommands } = line;
			let arg = TrailingArg {
				label: label.unwrap_or_else(|| "<ARG(S)…>".to_owned()),
				description,
			};
			if let Some(last) = subcommands.pop_last() {
				for s in subcommands {
					add_subcommand_arg(&mut subs, s, arg.clone())?;
				}
				add_subcommand_arg(&mut subs, last, arg)?;
			}
		}

		Ok(Self {
			dir_bash,
			dir_man,
			dir_credits,
			subcommands: subs.into_values().collect(),
			credits: credits.into_iter().map(Dependency::from).collect(),
		})
	}
}



#[derive(Debug, Deserialize)]
/// # Top-Level Structure.
///
/// The dependency details are split between two members, with `packages`
/// holding the verbose details and `nodes` holding inter-relations and
/// context.
///
/// Both lists include unused "dependencies" in the raw `cargo metadata` output
/// so a lot of manual deserialization is performed to keep the data sane.
struct Raw<'a> {
	#[serde(borrow)]
	/// # Packages.
	packages: Vec<RawPackage<'a>>,

	#[serde(borrow)]
	/// # Workspace Members.
	workspace_members: HashSet<&'a str>,

	#[serde(borrow)]
	/// # Resolved Nodes.
	resolve: RawResolve<'a>,
}

impl<'a> Raw<'a> {
	/// # Finalize!
	///
	/// This takes care of a few big-picture tasks post-deserialization and
	/// returns the packages and node lists.
	fn finalize(self, cargo: Option<CargoMetadata<'_>>)
	-> (Vec<RawPackage<'a>>, RawResolve<'a>) {
		let Self { packages, workspace_members, mut resolve } = self;
		let mut used = cargo.and_then(|c| c.exec_tree(&packages))
			.unwrap_or_default();

		// If cargo tree couldn't help us figure out which dependencies are
		// actually used, let's take a guess by traversing the root
		// dependencies, then each of their dependencies, and so on.
		let mut queue = Vec::new();
		if used.is_empty() || ! used.contains(resolve.root) {
			used.clear();
			queue.push(resolve.root);
			while let Some(next) = queue.pop() {
				// Only enqueue a given package's dependencies once to avoid infinite
				// loops.
				if used.insert(next) {
					// Add its children, if any.
					if let Some(next) = resolve.nodes.get(next) {
						queue.extend(next.iter().map(|nd| nd.id));
					}
				}
			}
		}

		// Remove unused node chains and dependencies.
		resolve.nodes.retain(|k, _| used.contains(k));
		for v in resolve.nodes.values_mut() {
			v.retain(|nd| used.contains(nd.id));
		}

		// Now let's traverse what remains to find the "normal" dependencies so
		// we can recurisvely propagate build flags to build-only
		// sub-dependencies.
		used.clear();
		queue.push(resolve.root);
		while let Some(next) = queue.pop() {
			if used.insert(next) {
				// Add its children, if any.
				if let Some(next) = resolve.nodes.get(next) {
					for nd in next {
						if Dependency::FLAG_CTX_NORMAL == nd.dep_kinds & Dependency::FLAG_CTX_NORMAL {
							queue.push(nd.id);
						}
					}
				}
			}
		}
		for (k, v) in &mut resolve.nodes {
			if ! used.contains(k) {
				for nd in v {
					nd.dep_kinds = (nd.dep_kinds & ! Dependency::MASK_CTX) | Dependency::FLAG_CTX_BUILD;
				}
			}
		}

		// Same as above, but this time we're looking for untargeted
		// dependencies so we can propagate conditionality where appropriate.
		used.clear();
		queue.push(resolve.root);
		while let Some(next) = queue.pop() {
			if used.insert(next) {
				// Add its children, if any.
				if let Some(next) = resolve.nodes.get(next) {
					for nd in next {
						if Dependency::FLAG_TARGET_ANY == nd.dep_kinds & Dependency::FLAG_TARGET_ANY {
							queue.push(nd.id);
						}
					}
				}
			}
		}
		for (k, v) in &mut resolve.nodes {
			if ! used.contains(k) {
				for nd in v {
					nd.dep_kinds = (nd.dep_kinds & ! Dependency::MASK_TARGET) | Dependency::FLAG_TARGET_CFG;
				}
			}
		}

		// Lastly, mark all direct dependencies of workspace members as being
		// directly required.
		for id in workspace_members {
			if let Some(v) = resolve.nodes.get_mut(id) {
				for nd in v {
					nd.dep_kinds |= Dependency::FLAG_DIRECT;
				}
			}
		}

		// Done!
		(packages, resolve)
	}
}



#[derive(Debug, Deserialize)]
/// # Package.
pub(super) struct RawPackage<'a> {
	/// # ID.
	pub(super) id: &'a str,

	/// # Name.
	pub(super) name: PackageName,

	/// # Version.
	pub(super) version: Version,

	#[serde(borrow)]
	/// # Package Description.
	description: Option<&'a RawValue>,

	#[serde(default)]
	#[serde(borrow)]
	/// # License.
	license: Option<&'a RawValue>,

	#[serde(default)]
	#[serde(borrow)]
	/// # Author(s).
	authors: Option<&'a RawValue>,

	#[serde(default)]
	#[serde(borrow)]
	/// # Repository URL.
	repository: Option<&'a RawValue>,

	#[serde(default)]
	#[serde(borrow)]
	/// # Has Features?
	///
	/// We'll only ever end up using this for the primary package, so there's
	/// no point getting specific about types and whatnot at this stage.
	features: Option<&'a RawValue>,

	#[serde(default)]
	#[serde(borrow)]
	/// # Metadata.
	///
	/// We'll only ever end up using this for the primary package, so there's
	/// no point getting specific about types and whatnot at this stage.
	metadata: Option<&'a RawValue>,
}

impl<'a> RawPackage<'a> {
	/// # Try Into Dependency.
	fn try_into_dependency(self, context: u8) -> Result<Dependency, BashManError> {
		// Deserialize deferred fields.
		let license: Option<String> = match self.license {
			Some(raw) => util::deserialize_license(raw)
				.map_err(|e| BashManError::ParseCargoMetadata(e.to_string()))?,
			None => None,
		};
		let authors: Vec<String> = match self.authors {
			Some(raw) => util::deserialize_authors(raw)
				.map_err(|e| BashManError::ParseCargoMetadata(e.to_string()))?,
			None => Vec::new(),
		};
		let url: Option<String> = match self.repository {
			Some(raw) => <Option<Url>>::deserialize(raw)
				.map_err(|e| BashManError::ParseCargoMetadata(e.to_string()))?
				.map(String::from),
			None => None,
		};

		// Done!
		Ok(Dependency {
			name: String::from(self.name),
			version: self.version,
			license,
			authors,
			url,
			context,
		})
	}
}



#[derive(Debug, Default, Deserialize)]
/// # Raw Metadata.
///
/// This is just a simple intermediary structure; we'll only end up keeping
/// what's inside.
struct RawMetadata<'a> {
	#[serde(borrow)]
	#[serde(default)]
	/// # Bashman Metadata.
	bashman: Option<RawBashMan<'a>>,
}



#[derive(Debug, Clone, Default, Deserialize)]
/// # Raw Package Metadata (bashman).
///
/// This is what is found under "package.metadata.bashman".
struct RawBashMan<'a> {
	#[serde(rename = "name")]
	#[serde(default)]
	#[serde(deserialize_with = "util::deserialize_nonempty_opt_str_normalized")]
	/// # Package Nice Name.
	nice_name: Option<String>,

	#[serde(rename = "bash-dir")]
	#[serde(default)]
	#[serde(deserialize_with = "util::deserialize_nonempty_opt_str")]
	/// # Directory For Bash Completions.
	dir_bash: Option<String>,

	#[serde(rename = "man-dir")]
	#[serde(default)]
	#[serde(deserialize_with = "util::deserialize_nonempty_opt_str")]
	/// # Directory for MAN pages.
	dir_man: Option<String>,

	#[serde(rename = "credits-dir")]
	#[serde(default)]
	#[serde(deserialize_with = "util::deserialize_nonempty_opt_str")]
	/// # Directory for Credits.
	dir_credits: Option<String>,

	#[serde(default)]
	/// # Subcommands.
	subcommands: Vec<RawSubCmd>,

	#[serde(rename = "switches")]
	#[serde(default)]
	#[serde(borrow)]
	/// # Switches.
	flags: Vec<RawSwitch<'a>>,

	#[serde(default)]
	/// # Options.
	options: Vec<RawOption<'a>>,

	#[serde(rename = "arguments")]
	#[serde(default)]
	/// # Arguments.
	args: Vec<RawArg<'a>>,

	#[serde(default)]
	/// # Sections.
	sections: Vec<RawSection>,

	#[serde(default)]
	/// # Credits.
	credits: Vec<RawCredits>,
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Subcommand.
///
/// This is what is found under "package.metadata.bashman.subcommands".
struct RawSubCmd {
	#[serde(default)]
	#[serde(deserialize_with = "util::deserialize_nonempty_opt_str_normalized")]
	/// # Nice Name.
	name: Option<String>,

	/// # (Sub)command.
	cmd: KeyWord,

	#[serde(deserialize_with = "util::deserialize_nonempty_str_normalized")]
	/// # Description.
	description: String,
}

impl RawSubCmd {
	/// # From Raw.
	fn into_subcommand(self, version: String, parent: Option<(String, KeyWord)>)
	-> Subcommand {
		Subcommand {
			nice_name: self.name,
			name: self.cmd,
			description: self.description,
			version,
			parent,
			data: ManifestData::default(),
		}
	}
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Switch.
///
/// This is what is found under "package.metadata.bashman.switches".
struct RawSwitch<'a> {
	#[serde(default)]
	/// # Short Key.
	short: Option<KeyWord>,

	#[serde(default)]
	/// # Long Key.
	long: Option<KeyWord>,

	#[serde(deserialize_with = "util::deserialize_nonempty_str_normalized")]
	/// # Description.
	description: String,

	#[serde(default)]
	/// # Allow Duplicates.
	duplicate: bool,

	#[serde(borrow)]
	#[serde(default)]
	/// # Applicable (Sub)commands.
	subcommands: BTreeSet<&'a str>,
}



#[derive(Debug, Clone, Deserialize)]
/// Raw Option.
///
/// This is what is found under "package.metadata.bashman.options".
struct RawOption<'a> {
	#[serde(default)]
	/// # Short Key.
	short: Option<KeyWord>,

	#[serde(default)]
	/// # Long Key.
	long: Option<KeyWord>,

	#[serde(deserialize_with = "util::deserialize_nonempty_str_normalized")]
	/// # Description.
	description: String,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_label")]
	/// # Value Label.
	label: Option<String>,

	#[serde(default)]
	/// # Value is Path?
	path: bool,

	#[serde(default)]
	/// # Allow Duplicates.
	duplicate: bool,

	#[serde(borrow)]
	#[serde(default)]
	/// # Applicable (Sub)commands.
	subcommands: BTreeSet<&'a str>,
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Argument.
///
/// This is what is found under "package.metadata.bashman.arguments".
struct RawArg<'a> {
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_label")]
	/// # Value Label.
	label: Option<String>,

	#[serde(deserialize_with = "util::deserialize_nonempty_str_normalized")]
	/// # Description.
	description: String,

	#[serde(borrow)]
	#[serde(default)]
	/// # Applicable (Sub)commands.
	subcommands: BTreeSet<&'a str>,
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Section.
///
/// This is what is found under "package.metadata.bashman.sections".
struct RawSection {
	#[serde(deserialize_with = "deserialize_section_name")]
	/// # Section Name.
	name: String,

	#[serde(default)]
	/// # Indent?
	inside: bool,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_lines")]
	/// # Text Lines.
	lines: Vec<String>,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_items")]
	/// # Text Bullets.
	items: Vec<[String; 2]>
}

impl From<RawSection> for super::Section {
	#[inline]
	fn from(raw: RawSection) -> Self {
		Self {
			name: raw.name,
			inside: raw.inside,
			lines: if raw.lines.is_empty() { String::new() } else { raw.lines.join("\n.RE\n") },
			items: raw.items,
		}
	}
}



#[derive(Debug, Clone, Deserialize)]
/// # Raw Credits.
///
/// This is what is found under "package.metadata.bashman.credits".
struct RawCredits {
	/// # Name.
	name: PackageName,

	/// # Version.
	version: Version,

	#[serde(default)]
	#[serde(deserialize_with = "util::deserialize_license")]
	/// # License.
	license: Option<String>,

	#[serde(default)]
	#[serde(deserialize_with = "util::deserialize_authors")]
	/// # Author(s).
	authors: Vec<String>,

	#[serde(default)]
	/// # Repository URL.
	repository: Option<Url>,

	#[serde(default)]
	/// # Optional?
	optional: bool,
}

impl From<RawCredits> for Dependency {
	#[inline]
	fn from(src: RawCredits) -> Self {
		Self {
			name: String::from(src.name),
			version: src.version,
			license: src.license,
			authors: src.authors,
			url: src.repository.map(String::from),
			context:
				if src.optional { Self::FLAG_DIRECT | Self::FLAG_OPTIONAL }
				else { Self::FLAG_DIRECT },
		}
	}
}



#[derive(Debug, Deserialize)]
/// # Resolved Nodes.
struct RawResolve<'a> {
	#[serde(borrow)]
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_nodes")]
	/// # Nodes.
	nodes: HashMap<&'a str, Vec<RawNodeDep<'a>>>,

	/// # Root Package ID.
	root: &'a str,
}

impl<'a> RawResolve<'a> {
	/// # Cumulative Context Flags.
	///
	/// Flags are calculated per parent/child during deserialization; this
	/// method adds up the flags for each child — a given package might be an
	/// optional dependency of one crate but a required one of another —
	/// returning an orderly lookup map of the results.
	fn flags(&self, targeted: bool) -> HashMap<&str, u8> {
		let mut out = HashMap::<&str, u8>::with_capacity(self.nodes.len());
		for RawNodeDep { id, dep_kinds } in self.nodes.values().flat_map(|n| n.iter().copied()) {
			match out.entry(id) {
				Entry::Occupied(mut e) => { *e.get_mut() |= dep_kinds; },
				Entry::Vacant(e) => { e.insert(dep_kinds); },
			}
		}

		// If we're targeting specifically, unset the specific target bit.
		if targeted {
			for flag in out.values_mut() { *flag &= ! Dependency::FLAG_TARGET_CFG; }
		}

		out
	}
}



#[derive(Debug, Deserialize)]
/// # Node.
struct RawNode<'a> {
	/// # ID.
	id: &'a str,

	#[serde(borrow)]
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_deps")]
	/// # Dependent Nodes.
	deps: Vec<RawNodeDep<'a>>,
}



#[derive(Debug, Clone, Copy, Deserialize)]
/// # Node Dependency.
struct RawNodeDep<'a> {
	#[serde(rename = "pkg")]
	/// # ID.
	id: &'a str,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_dep_kinds")]
	/// # Dependency Contexts.
	///
	/// This is an unruly vector map in the raw data, but since we ultimately
	/// only care about the sum of states — of which there are few — we can
	/// more succinctly represent this as a tiny bitflag.
	dep_kinds: u8,
}



#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default)]
/// # Node Dependency Context.
///
/// This struct is a simplified abstraction over the `dep_kind` maps found in
/// the raw data.
///
/// The `kind` field is used to differentiate between `dependency`,
/// `dev-dependency`, and `build-dependency` manifest entries.
///
/// The `target` field holds `cfg`-specific rules, if any, but since we don't
/// care about the particulars — just whether or not there are any — our
/// representation is just an always/sometimes/never trit.
struct RawNodeDepKind {
	/// # Where (Build, Dev, or Runtime).
	kind: NodeDepKind,

	/// # Who/When (Target Conditions).
	target: NodeDepTarget,
}

impl RawNodeDepKind {
	/// # As `Dependency` Flag.
	///
	/// If either the kind is "dev" or the target unsatisfiable, zero will be
	/// returned. Otherwise `USED | TARGET_ANY` or `USED | TARGET_CFG`
	/// depending on the target.
	///
	/// Note that a fourth `OPTIONAL` flag comes into play later on, but isn't
	/// knowable at this stage.
	const fn as_flag(self) -> u8 {
		if matches!(self.kind, NodeDepKind::Dev) || matches!(self.target, NodeDepTarget::None) { 0 }
		else { (self.kind as u8) | (self.target as u8) }
	}
}



#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
/// # Node Dependency Context: Kind.
///
/// This trit differentiates between the `dependencies`, `build-dependencies`,
/// and `dev-dependencies`.
enum NodeDepKind {
	/// # Dev Dependency.
	Dev = 0_u8,

	#[default]
	/// # Normal Runtime Usage.
	Normal = Dependency::FLAG_CTX_NORMAL,

	/// # Build Dependency.
	Build = Dependency::FLAG_CTX_BUILD,
}

impl<'de> Deserialize<'de> for NodeDepKind {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where D: de::Deserializer<'de> {
		match <&'de RawValue>::deserialize(deserializer).map(RawValue::get) {
			Ok(r#""build""#) => Ok(Self::Build),
			Ok(r#""dev""#) => Ok(Self::Dev),
			_ => Ok(Self::Normal),
		}
	}
}



#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
/// # Node Dependency Context: Target.
///
/// The raw JSON representation includes the actual `cfg` rule, but we only
/// want to know whether or not there are any such rules, so can get away with
/// a trit akin to always/sometimes/never.
enum NodeDepTarget {
	/// # For NOBODY.
	None = 0,

	#[default]
	/// # For Any Target.
	Any = Dependency::FLAG_TARGET_ANY,

	/// # For Some Targets.
	Cfg = Dependency::FLAG_TARGET_CFG,
}

impl<'de> Deserialize<'de> for NodeDepTarget {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where D: de::Deserializer<'de> {
		match <&'de RawValue>::deserialize(deserializer).map(RawValue::get) {
			// Never applies.
			Ok(r#""cfg(any())""#) => Ok(Self::None),
			// Always applies.
			Ok(r#""cfg(all())""# | "null") | Err(_) => Ok(Self::Any),
			// Assume anything else is an actual rule.
			Ok(_) => Ok(Self::Cfg),
		}
	}
}



/// # Add Subcommand Flag.
fn add_subcommand_flag(subs: &mut BTreeMap<String, Subcommand>, key: &str, flag: Flag)
-> Result<(), BashManError> {
	subs.get_mut(key)
		.ok_or_else(|| BashManError::UnknownCommand(key.to_owned()))?
		.data
		.flags
		.insert(flag);
	Ok(())
}

/// # Add Subcommand Option Flag.
fn add_subcommand_option(
	subs: &mut BTreeMap<String, Subcommand>,
	key: &str,
	flag: OptionFlag,
) -> Result<(), BashManError> {
	subs.get_mut(key)
		.ok_or_else(|| BashManError::UnknownCommand(key.to_owned()))?
		.data
		.options
		.insert(flag);
	Ok(())
}

/// # Add Subcommand Trailing Arg.
fn add_subcommand_arg(
	subs: &mut BTreeMap<String, Subcommand>,
	key: &str,
	flag: TrailingArg,
) -> Result<(), BashManError> {
	let res = subs.get_mut(key)
		.ok_or_else(|| BashManError::UnknownCommand(key.to_owned()))?
		.data
		.args
		.replace(flag)
		.is_none();

	if res { Ok(()) }
	else { Err(BashManError::MultipleArgs(key.to_owned())) }
}

/// # Deserialize: Bashman Metadata.
fn deserialize_bashman<'a>(raw: &'a RawValue) -> Result<Option<RawBashMan<'a>>, BashManError> {
	let res = <Option<RawMetadata<'a>>>::deserialize(raw)
		.map_err(|e| BashManError::ParseCargoMetadata(e.to_string()))?;

	if let Some(mut bashman) = res.and_then(|RawMetadata { bashman }| bashman) {
		// Prune flags that are missing keys.
		bashman.flags.retain(|s| s.short.is_some() || s.long.is_some());
		bashman.options.retain(|s| s.short.is_some() || s.long.is_some());

		// Prune sections that are missing text.
		bashman.sections.retain(|s| ! s.lines.is_empty() || ! s.items.is_empty());

		// Populate empty subcommand lists with an empty string, which is what
		// we use for top-level stuff.
		let iter = bashman.flags.iter_mut().map(|s| &mut s.subcommands)
			.chain(bashman.options.iter_mut().map(|s| &mut s.subcommands))
			.chain(bashman.args.iter_mut().map(|s| &mut s.subcommands));
		for v in iter {
			if v.is_empty() { v.insert(""); }
		}

		// Check for duplicate subcommands.
		let mut subs = BTreeMap::<&str, BTreeSet<&KeyWord>>::new();
		subs.insert("", BTreeSet::new());
		for e in &bashman.subcommands {
			if subs.insert(e.cmd.as_str(), BTreeSet::new()).is_some() {
				return Err(BashManError::DuplicateKeyWord(e.cmd.clone()));
			}
		}

		// Check for duplicate keys.
		let iter = bashman.flags.iter().map(|f| (f.short.as_ref(), f.long.as_ref(), &f.subcommands))
			.chain(bashman.options.iter().map(|f| (f.short.as_ref(), f.long.as_ref(), &f.subcommands)));
		for (short, long, flag_subs) in iter {
			for &s in flag_subs {
				let entry = subs.get_mut(s)
					.ok_or_else(|| BashManError::UnknownCommand(s.to_owned()))?;
				for key in [short, long].into_iter().flatten() {
					if ! entry.insert(key) {
						return Err(BashManError::DuplicateKeyWord(key.clone()));
					}
				}
			}
		}

		return Ok(Some(bashman));
	}

	Ok(None)
}

#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: Node Sub-Dependency Kinds.
///
/// This is natively encoded as a vector of structs, but we only care about
/// the "sum" of combinations, so can more efficiently store this as a tiny
/// bitflag.
///
/// Note that zero-value dependency references will be subsequently pruned.
fn deserialize_dep_kinds<'de, D>(deserializer: D) -> Result<u8, D::Error>
where D: Deserializer<'de> {
	Ok(<Vec<RawNodeDepKind>>::deserialize(deserializer).map_or(
		0_u8,
		|v| v.into_iter().fold(0_u8, |acc, dk| acc | dk.as_flag())
	))
}

#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: Node Dependencies.
///
/// This filters the list of node dependencies to remove irrelevant entries,
/// such as those with unsatisfiable targets or only used in dev contexts.
fn deserialize_deps<'de, D>(deserializer: D) -> Result<Vec<RawNodeDep<'de>>, D::Error>
where D: Deserializer<'de> {
	Ok(<Vec<RawNodeDep<'de>>>::deserialize(deserializer).map_or_else(
		|_| Vec::new(),
		|mut v| {
			v.retain(|nd|
				(0 != nd.dep_kinds & Dependency::MASK_CTX) &&
				(0 != nd.dep_kinds & Dependency::MASK_TARGET)
			);
			v
		}
	))
}

/// # Deserialize: Features.
///
/// We just want to know if there _are_ features; the details are irrelevant.
fn deserialize_features<'a>(raw: &'a RawValue) -> bool {
	<HashMap<Cow<'a, str>, &'a RawValue>>::deserialize(raw).map_or(
		false,
		|map| match 1_usize.cmp(&map.len()) {
			// 2+ features is always a YES.
			Ordering::Less => true,
			// A single feature is a YES so long as it isn't "default".
			Ordering::Equal => ! map.contains_key("default"),
			// Zero is a NO.
			Ordering::Greater => false,
		}
	)
}

#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: Section Items.
fn deserialize_items<'de, D>(deserializer: D) -> Result<Vec<[String; 2]>, D::Error>
where D: Deserializer<'de> {
	let mut out = Vec::<[String; 2]>::deserialize(deserializer).unwrap_or_default();
	out.retain_mut(|line| {
		util::normalize_string(&mut line[0]);
		util::normalize_string(&mut line[1]);
		! line[0].is_empty() || ! line[1].is_empty()
	});

	Ok(out)
}

#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: Optional Option/Arg Label.
///
/// This will return `None` if the string is empty.
fn deserialize_label<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where D: Deserializer<'de> {
	Ok(
		<String>::deserialize(deserializer).ok()
			.and_then(|mut x| {
				util::normalize_string(&mut x);
				if x.is_empty() { None }
				else {
					if ! x.starts_with('<') { x.insert(0, '<'); }
					if ! x.ends_with('>') { x.push('>'); }
					Some(x)
				}
			})
	)
}

#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: Section Lines.
fn deserialize_lines<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where D: Deserializer<'de> {
	let mut out = Vec::<String>::deserialize(deserializer).unwrap_or_default();
	let mut any = false;
	out.retain_mut(|line| {
		util::normalize_string(line);
		if line.is_empty() && ! any { false }
		else {
			any = true;
			true
		}
	});

	// Remove trailing empty lines.
	while out.last().filter(|v| v.is_empty()).is_some() {
		out.truncate(out.len() - 1);
	}

	Ok(out)
}

#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: Resolve Nodes.
///
/// This is natively stored as a vector of structs, but we'll be doing a lot of
/// ID-based lookups so a keyed map is a lot more efficient.
fn deserialize_nodes<'de, D>(deserializer: D)
-> Result<HashMap<&'de str, Vec<RawNodeDep<'de>>>, D::Error>
where D: Deserializer<'de> {
	Ok(<Vec<RawNode<'de>>>::deserialize(deserializer).map_or_else(
		|_| HashMap::new(),
		|v| v.into_iter().map(|RawNode { id, deps }| (id, deps)).collect(),
	))
}

/// # Deserialize: Section Name.
///
/// This will return an error if a string is present but empty.
fn deserialize_section_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where D: Deserializer<'de> {
	let tmp = <String>::deserialize(deserializer)?;
	let mut out: String = tmp.normalized_control_and_whitespace()
		.flat_map(char::to_uppercase)
		.collect();

	let last = out.chars().last()
		.ok_or_else(|| serde::de::Error::custom("value cannot be empty"))?;
	if ! last.is_ascii_punctuation() { out.push(':'); }
	Ok(out)
}



#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn t_deserialize_raw() {
		let target = TargetTriple::try_from("x86_64-unknown-linux-gnu".to_owned()).ok();
		assert!(target.is_some(), "Target failed.");

		let (main, deps) = fetch_test(target).expect("Fetch test failed.");

		// Confirm the dependency count.
		assert_eq!(deps.len(), 67);

		// We have 2 of 3 directories defined.
		assert_eq!(main.dir_bash.as_deref(), Some("./"));
		assert_eq!(main.dir_man.as_deref(), Some("./"));
		assert!(main.dir_credits.is_none());

		// Only one command.
		assert_eq!(main.subcommands.len(), 1);
		assert_eq!(main.subcommands[0].nice_name.as_deref(), Some("Cargo BashMan"));
		assert_eq!(main.subcommands[0].name.as_str(), "cargo-bashman");
		assert_eq!(
			main.subcommands[0].description,
			"A Cargo plugin to generate bash completions, man pages, and/or crate credits.",
		);
		assert_eq!(main.subcommands[0].version, "0.6.3");
		assert!(main.subcommands[0].parent.is_none());

		// Six flags, two options, no args or sections.
		assert_eq!(main.subcommands[0].data.flags.len(), 6);
		assert_eq!(main.subcommands[0].data.options.len(), 2);
		assert!(main.subcommands[0].data.args.is_none());
		assert!(main.subcommands[0].data.sections.is_empty());
	}

	#[test]
	fn t_raw_node_dep_kind() {
		// No values.
		let kind: RawNodeDepKind = serde_json::from_str(r#"{"kind": null, "target": null}"#)
			.expect("Failed to deserialize RawNodeDepKind");
		assert!(matches!(kind.kind, NodeDepKind::Normal));
		assert!(matches!(kind.target, NodeDepTarget::Any));
		assert_eq!(kind.as_flag(), Dependency::FLAG_CTX_NORMAL | Dependency::FLAG_TARGET_ANY);

		// Build.
		let kind: RawNodeDepKind = serde_json::from_str(r#"{"kind": "build", "target": null}"#)
			.expect("Failed to deserialize RawNodeDepKind");
		assert!(matches!(kind.kind, NodeDepKind::Build));
		assert!(matches!(kind.target, NodeDepTarget::Any));
		assert_eq!(kind.as_flag(), Dependency::FLAG_CTX_BUILD | Dependency::FLAG_TARGET_ANY);

		// Build and Target.
		let kind: RawNodeDepKind = serde_json::from_str(r#"{"kind": "build", "target": "cfg(unix)"}"#)
			.expect("Failed to deserialize RawNodeDepKind");
		assert!(matches!(kind.kind, NodeDepKind::Build));
		assert!(matches!(kind.target, NodeDepTarget::Cfg));
		assert_eq!(kind.as_flag(), Dependency::FLAG_CTX_BUILD | Dependency::FLAG_TARGET_CFG);

		// Target.
		let kind: RawNodeDepKind = serde_json::from_str(r#"{"kind": null, "target": "cfg(target_os = \"hermit\")"}"#)
			.expect("Failed to deserialize RawNodeDepKind");
		assert!(matches!(kind.kind, NodeDepKind::Normal));
		assert!(matches!(kind.target, NodeDepTarget::Cfg));
		assert_eq!(kind.as_flag(), Dependency::FLAG_CTX_NORMAL | Dependency::FLAG_TARGET_CFG);

		// Bullshit target (should be treated as dev).
		let kind: RawNodeDepKind = serde_json::from_str(r#"{"kind": null, "target": "cfg(any())"}"#)
			.expect("Failed to deserialize RawNodeDepKind");
		assert!(matches!(kind.kind, NodeDepKind::Normal));
		assert!(matches!(kind.target, NodeDepTarget::None));
		assert_eq!(kind.as_flag(), 0);

		// Dev.
		let kind: RawNodeDepKind = serde_json::from_str(r#"{"kind": "dev", "target": null}"#)
			.expect("Failed to deserialize RawNodeDepKind");
		assert!(matches!(kind.kind, NodeDepKind::Dev));
		assert!(matches!(kind.target, NodeDepTarget::Any));
		assert_eq!(kind.as_flag(), 0);

		// Dev and target (should be treated as dev).
		let kind: RawNodeDepKind = serde_json::from_str(r#"{"kind": "dev", "target": "cfg(target_os = \"wasi\")"}"#)
			.expect("Failed to deserialize RawNodeDepKind");
		assert!(matches!(kind.kind, NodeDepKind::Dev));
		assert!(matches!(kind.target, NodeDepTarget::Cfg));
		assert_eq!(kind.as_flag(), 0);
	}

	#[test]
	fn t_deserialize_features() {
		let raw = RawValue::from_string(r#"{}"#.to_owned()).unwrap();
		assert!(! deserialize_features(&raw));

		let raw = RawValue::from_string(r#"{"default": ["foo"]}"#.to_owned()).unwrap();
		assert!(! deserialize_features(&raw));

		let raw = RawValue::from_string(r#"{"utc2k": null}"#.to_owned()).unwrap();
		assert!(deserialize_features(&raw));

		let raw = RawValue::from_string(r#"{"default": ["foo"], "bar": null}"#.to_owned()).unwrap();
		assert!(deserialize_features(&raw));
	}
}
