/*!
# Cargo BashMan: Parsing Helpers.
*/

use serde::{
	Deserialize,
	Deserializer,
};
use trimothy::{
	NormalizeWhitespace,
	TrimMut,
};



/// # Deserialize: Non-Empty String, Normalized.
///
/// This will return an error if a string is present but empty.
pub(super) fn deserialize_nonempty_str_normalized<'de, D>(deserializer: D) -> Result<String, D::Error>
where D: Deserializer<'de> {
	let mut out = <String>::deserialize(deserializer)?;
	normalize_string(&mut out);
	if out.is_empty() { Err(serde::de::Error::custom("value cannot be empty")) }
	else { Ok(out) }
}

#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: Optional Non-Empty String.
///
/// This will return `None` if the string is empty.
pub(super) fn deserialize_nonempty_opt_str<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where D: Deserializer<'de> {
	Ok(
		<String>::deserialize(deserializer).ok()
			.and_then(|mut x| {
				x.trim_mut();
				if x.is_empty() { None }
				else { Some(x) }
			})
	)
}

#[expect(clippy::unnecessary_wraps, reason = "We don't control this signature.")]
/// # Deserialize: Optional Non-Empty String, Normalized.
///
/// This will return `None` if the string is empty, normalizing whitespace and
/// control characters along the way.
pub(super) fn deserialize_nonempty_opt_str_normalized<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where D: Deserializer<'de> {
	Ok(
		<String>::deserialize(deserializer).ok()
			.and_then(|mut x| {
				normalize_string(&mut x);
				if x.is_empty() { None }
				else { Some(x) }
			})
	)
}

/// # Normalize String.
///
/// Compact whitespace and strip control characters.
///
/// This proceeds under the assumption that most normalization can be achieved
/// "inline" via `retain`, but if substitution is required it will rebuild the
/// string char-by-char.
pub(super) fn normalize_string(raw: &mut String) {
	let mut ws = true;
	let mut rebuild = false;
	raw.retain(|c: char|
		if c.is_whitespace() {
			if ws { false }
			else {
				ws = true;
				if c != ' ' { rebuild = true; }
				true
			}
		}
		else if c.is_control() { false }
		else {
			ws = false;
			true
		}
	);

	// We encountered something requiring more than a strip; rebuild!
	if rebuild { *raw = raw.normalized_whitespace().collect(); }
	// Just trim the end and we're good to go!
	else { raw.trim_end_mut(); }
}



#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn t_normalized_control() {
		let mut buf = String::new();

		for (raw, expected) in [
			("Björk", "Björk"),
			(" Björk\t\n", "Björk"),
			("hello\tB\0j\x1börk", "hello Björk"),
			(" \0 ", ""),
		] {
			raw.clone_into(&mut buf);
			normalize_string(&mut buf);
			assert_eq!(buf, expected);
		}
	}
}
