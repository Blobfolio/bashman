# Cargo BashMan

`BashMan` is a Cargo plugin that helps you generate BASH completions and/or MAN pages for your Rust apps using metadata from your projects' `Cargo.toml` manifests. It pairs well with the (unaffiliated) [cargo-deb](https://github.com/mmstick/cargo-deb).

BASH completions are sub-command aware — one level deep — and avoid making duplicate suggestions. For example, if the line already has `-h`, it will not suggest `-h` or its long variant `--help`.

MAN pages are automatically populated with the primary sections — `NAME`, `DESCRIPTION`, `USAGE`, `SUBCOMMANDS`, `FLAGS`, `OPTIONS`, `ARGUMENTS` — and the top level page can be extended with additional arbitrary sections as needed. If subcommands are defined, additional pages for each are generated, showing their particular usage, flags, etc.

MAN pages are saved in both plain ("app.1") and GZipped ("app.1.gz") states. Linux `man` can read either format, so just pick whichever you prefer for distribution purposes. (Though Gzip is smaller…)

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



## Credits
| Library | License | Author |
| ---- | ---- | ---- |
| [chrono](https://crates.io/crates/chrono) | Apache-2.0 OR MIT | Kang Seonghoon, Brandon W Maister |
| [indexmap](https://crates.io/crates/indexmap) | Apache-2.0 OR MIT | bluss, Josh Stone |
| [libdeflater](https://crates.io/crates/libdeflater) | Apache-2.0 | Adam Kewley |
| [toml](https://crates.io/crates/toml) | Apache-2.0 OR MIT | Alex Crichton |



## License

Copyright © 2020 [Blobfolio, LLC](https://blobfolio.com) &lt;hello@blobfolio.com&gt;

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
