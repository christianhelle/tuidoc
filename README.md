# TUIDoc

[![crates.io](https://img.shields.io/crates/v/tuidoc.svg)](https://crates.io/crates/tuidoc)
[![CI](https://github.com/christianhelle/tuidoc/actions/workflows/ci.yml/badge.svg)](https://github.com/christianhelle/tuidoc/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

View and edit Word `.docx` documents from the terminal, with vim-style keys. Saving rewrites only
the paragraphs you edited, so images, styles, headers, comments and everything else tuidoc does not
show survive untouched.

## Install

tuidoc installs as the `tdoc` command, which is quicker to type.

### macOS/Linux

```bash
curl -fsSL https://christianhelle.com/tuidoc/install | bash
```

This installs the latest release to `~/.local/bin`. To install somewhere else, or to pin a release:

```bash
curl -fsSL https://christianhelle.com/tuidoc/install | INSTALL_DIR="$HOME/bin" bash
curl -fsSL https://christianhelle.com/tuidoc/install | VERSION="0.1.0" bash
```

### Windows PowerShell

```powershell
irm https://christianhelle.com/tuidoc/install.ps1 | iex
```

This installs the latest release to `%LOCALAPPDATA%\Programs\tuidoc`, or to `~\.local\bin` or `~\bin`
if one of those already exists, and adds it to your user `PATH`. To install somewhere else, or to pin
a release:

```powershell
$install = irm https://christianhelle.com/tuidoc/install.ps1
& ([scriptblock]::Create($install)) -InstallDir "$env:USERPROFILE\bin"
& ([scriptblock]::Create($install)) -Version "0.1.0"
```

### Cargo

With [Rust](https://rustup.rs) 1.89 or later, install it from
[crates.io](https://crates.io/crates/tuidoc):

```bash
cargo install tuidoc --locked
```

### Release archives

Download a build from the [releases page](https://github.com/christianhelle/tuidoc/releases), unpack
it and put `tdoc` somewhere on your `PATH`. Archives are available for Linux, macOS and Windows, on
x64 and ARM64.

## Usage

```bash
tdoc report.docx
tdoc
```

Without a file, tuidoc starts with an empty document.

## Development

```bash
cargo test
```

## License

[MIT](LICENSE)
