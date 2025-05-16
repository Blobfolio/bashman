/*!
# Cargo BashMan: Keyword.
*/

use crate::BashManError;
use serde::de;
use std::fmt;



#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
/// # Keyword.
///
/// This struct is used to enforce key and command requirements.
/// * Keys must start with one or two dashes followed by an ASCII alphanumeric character;
///   * Subsequent characters in long keys, if any, must be alphanumeric, `-`, or `_`;
/// * Commands must be lowercase, start with an ASCII alphanumeric, and contain only alphanumerics, `-`, or `_`;
pub(crate) enum KeyWord {
	/// # A (sub)command.
	Command(String),

	/// # A short or long key.
	Key(String),
}

impl<'de> de::Deserialize<'de> for KeyWord {
	#[inline]
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where D: de::Deserializer<'de> {
		let raw = <String>::deserialize(deserializer)?;
		Self::try_from(raw.as_str()).map_err(|_| de::Error::custom("invalid keyword"))
	}
}

impl fmt::Display for KeyWord {
	#[inline]
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		<str as fmt::Display>::fmt(self.as_str(), f)
	}
}

impl TryFrom<&str> for KeyWord {
	type Error = BashManError;

	fn try_from(src: &str) -> Result<Self, Self::Error> {
		/// # Valid Bytes?
		///
		/// Returns true if there is a first byte, that byte is ASCII
		/// alphanumeric, and all subsequent characters are alphanumeric, `-`,
		/// or `_`.
		const fn valid_bytes(mut bytes: &[u8]) -> bool {
			let [b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9', rest @ ..] = bytes else { return false; };
			bytes = rest;
			while let [b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_', rest @ ..] = bytes {
				bytes = rest;
			}
			bytes.is_empty()
		}

		let src = src.trim();
		if ! src.is_empty() && src.is_ascii() {
			// Count the leading dashes.
			let mut bytes = src.as_bytes();
			let mut dashes = 0;
			while let [b'-', rest @ ..] = bytes {
				dashes += 1;
				bytes = rest;
			}

			// A subcommand?
			if dashes == 0 {
				if valid_bytes(bytes) {
					return Ok(Self::Command(src.to_owned()));
				}
			}
			// A short key?
			else if dashes == 1 {
				if bytes.len() == 1 && bytes[0].is_ascii_alphanumeric() {
					return Ok(Self::Key(src.to_owned()));
				}
			}
			// A long key?
			else if dashes == 2 && valid_bytes(bytes) {
				return Ok(Self::Key(src.to_owned()));
			}
		}

		Err(BashManError::KeyWord(src.to_owned()))
	}
}

impl KeyWord {
	/// # Label.
	pub(crate) const fn label(&self) -> &'static str {
		match self {
			Self::Command(_) => "(sub)command",
			Self::Key(_) => "key",
		}
	}

	/// # As String Slice.
	pub(crate) const fn as_str(&self) -> &str {
		match self { Self::Command(s) | Self::Key(s) => s.as_str() }
	}
}
