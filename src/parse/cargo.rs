/*!
# Cargo BashMan: Cargo Metadata Parsing.
*/

use crate::{
	BashManError,
	Dependency,
	KeyWord,
	PackageName,
	TargetTriple,
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
	util::{
		self,
		CargoMetadata,
	},
};
use trimothy::NormalizeWhitespace;
use url::Url;



/// # Intermediary Manifest.
pub(super) struct RawManifest {
	/// # Main Package.
	pub(super) main: RawMainPackage,

	/// # Dependencies.
	pub(super) deps: BTreeSet<Dependency>,
}

impl RawManifest {
	/// # New.
	pub(super) fn new(src: &Path, target: Option<TargetTriple>)
	-> Result<Self, BashManError> {
		let cargo = CargoMetadata::new(src, target).with_features(false);

		// Query without features first.
		let raw1 = cargo.exec()?;
		let Raw { packages, resolve } = serde_json::from_slice(&raw1)
			.map_err(|e| BashManError::ParseCargoMetadata(e.to_string()))?;

		// Build the dependency list (and find the main package).
		let flags = resolve.flags(target.is_some());
		let mut main = None;
		let mut deps: BTreeSet<Dependency> = packages.into_iter()
			.filter_map(|p|
				// Split out the main crate.
				if p.id == resolve.root {
					main.replace(p);
					None
				}
				// Convert and keep used dependencies.
				else if resolve.nodes.contains_key(p.id) {
					Some(Dependency {
						name: String::from(p.name),
						version: p.version,
						license: if p.license.is_empty() { None } else { Some(p.license) },
						authors: p.authors,
						url: p.repository.map(String::from),
						context: flags.get(p.id).copied().unwrap_or(0),
					})
				}
				// Nope.
				else { None }
			)
			.collect();

		// We should have a main package by now.
		let main = main.ok_or_else(|| BashManError::ParseCargoMetadata(
			"unable to determine root package".to_owned()
		))?;

		// If this crate has features, repeat the process to figure out if
		// there are any additional optional dependencies.
		if main.features {
			if let Ok(raw2) = cargo.with_features(true).exec() {
				if let Ok(Raw { packages, resolve }) = serde_json::from_slice(&raw2) {
					// Build the dependency list (and find the main package).
					let flags = resolve.flags(target.is_some());
					for p in packages {
						if p.id != main.id && resolve.nodes.contains_key(p.id) {
							// Insert it if unique; discard it if not!
							deps.insert(Dependency {
								name: String::from(p.name),
								version: p.version,
								license: if p.license.is_empty() { None } else { Some(p.license) },
								authors: p.authors,
								url: p.repository.map(String::from),
								context: flags.get(p.id).copied().unwrap_or(0) | Dependency::FLAG_OPTIONAL,
							});
						}
					}
				}
			}
		}

		// Finish deserializing the main package.
		let main = RawMainPackage::try_from(main)?;
		Ok(Self { main, deps })
	}
}



#[derive(Debug, Deserialize)]
/// # Main Package.
///
/// This is almost the same as `RawPackage`, but includes the the `bashman`
/// metadata, if any.
pub(super) struct RawMainPackage {
	/// # Name.
	pub(super) name: PackageName,

	/// # Version.
	pub(super) version: Version,

	/// # Package Description.
	pub(super) description: String,

	/// # Metadata.
	pub(super) metadata: RawBashMan,
}

impl<'a> TryFrom<RawPackage<'a>> for RawMainPackage {
	type Error = BashManError;
	fn try_from(raw: RawPackage<'a>) -> Result<Self, Self::Error> {
		// Destructure the raw, discarding all the fields we don't care about.
		let RawPackage { name, version, description, metadata, .. } = raw;

		// Description is mandatory for this one!
		let description = description.ok_or_else(|| BashManError::ParseCargoMetadata(
			"missing description for main package".to_owned()
		))?;

		// If we have metadata, deserialize it now!
		let mut metadata: Option<RawBashMan> = match metadata {
			Some(m) => serde_json::from_str::<Option<RawMetadata>>(m.get())
				.map_err(|e| BashManError::ParseCargoMetadata(e.to_string()))?
				.and_then(|m| m.bashman),
			None => None,
		};

		// Perform some sanity checks on the data, if any.
		if let Some(meta) = &mut metadata {
			// Prune flags that are missing keys.
			meta.flags.retain(|s| s.short.is_some() || s.long.is_some());
			meta.options.retain(|s| s.short.is_some() || s.long.is_some());

			// Prune sections that are missing text.
			meta.sections.retain(|s| ! s.lines.is_empty() || ! s.items.is_empty());

			// Populate empty subcommand lists with an empty string, which is what
			// we use for top-level stuff.
			let iter = meta.flags.iter_mut().map(|s| &mut s.subcommands)
				.chain(meta.options.iter_mut().map(|s| &mut s.subcommands))
				.chain(meta.args.iter_mut().map(|s| &mut s.subcommands));
			for v in iter {
				if v.is_empty() { v.insert(String::new()); }
			}

			// Check for duplicate subcommands.
			let mut subs = BTreeMap::<&str, BTreeSet<&KeyWord>>::new();
			subs.insert("", BTreeSet::new());
			for e in &meta.subcommands {
				if subs.insert(e.cmd.as_str(), BTreeSet::new()).is_some() {
					return Err(BashManError::DuplicateKeyWord(e.cmd.clone()));
				}
			}

			// Check for duplicate keys.
			let iter = meta.flags.iter().map(|f| (f.short.as_ref(), f.long.as_ref(), &f.subcommands))
				.chain(meta.options.iter().map(|f| (f.short.as_ref(), f.long.as_ref(), &f.subcommands)));
			for (short, long, flag_subs) in iter {
				for s in flag_subs {
					let entry = subs.get_mut(s.as_str())
						.ok_or_else(|| BashManError::UnknownCommand(s.clone()))?;
					for key in [short, long].into_iter().flatten() {
						if ! entry.insert(key) {
							return Err(BashManError::DuplicateKeyWord(key.clone()))?;
						}
					}
				}
			}
		}

		Ok(Self {
			name,
			version,
			description,
			metadata: metadata.unwrap_or_default(),
		})
	}
}

#[derive(Debug, Default, Deserialize)]
/// # Raw Metadata.
struct RawMetadata {
	#[serde(default)]
	/// # Bashman Metadata.
	bashman: Option<RawBashMan>,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// # Raw Package Metadata (bashman).
///
/// This is what is found under "package.metadata.bashman".
pub(super) struct RawBashMan {
	#[serde(rename = "name")]
	#[serde(default)]
	#[serde(deserialize_with = "util::deserialize_nonempty_opt_str_normalized")]
	/// # Package Nice Name.
	pub(super) nice_name: Option<String>,

	#[serde(rename = "bash-dir")]
	#[serde(default)]
	#[serde(deserialize_with = "util::deserialize_nonempty_opt_str")]
	/// # Directory For Bash Completions.
	pub(super) dir_bash: Option<String>,

	#[serde(rename = "man-dir")]
	#[serde(default)]
	#[serde(deserialize_with = "util::deserialize_nonempty_opt_str")]
	/// # Directory for MAN pages.
	pub(super) dir_man: Option<String>,

	#[serde(rename = "credits-dir")]
	#[serde(default)]
	#[serde(deserialize_with = "util::deserialize_nonempty_opt_str")]
	/// # Directory for Credits.
	pub(super) dir_credits: Option<String>,

	#[serde(default)]
	/// # Subcommands.
	pub(super) subcommands: Vec<RawSubCmd>,

	#[serde(rename = "switches")]
	#[serde(default)]
	/// # Switches.
	pub(super) flags: Vec<RawSwitch>,

	#[serde(default)]
	/// # Options.
	pub(super) options: Vec<RawOption>,

	#[serde(rename = "arguments")]
	#[serde(default)]
	/// # Arguments.
	pub(super) args: Vec<RawArg>,

	#[serde(default)]
	/// # Sections.
	pub(super) sections: Vec<RawSection>,

	#[serde(default)]
	/// # Credits.
	pub(super) credits: Vec<RawCredits>,
}

#[derive(Debug, Clone, Deserialize)]
/// # Raw Subcommand.
///
/// This is what is found under "package.metadata.bashman.subcommands".
pub(super) struct RawSubCmd {
	#[serde(default)]
	#[serde(deserialize_with = "util::deserialize_nonempty_opt_str_normalized")]
	/// # Nice Name.
	pub(super) name: Option<String>,

	/// # (Sub)command.
	pub(super) cmd: KeyWord,

	#[serde(deserialize_with = "util::deserialize_nonempty_str_normalized")]
	/// # Description.
	pub(super) description: String,
}

#[derive(Debug, Clone, Deserialize)]
/// # Raw Switch.
///
/// This is what is found under "package.metadata.bashman.switches".
pub(super) struct RawSwitch {
	#[serde(default)]
	/// # Short Key.
	pub(super) short: Option<KeyWord>,

	#[serde(default)]
	/// # Long Key.
	pub(super) long: Option<KeyWord>,

	#[serde(deserialize_with = "util::deserialize_nonempty_str_normalized")]
	/// # Description.
	pub(super) description: String,

	#[serde(default)]
	/// # Allow Duplicates.
	pub(super) duplicate: bool,

	#[serde(default)]
	/// # Applicable (Sub)commands.
	pub(super) subcommands: BTreeSet<String>,
}

#[derive(Debug, Clone, Deserialize)]
/// Raw Option.
///
/// This is what is found under "package.metadata.bashman.options".
pub(super) struct RawOption {
	#[serde(default)]
	/// # Short Key.
	pub(super) short: Option<KeyWord>,

	#[serde(default)]
	/// # Long Key.
	pub(super) long: Option<KeyWord>,

	#[serde(deserialize_with = "util::deserialize_nonempty_str_normalized")]
	/// # Description.
	pub(super) description: String,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_label")]
	/// # Value Label.
	pub(super) label: Option<String>,

	#[serde(default)]
	/// # Value is Path?
	pub(super) path: bool,

	#[serde(default)]
	/// # Allow Duplicates.
	pub(super) duplicate: bool,

	#[serde(default)]
	/// # Applicable (Sub)commands.
	pub(super) subcommands: BTreeSet<String>,
}

#[derive(Debug, Clone, Deserialize)]
/// # Raw Argument.
///
/// This is what is found under "package.metadata.bashman.arguments".
pub(super) struct RawArg {
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_label")]
	/// # Value Label.
	pub(super) label: Option<String>,

	#[serde(deserialize_with = "util::deserialize_nonempty_str_normalized")]
	/// # Description.
	pub(super) description: String,

	#[serde(default)]
	/// # Applicable (Sub)commands.
	pub(super) subcommands: BTreeSet<String>,
}

#[derive(Debug, Clone, Deserialize)]
/// # Raw Section.
///
/// This is what is found under "package.metadata.bashman.sections".
pub(super) struct RawSection {
	#[serde(deserialize_with = "deserialize_section_name")]
	/// # Section Name.
	pub(super) name: String,

	#[serde(default)]
	/// # Indent?
	pub(super) inside: bool,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_lines")]
	/// # Text Lines.
	pub(super) lines: Vec<String>,

	#[serde(default)]
	#[serde(deserialize_with = "deserialize_items")]
	/// # Text Bullets.
	pub(super) items: Vec<[String; 2]>
}

#[derive(Debug, Clone, Deserialize)]
/// # Raw Credits.
///
/// This is what is found under "package.metadata.bashman.credits".
pub(super) struct RawCredits {
	/// # Name.
	name: PackageName,

	/// # Version.
	version: Version,

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
	/// # Optional?
	optional: bool,
}

impl From<RawCredits> for Dependency {
	#[inline]
	fn from(src: RawCredits) -> Self {
		Self {
			name: String::from(src.name),
			version: src.version,
			license: if src.license.is_empty() { None } else { Some(src.license) },
			authors: src.authors,
			url: src.repository.map(String::from),
			context: if src.optional { Self::FLAG_OPTIONAL } else { 0 },
		}
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
	#[serde(deserialize_with = "deserialize_resolve")]
	/// # Resolved Nodes.
	resolve: RawResolve<'a>,
}

#[derive(Debug, Deserialize)]
/// # Package.
struct RawPackage<'a> {
	/// # ID.
	id: &'a str,

	/// # Name.
	name: PackageName,

	/// # Version.
	version: Version,

	#[serde(deserialize_with = "util::deserialize_nonempty_opt_str_normalized")]
	/// # Package Description.
	description: Option<String>,

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
	///
	/// This field is a map in the raw data, but since we only want to know
	/// whether or not there _are_ features, a yay/nay is sufficient here.
	features: bool,

	#[serde(default)]
	#[serde(borrow)]
	/// # Metadata.
	///
	/// We'll only ever end up using this for the primary package, so there's
	/// no point getting specific about types and whatnot at this stage.
	metadata: Option<&'a RawValue>,
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

		// For this, we only want to loop the children.
		for RawNodeDep { id, dep_kinds } in self.nodes.values().flat_map(|n| n.iter().copied()) {
			match out.entry(id) {
				Entry::Occupied(mut e) => { *e.get_mut() |= dep_kinds; },
				Entry::Vacant(e) => { e.insert(dep_kinds); },
			}
		}

		if targeted {
			for flag in out.values_mut() { *flag &= ! RawNodeDepKind::TARGET_CFG; }
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
	///
	/// All three are represented by the enum for clarity, but `Dev` is the
	/// only actionable variant
	kind: NodeDepKind,

	/// # Who/When (Target Conditions).
	target: NodeDepTarget,
}

impl RawNodeDepKind {
	/// # Referenced.
	///
	/// A dependency referenced in release builds, i.e. not "dev".
	const USED: u8 =       0b1000_0000;

	/// # All Targets.
	const TARGET_ANY: u8 = Dependency::FLAG_TARGET_ANY;

	/// # Select Targets.
	const TARGET_CFG: u8 = Dependency::FLAG_TARGET_CFG;

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
		else { Self::USED | (self.target as u8) }
	}
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
/// # Node Dependency Context: Kind.
///
/// This trit differentiates between the types of dependency declarations. In
/// practice `Normal` and `Build` are treated the same and kept separate merely
/// for readability. (Hopefully the compiler will recognize that and treat this
/// more like a bool.)
enum NodeDepKind {
	#[default]
	/// # Normal Runtime Usage.
	Normal,

	/// # Build Dependency.
	Build,

	/// # Dev Dependency.
	Dev,
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
	Any = RawNodeDepKind::TARGET_ANY,

	/// # For Some Targets.
	Cfg = RawNodeDepKind::TARGET_CFG,
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
			v.retain(|nd| RawNodeDepKind::USED == nd.dep_kinds & RawNodeDepKind::USED);
			v
		}
	))
}

#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: Features.
///
/// We just want to know if there _are_ features; the details are irrelevant.
fn deserialize_features<'de, D>(deserializer: D) -> Result<bool, D::Error>
where D: Deserializer<'de> {
	Ok(<HashMap<Cow<'de, str>, &'de RawValue>>::deserialize(deserializer).map_or(
		false,
		|map| match 1_usize.cmp(&map.len()) {
			// 2+ features is always a YES.
			Ordering::Less => true,
			// A single feature is a YES so long as it isn't "default".
			Ordering::Equal => ! map.contains_key("default"),
			// Zero is a NO.
			Ordering::Greater => false,
		}
	))
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

/// # Deserialize: Resolve.
///
/// This rebuilds the node list so that only _used_ dependencies are included.
fn deserialize_resolve<'de, D>(deserializer: D)
-> Result<RawResolve<'de>, D::Error>
where D: Deserializer<'de> {
	let mut resolve = <RawResolve<'de>>::deserialize(deserializer)?;

	// To figure out which dependencies are actually used, we need to traverse
	// the root node's child dependencies, then traverse each of their
	// dependencies, and so on. A simple push/pop queue will suffice!
	let mut used: HashSet<&str> = HashSet::with_capacity(resolve.nodes.len());
	let mut queue = vec![resolve.root];
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

	// And with that, let's prune the unused nodes.
	resolve.nodes.retain(|k, _| used.contains(k));

	// Done!
	Ok(resolve)
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
	fn t_raw_node_dep_kind() {
		// No values.
		let kind: RawNodeDepKind = serde_json::from_str(r#"{"kind": null, "target": null}"#)
			.expect("Failed to deserialize RawNodeDepKind");
		assert!(matches!(kind.kind, NodeDepKind::Normal));
		assert!(matches!(kind.target, NodeDepTarget::Any));
		assert_eq!(kind.as_flag(), RawNodeDepKind::USED | RawNodeDepKind::TARGET_ANY);

		// Build.
		let kind: RawNodeDepKind = serde_json::from_str(r#"{"kind": "build", "target": null}"#)
			.expect("Failed to deserialize RawNodeDepKind");
		assert!(matches!(kind.kind, NodeDepKind::Build));
		assert!(matches!(kind.target, NodeDepTarget::Any));
		assert_eq!(kind.as_flag(), RawNodeDepKind::USED | RawNodeDepKind::TARGET_ANY);

		// Build and Target.
		let kind: RawNodeDepKind = serde_json::from_str(r#"{"kind": "build", "target": "cfg(unix)"}"#)
			.expect("Failed to deserialize RawNodeDepKind");
		assert!(matches!(kind.kind, NodeDepKind::Build));
		assert!(matches!(kind.target, NodeDepTarget::Cfg));
		assert_eq!(kind.as_flag(), RawNodeDepKind::USED | RawNodeDepKind::TARGET_CFG);

		// Target.
		let kind: RawNodeDepKind = serde_json::from_str(r#"{"kind": null, "target": "cfg(target_os = \"hermit\")"}"#)
			.expect("Failed to deserialize RawNodeDepKind");
		assert!(matches!(kind.kind, NodeDepKind::Normal));
		assert!(matches!(kind.target, NodeDepTarget::Cfg));
		assert_eq!(kind.as_flag(), RawNodeDepKind::USED | RawNodeDepKind::TARGET_CFG);

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
		let raw = r#"{}"#;
		assert!(matches!(
			deserialize_features(&mut serde_json::de::Deserializer::from_str(raw)),
			Ok(false),
		));

		let raw = r#"{"default": ["foo"]}"#;
		assert!(matches!(
			deserialize_features(&mut serde_json::de::Deserializer::from_str(raw)),
			Ok(false),
		));

		let raw = r#"{"utc2k": null}"#;
		assert!(matches!(
			deserialize_features(&mut serde_json::de::Deserializer::from_str(raw)),
			Ok(true),
		));

		let raw = r#"{"default": ["foo"], "bar": null}"#;
		assert!(matches!(
			deserialize_features(&mut serde_json::de::Deserializer::from_str(raw)),
			Ok(true),
		));
	}
}
