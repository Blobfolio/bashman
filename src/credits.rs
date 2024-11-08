/*!
# Cargo BashMan: Crate Credits.
*/

use crate::{
	BashManError,
	Dependency,
	Manifest,
	TargetTriple,
};
use std::{
	fmt,
	path::{
		Path,
		PathBuf,
	},
};
use utc2k::Utc2k;



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

	/// # Target.
	target: Option<TargetTriple>,

	/// # Dependencies.
	dependencies: &'a [Dependency],
}

impl<'a> fmt::Display for CreditsWriter<'a> {
	/// # Write Credits!
	///
	/// This method writes a markdown table entry for the dependency.
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		// With target.
		if let Some(target) = self.target {
			writeln!(
				f,
				"# Project Dependencies
    Package:   {}
    Version:   {}
    Target:    {target}
    Generated: {} UTC
",
				self.name,
				self.version,
				Utc2k::now(),
			)?;
		}
		// Without target.
		else {
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
		}

		// There may not be any dependencies.
		let Some(last) = self.dependencies.last() else {
			return f.write_str("This project has no dependencies.\n");
		};

		// Print a header and each dependency.
		f.write_str("| Package | Version | Author(s) | License |\n| ---- | ---- | ---- | ---- |\n")?;
		let mut build = false;
		let mut children = false;
		for dep in self.dependencies {
			if dep.build() { build = true; }
			if ! dep.direct() { children = true; }
			writeln!(f, "{dep}")?;
		}

		// If we have contexts, note them.
		if build || children || last.conditional() {
			f.write_str("\n### Legend\n\n")?;
			if children {
				f.write_str("* **Direct Dependency**\n* Child Dependency\n")?;
			}
			if last.conditional() { f.write_str("* _Optional Dependency_\n")?; }
			if build { f.write_str("* ⚒️ Build-Only\n")?; }
		}

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

		// Done!
		Ok(Self {
			src,
			dst,
			name,
			version: cmd.version(),
			target: man.target(),
			dependencies: man.dependencies(),
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



#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn t_creditswriter() {
		let manifest = Manifest::from_test().expect("Manifest failed.");
		let writer = CreditsWriter::try_from(&manifest).expect("CreditsWriter failed.");

		// Test the credits generate as expected, save for the timestamp.
		let expected = std::fs::read_to_string("skel/metadata.credits")
			.expect("Missing skel/metadata.credits");
		let mut out = writer.to_string();
		let pos = out.find("    Generated: ").expect("Missing timestamp.");
		out.replace_range(pos + 15..pos + 35, "");

		assert_eq!(out, expected);
	}
}
