/*!
# `Cargo BashMan`

`BashMan` is a Cargo plugin that helps you generate BASH completions and/or MAN pages for your Rust apps using metadata from your projects' `Cargo.toml` manifests. It pairs well with the (unaffiliated) [cargo-deb](https://github.com/mmstick/cargo-deb).

BASH completions are sub-command aware — one level deep — and avoid making duplicate suggestions. For example, if the line already has `-h`, it will not suggest `-h` or its long variant `--help`.

MAN pages are automatically populated with the primary sections — `NAME`, `DESCRIPTION`, `USAGE`, `SUBCOMMANDS`, `FLAGS`, `OPTIONS`, `ARGUMENTS` — and the top level page can be extended with additional arbitrary sections as needed. If subcommands are defined, additional pages for each are generated, showing their particular usage, flags, etc.

MAN pages are saved in both plain ("app.1") and Gzipped ("app.1.gz") states. Linux `man` can read either format, so just pick whichever you prefer for distribution purposes. (Though Gzip is smaller…)

**This software is a work-in-progress.**

Feel free to use it, but if something weird happens — or if you have ideas for improvement — please open an [issue](https://github.com/Blobfolio/bashman/issues)!



## Installation

This application is written in [Rust](https://www.rust-lang.org/) and can be installed using [Cargo](https://github.com/rust-lang/cargo).

For stable Rust (>= `1.47.0`), run:
```bash
RUSTFLAGS="-C link-arg=-s" cargo install \
    --git https://github.com/Blobfolio/channelz.git \
    --bin channelz \
    --target x86_64-unknown-linux-gnu
```

Pre-built `.deb` packages are also added for each [release](https://github.com/Blobfolio/channelz/releases/latest). They should always work for the latest stable Debian and Ubuntu.



## Usage

`BashMan` pulls all the data it needs to compile BASH completions and MAN pages straight from the specified `Cargo.toml` file. Once your manifest is set, all you need to do is run:

```bash
cargo bashman [-m/--manifest-path] /path/to/Cargo.toml
```


## CONFIGURATION

The binary name, version, and description are taken from the standard `Cargo.toml` fields.

For everything else, start by adding a section to your `Cargo.toml` manifest like:

```toml
[package.metadata.bashman]
name = "Cargo BashMan"
bash-dir = "../release/bash_completion.d"
man-dir = "../release/man1"
```

| Key | Type | Description | Default |
| --- | ---- | ----------- | ------- |
| name | *string* | The proper name of your application. | If not provided, the binary name is used. |
| bash-dir | *directory* | The output directory for BASH completions. This can be an absolute path, or a path relative to the manifest. | If not provided, the manifest's parent directory is used. |
| man-dir | *directory* | The output directory for MAN page(s). This can be an absolute path, or a path relative to the manifest. | If not provided, the manifest's parent directory is used. |
| subcommands | *array* | An array of your app's subcommands, if any. | |
| switches | *array* | An array of your app's true/false flags, if any. | |
| options | *array* | An array of your app's key=value options, if any. | |
| arguments | *array* | An array of any trailing arguments expected by your app. | |
| sections | *array* | Arbitrary sections to append to the MAN page. | |

### SUBCOMMANDS

When adding subcommands, each entry requires the following fields:

| Key | Type | Description | Default |
| --- | ---- | ----------- | ------- |
| name | *string* | The proper name of the command. | If not provided, the `cmd` value will be used. |
| cmd | *string* | The subcommand. | |
| description | *string* | A description of what the subcommand does. | |

Subcommands can have their own switches, options, arguments. These are specified in the `switches`, `options`, and `arguments` sections respectively. Keep reading…

Example:
```toml
[[package.metadata.bashman.subcommands]]
name="Whale Talk"
cmd="whale"
description="Print an underwater message."
```

### SWITCHES

A "switch" is a CLI flag that either is or isn't. It can be a short key, like `-h`, or a long key like `--help`, or both. The value is implicitly `true` if the flag is present, or `false` if not.

Switches have the following fields:

| Key | Type | Description |
| --- | ---- | ----------- |
| short | *string* | A short key, like `-h`. |
| long | *string* | A long key, like `--help`. |
| description | *string* | A description for the flag. |
| subcommands | *array* | If this switch applies to one or more subcommands, list the commands here. If a switch applies to the top-level app, omit this field, or include an empty `""` entry in the array. |

Example:
```toml
[[package.metadata.bashman.switches]]
short = "-V"
long = "--version"
description = "Print application version."

[[package.metadata.bashman.switches]]
short = "-h"
long = "--help"
description = "Print help information."
subcommands = [ "call", "text", "" ]
```

### OPTIONS

An "option" is exactly like a "switch", except it takes a value. As such, they have a couple more fields:

| Key | Type | Description |
| --- | ---- | ----------- |
| short | *string* | A short key, like `-h`. |
| long | *string* | A long key, like `--help`. |
| description | *string* | A description for the flag. |
| label | *string* | A placeholder label for the value bit, like `<FILE>`. |
| path | *bool* | If `true`, the BASH completions will suggest files/directories as potential values. If `false`, no value suggestion will be hazarded. |
| subcommands | *array* | If this option applies to one or more subcommands, list the commands here. If an option applies to the top-level app, omit this field, or include an empty `""` entry in the array. |

Example:
```toml
[[package.metadata.bashman.options]]
short = "-m"
long = "--manifest-path"
description = "Path to the Cargo.toml file to use."
label = "<Cargo.toml>"
path = true

[[package.metadata.bashman.options]]
short = "-c"
long = "--color"
description = "Use this foreground color."
label = "<NUM>"
subcommands = [ "print", "echo" ]
```

### ARGUMENTS

A trailing argument is what comes after everything else.

| Key | Type | Description |
| --- | ---- | ----------- |
| label | *string* | A placeholder label for the value bit, like `<FILE(s)…>`. |
| description | *string* | A description for the argument. |
| subcommands | *array* | If this argument applies to one or more subcommands, list the commands here. If it applies to the top-level app, omit this field, or include an empty `""` entry in the array. |

Example:
```toml
[[package.metadata.bashman.arguments]]
label = "<FILE(s)…>"
description = "Files and directories to search."
subcommands = [ "search" ]
```

### SECTIONS

`BashMan` will automatically generate manual sections for:
 * `NAME`
 * `DESCRIPTION`
 * `USAGE`
 * `SUBCOMMANDS` (if any)
 * `FLAGS` (i.e. "switches", if any)
 * `OPTIONS` (if any)
 * `ARGUMENTS` (if any)

If you would like to include other information (for the top-level MAN page), such as a list of sister software repositories, you can do so by adding sections with the following fields:

| Key | Type | Description |
| --- | ---- | ----------- |
| name | *string* | The section name, e.g. `RECIPES`. |
| inside | *bool* | If `true`, the section will be indented (like most sections are). |
| lines | *array* | An array of paragraph lines (strings) to append. Line breaks are forced between entries, but you could jam everything into one string to just have it wrap. |
| items | *array* | An array of key/value pairs to list in a manner similar to how arguments are presented. Each entry should be an array with exactly two string values, `[ "Label", "A description or whatever." ]` |

Generally speaking, you'll want either "lines" or "items" for a given section, but not both.

Example:
```toml
[[package.metadata.bashman.sections]]
name = "FILE TYPES"
inside = true
lines = [
    "This program will search for files with the following extensions:",
    ".foo; .bar; .img",
]

[[package.metadata.bashman.sections]]
name = "CREDITS"
inside = true
items = [
    ["Bob", "https://bob.com"],
    ["Alice", "https://github.com/Alice"],
]
```

### ALL TOGETHER NOW

Taking `BashMan` as an example, the `Cargo.toml` will end up containing something like:
```toml
[package.metadata.bashman]
name = "Cargo BashMan"
bash-dir = "../release/bash_completion.d"
man-dir = "../release/man1"

[[package.metadata.bashman.switches]]
short = "-h"
long = "--help"
description = "Print help information."

[[package.metadata.bashman.switches]]
short = "-V"
long = "--version"
description = "Print application version."

[[package.metadata.bashman.options]]
short = "-m"
long = "--manifest-path"
description = "Path to the Cargo.toml file to use."
label = "<Cargo.toml>"
path = true
```
*/

#![warn(clippy::filetype_is_file)]
#![warn(clippy::integer_division)]
#![warn(clippy::needless_borrow)]
#![warn(clippy::nursery)]
#![warn(clippy::pedantic)]
#![warn(clippy::perf)]
#![warn(clippy::suboptimal_flops)]
#![warn(clippy::unneeded_field_pattern)]
#![warn(macro_use_extern_crate)]
#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(non_ascii_idents)]
#![warn(trivial_casts)]
#![warn(trivial_numeric_casts)]
#![warn(unreachable_pub)]
#![warn(unused_extern_crates)]
#![warn(unused_import_braces)]

#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::map_err_ignore)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]


use cargo_bashman::BashManError;
use fyi_menu::{
	ArgueError,
	FLAG_HELP,
	FLAG_VERSION,
};
use fyi_msg::Msg;
use std::{
	ffi::OsStr,
	os::unix::ffi::OsStrExt,
};



/// Main.
fn main() {
	match _main() {
		Err(BashManError::Argue(ArgueError::WantsVersion)) => {
			fyi_msg::plain!(concat!("Cargo BashMan v", env!("CARGO_PKG_VERSION")));
		},
		Err(BashManError::Argue(ArgueError::WantsHelp)) => {
			helper();
		},
		Err(e) => {
			Msg::error(e.to_string()).die(1);
		},
		Ok(_) => {},
	}
}

#[inline]
// Actual main.
fn _main() -> Result<(), BashManError> {
	// Parse CLI arguments.
	let args = fyi_menu::Argue::new(FLAG_HELP | FLAG_VERSION)
		.map_err(BashManError::Argue)?;

	let bm = cargo_bashman::load(
		args.option2(b"-m", b"--manifest-path")
			.map(OsStr::from_bytes)
	)?;

	bm.write()?;

	Ok(())
}

#[cold]
/// Print Help.
fn helper() {
	fyi_msg::plain!(concat!(
		r"
   __              __
   \ `-._......_.-` /
    `.  '.    .'  .'
     //  _`\/`_  \\
    ||  /\O||O/\  ||
    |\  \_/||\_/  /|
    \ '.   \/   .' /
    / ^ `'~  ~'`   \
   /  _-^_~ -^_ ~-  |    ", "\x1b[38;5;199mCargo BashMan\x1b[0;38;5;69m v", env!("CARGO_PKG_VERSION"), "\x1b[0m", r"
   | / ^_ -^_- ~_^\ |    A BASH completion script and MAN
   | |~_ ^- _-^_ -| |    page generator for Rust projects.
   | \  ^-~_ ~-_^ / |
   \_/;-.,____,.-;\_/
======(_(_(==)_)_)======

USAGE:
    cargo bashman [FLAGS] [OPTIONS]

FLAGS:
    -h, --help                  Prints help information.
    -V, --version               Prints version information.

OPTIONS:
    -m, --manifest-path <FILE>  Read file paths from this list.
"
    ));
}
