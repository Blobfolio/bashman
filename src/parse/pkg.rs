/*!
# Cargo BashMan: Package Name.
*/

use crate::{
	BashManError,
	KeyWord,
};
use oxford_join::OxfordJoinFmt;
use serde::de;
use semver::Version;
use std::{
	cmp::Ordering,
	fmt,
};
use trimothy::TrimMut;



#[derive(Debug, Clone)]
/// # Dependency.
///
/// This holds basic package information for a dependency. It is used when
/// generating credits.
pub(crate) struct Dependency {
	/// # Name.
	pub(super) name: String,

	/// # Version.
	pub(super) version: Version,

	/// # License.
	pub(super) license: Option<String>,

	/// # Author(s).
	pub(super) authors: Vec<String>,

	/// # Repository URL.
	pub(super) url: Option<String>,

	/// # Context Flags.
	pub(super) context: u8,
}

impl Eq for Dependency {}

impl Ord for Dependency {
	#[inline]
	fn cmp(&self, other: &Self) -> Ordering {
		match self.name.cmp(&other.name) {
			Ordering::Equal => self.version.cmp(&other.version),
			cmp => cmp,
		}
	}
}

impl PartialEq for Dependency {
	#[inline]
	fn eq(&self, other: &Self) -> bool {
		self.name == other.name && self.version == other.version
	}
}

impl PartialOrd for Dependency {
	#[inline]
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl Dependency {
	/// # Feature-Specific.
	pub(super) const FLAG_OPTIONAL: u8 =   0b0000_0001;

	/// # Not Target-Specific.
	pub(super) const FLAG_TARGET_ANY: u8 = 0b0000_0010;

	/// # Target-Specific.
	pub(super) const FLAG_TARGET_CFG: u8 = 0b0000_0100;

	/// # Platform Flags.
	pub(super) const FLAG_TARGET: u8 = Self::FLAG_TARGET_ANY | Self::FLAG_TARGET_CFG;
}

impl Dependency {
	/*
	/// # Name.
	pub(crate) fn name(&self) -> &str { &self.name }

	/// # Version.
	pub(super) const fn version(&self) -> &Version { &self.version }
	*/

	/// # License.
	pub(super) fn license(&self) -> Option<&str> { self.license.as_deref() }

	/// # Author(s).
	pub(super) fn authors(&self) -> &[String] { self.authors.as_slice() }

	/// # Repository URL.
	pub(super) fn url(&self) -> Option<&str> { self.url.as_deref() }

	/// # Optional?
	pub(crate) const fn optional(&self) -> bool {
		Self::FLAG_OPTIONAL == self.context & Self::FLAG_OPTIONAL
	}

	/// # Target-Specific?
	pub(crate) const fn target_specific(&self) -> bool {
		Self::FLAG_TARGET_CFG == self.context & Self::FLAG_TARGET
	}

	/// # Conditional?
	///
	/// Returns `true` if optional or target specific.
	pub(crate) const fn conditional(&self) -> bool {
		self.optional() || self.target_specific()
	}

	/// # Context Flags as String Slice.
	///
	/// Return a textual representation of the dependency's context(s). There
	/// are only a few combinations so this is pretty easy to construct
	/// manually.
	pub(crate) const fn context(&self) -> &'static str {
		match (self.optional(), self.target_specific()) {
			(true, true) => "optional, target-specific",
			(true, false) => "optional",
			(false, true) => "target-specific",
			(false, false) => "",
		}
	}
}

impl fmt::Display for Dependency {
	/// # Write as Markdown.
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		// The name as a link.
		if let Some(url) = self.url() {
			write!(
				f,
				"| [{}]({url}) | {} | {} | {} |",
				self.name,
				self.version,
				OxfordJoinFmt::and(self.authors()),
				self.license().unwrap_or(""),
			)
		}
		// The name plain.
		else {
			write!(
				f,
				"| {} | {} | {} | {} |",
				self.name,
				self.version,
				OxfordJoinFmt::and(self.authors()),
				self.license().unwrap_or(""),
			)
		}
	}
}



#[derive(Debug, Clone)]
/// # Package Name.
///
/// This struct primarily enforces proper package-naming requirements:
/// * Must be non-empty;
/// * May only contain ASCII alphanumeric, `-`, and `_`;
/// * The first character must be alphanumreic.
///
/// It also ensures that for equality and sorting purposes, `-` and `_` are
/// treated as equivalent.
pub(crate) struct PackageName {
	/// # The Name.
	name: String,

	/// # Includes Hyphen(s)?
	hyphens: bool,
}

impl<'de> de::Deserialize<'de> for PackageName {
	#[inline]
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where D: de::Deserializer<'de> {
		let raw = <String>::deserialize(deserializer)?;
		Self::try_from(raw).map_err(|_| de::Error::custom("invalid package name"))
	}
}

impl Eq for PackageName {}

impl fmt::Display for PackageName {
	#[inline]
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.pad(&self.name) }
}

impl From<PackageName> for KeyWord {
	#[inline]
	fn from(src: PackageName) -> Self {
		let PackageName { name, .. } = src;
		Self::Command(name)
	}
}

impl From<PackageName> for String {
	#[inline]
	fn from(src: PackageName) -> Self { src.name }
}

impl Ord for PackageName {
	fn cmp(&self, other: &Self) -> Ordering {
		// Do it the hard way.
		if self.hyphens || other.hyphens {
			NormalizeHyphens(self.name.bytes()).cmp(NormalizeHyphens(other.name.bytes()))
		}
		else { self.name.cmp(&other.name) }
	}
}

impl PartialEq for PackageName {
	fn eq(&self, other: &Self) -> bool {
		// Do it the hard way.
		if self.hyphens || other.hyphens {
			if self.hyphens && other.hyphens {
				let a = NormalizeHyphens(self.name.bytes());
				let b = NormalizeHyphens(other.name.bytes());
				a.len() == b.len() && a.eq(b)
			}
			else { false }
		}
		// Straight shot!
		else { self.name == other.name }
	}
}

impl PartialOrd for PackageName {
	#[inline]
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl TryFrom<String> for PackageName {
	type Error = BashManError;

	fn try_from(mut name: String) -> Result<Self, Self::Error> {
		name.trim_mut();
		name.make_ascii_lowercase();
		let bytes = name.as_bytes();
		if ! bytes.is_empty() && bytes[0].is_ascii_alphabetic() {
			let mut hyphens = false;
			for b in bytes.iter().copied() {
				if b == b'-' { hyphens = true; }
				else if ! matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_') {
					return Err(BashManError::PackageName(name));
				}
			}

			return Ok(Self { name, hyphens });
		}

		Err(BashManError::PackageName(name))
	}
}



/// # No Hyphens.
///
/// This wraps a byte iterator for the sole purpose of replacing hyphens with
/// underscores.
struct NormalizeHyphens<I: ExactSizeIterator<Item=u8>>(I);

impl<I: ExactSizeIterator<Item=u8>> Iterator for NormalizeHyphens<I> {
	type Item = u8;

	#[inline]
	fn next(&mut self) -> Option<Self::Item> {
		match self.0.next() {
			Some(b'-') => Some(b'_'),
			Some(next) => Some(next),
			None => None,
		}
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		let len = self.len();
		(len, Some(len))
	}
}

impl<I: ExactSizeIterator<Item=u8>> ExactSizeIterator for NormalizeHyphens<I> {
	#[inline]
	fn len(&self) -> usize { self.0.len() }
}
