# Cargo BashMan

[![Build Status](https://github.com/Blobfolio/bashman/workflows/Build/badge.svg)](https://github.com/Blobfolio/bashman/actions)
[![Dependency Status](https://deps.rs/repo/github/blobfolio/bashman/status.svg)](https://deps.rs/repo/github/blobfolio/bashman)

`BashMan` is a Cargo plugin that helps you generate BASH completions, MAN pages, and/or a `CREDITS.md` page for your Rust apps using metadata from your projects' `Cargo.toml` manifests. It pairs well with the (unaffiliated) [cargo-deb](https://github.com/mmstick/cargo-deb).

(This can technically be used for non-Rust apps. It just parses the data out of a [TOML](https://en.wikipedia.org/wiki/TOML) file. Any TOML'll do.)

BASH completions are sub-command aware — one level deep — and avoid making duplicate suggestions. For example, if the line already has `-h`, it will not suggest `-h` or its long variant `--help`.

MAN pages are automatically populated with the primary sections — `NAME`, `DESCRIPTION`, `USAGE`, `SUBCOMMANDS`, `FLAGS`, `OPTIONS`, `ARGUMENTS` — and the top level page can be extended with additional arbitrary sections as needed. If subcommands are defined, additional pages for each are generated, showing their particular usage, flags, etc.

MAN pages are saved in both plain ("app.1") and GZipped ("app.1.gz") states. Linux `man` can read either format, so just pick whichever you prefer for distribution purposes. (Though Gzip is smaller…)

**This software is a work-in-progress.**

Feel free to use it, but if something weird happens — or if you have ideas for improvement — please open an [issue](https://github.com/Blobfolio/bashman/issues)!



## Installation

Debian and Ubuntu users can just grab the pre-built `.deb` package from the [latest release](https://github.com/Blobfolio/bashman/releases/latest).

This application is written in [Rust](https://www.rust-lang.org/) and can alternatively be built from source using [Cargo](https://github.com/rust-lang/cargo):

```bash
# Clone the source.
git clone https://github.com/Blobfolio/bashman.git

# Go to it.
cd bashman

# Build as usual. Specify additional flags as desired.
cargo build \
    --bin bashman \
    --release
```

(This should work under other 64-bit Unix environments too, like MacOS.)



## Usage

`BashMan` pulls all the data it needs to compile BASH completions and MAN pages straight from the specified `Cargo.toml` file. Once your manifest is set, all you need to do is run:

```bash
# Generate the stuff for the thing:
cargo bashman [--manifest-path /path/to/Cargo.toml]

# You can also pull up help via:
cargo bashman [-h/--help]
```

The flags `--no-bash`, `--no-man`, and `--no-credits` can be used to skip the generation of BASH completions, MAN pages, and/or `CREDITS.md` respectively.


## CONFIGURATION

The binary name, version, and description are taken from the standard `Cargo.toml` fields.

For everything else, start by adding a section to your `Cargo.toml` manifest like:

```toml
[package.metadata.bashman]
name = "Cargo BashMan"
bash-dir = "../release/completions"
man-dir = "../release/man"
credits-dir = "../"
```

| Key | Type | Description | Default |
| --- | ---- | ----------- | ------- |
| name | *string* | The proper name of your application. | If not provided, the binary name is used. |
| bash-dir | *directory* | The output directory for BASH completions. This can be an absolute path, or a path relative to the manifest. | If not provided, the manifest's parent directory is used. |
| credits-dir | *directory* | The output directory for the `CREDITS.md` dependency list. This can be an absolute path, or a path relative to the manifest. | If not provided, the manifest's parent directory is used. |
| man-dir | *directory* | The output directory for MAN page(s). This can be an absolute path, or a path relative to the manifest. | If not provided, the manifest's parent directory is used. |
| subcommands | *array* | An array of your app's subcommands, if any. | |
| switches | *array* | An array of your app's true/false flags, if any. | |
| options | *array* | An array of your app's key=value options, if any. | |
| arguments | *array* | An array of any trailing arguments expected by your app. | |
| sections | *array* | Arbitrary sections to append to the MAN page. | |

While `bash-dir`, `man-dir`, and `credits-dir` are required, the actual content generation can be skipped by using the CLI flags `--no-bash`, `--no-man`, and/or `--no-credits` respectively.


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
[package]
name = "cargo-bashman"
version = "0.1.0"
description = "BashMan is a Cargo plugin that helps you generate BASH completions and/or MAN pages for your Rust project."

...

[package.metadata.bashman]
name = "Cargo BashMan"
bash-dir = "../release/completions"
man-dir = "../release/man"
credits-dir = "../"

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



## License

See also: [CREDITS.md](CREDITS.md)

Copyright © 2022 [Blobfolio, LLC](https://blobfolio.com) &lt;hello@blobfolio.com&gt;

This work is free. You can redistribute it and/or modify it under the terms of the Do What The Fuck You Want To Public License, Version 2.

    DO WHAT THE FUCK YOU WANT TO PUBLIC LICENSE
    Version 2, December 2004
    
    Copyright (C) 2004 Sam Hocevar <sam@hocevar.net>
    
    Everyone is permitted to copy and distribute verbatim or modified
    copies of this license document, and changing it is allowed as long
    as the name is changed.
    
    DO WHAT THE FUCK YOU WANT TO PUBLIC LICENSE
    TERMS AND CONDITIONS FOR COPYING, DISTRIBUTION AND MODIFICATION
    
    0. You just DO WHAT THE FUCK YOU WANT TO.
