/*!
# Cargo BashMan: Package Name.
*/

use crate::{
	BashManError,
	KeyWord,
};
use serde::de;
use std::{
	cmp::Ordering,
	fmt,
};



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
		Self::try_from(raw.as_str()).map_err(|_| de::Error::custom("invalid package name"))
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

impl TryFrom<&str> for PackageName {
	type Error = BashManError;

	fn try_from(src: &str) -> Result<Self, Self::Error> {
		let src = src.trim();
		let mut name = String::with_capacity(src.len());
		let mut hyphens = false;
		for c in src.chars() {
			match c {
				'a'..='z' => { name.push(c); },
				'A'..='Z' => { name.push(c.to_ascii_lowercase()); },
				'0'..='9' | '-' | '_' if ! name.is_empty() => {
					if c == '-' { hyphens = true; }
					name.push(c);
				},
				_ => return Err(BashManError::PackageName(src.to_owned())),
			}
		}

		if name.is_empty() { Err(BashManError::PackageName(src.to_owned())) }
		else { Ok(Self { name, hyphens }) }
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
