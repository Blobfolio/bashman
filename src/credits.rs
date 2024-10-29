/*!
# Cargo BashMan: Crate Credits.
*/

use crate::{
	BashManError,
	Dependency,
	Manifest,
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
		let deps = self.dependencies.as_slice();
		if deps.is_empty() {
			return f.write_str("This project has no dependencies.\n");
		}

		// Some dependencies are context dependent; some work is required.
		if deps.iter().any(Dependency::conditional) {
			f.write_str("| Package | Version | Author(s) | License | Context |\n| ---- | ---- | ---- | ---- | ---- |\n")?;

			// Required first.
			for dep in deps {
				if ! dep.conditional() { writeln!(f, "{dep} |")?; }
			}
			// Now the specific ones.
			for dep in deps {
				if dep.conditional() { writeln!(f, "{dep} {} |", dep.context())?; }
			}
		}
		// Everything is needed all the time!
		else {
			f.write_str("| Package | Version | Author(s) | License |\n| ---- | ---- | ---- | ---- |\n")?;
			for dep in deps { writeln!(f, "{dep}")?; }
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
		let dependencies = man.dependencies()?;

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
