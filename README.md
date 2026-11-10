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

Without a file, tuidoc starts with an empty document and asks where to put it when you save.
`tdoc --version` prints the version and `tdoc --help` the usage.

The document fills the screen, wrapped to the width of the terminal. The title shows the file, a
`*` while there are unsaved changes, and which part of the document you are in. Documents with
headers, footers, footnotes, endnotes or comments get a line of tabs for them above the status
line. The status line shows the mode, the keys that matter most, keys typed so far, and the
cursor's paragraph out of how many.

## Keys

Press `?` at any time for the keys of the mode you are in.

### Normal mode

| Key | Action |
| --- | --- |
| `h` `j` `k` `l`, arrows | Move. `j` and `k` go by screen line, since paragraphs wrap; counts work, as in `5j` |
| `w` `b` `e`, `W` `B` `E` | Next word, word back, end of word; the capitals only stop at spaces |
| `0` `^` `$` | Start of the paragraph, its first text, its end |
| `Home` `End` | Start and end of the screen line |
| `f` `F` `t` `T`, `;` `,` | Find a character in the paragraph; repeat, and repeat the other way |
| `{` `}` | Previous, next paragraph |
| `[[` `]]` | Previous, next heading |
| `gg` `G`, `{n}G` | First, last, nth paragraph |
| `Ctrl-G` | Ask for a paragraph number to go to |
| `H` `M` `L` | Top, middle, bottom of the screen |
| `Ctrl-D` `Ctrl-U`, `Ctrl-F` `Ctrl-B`, `PgDn` `PgUp` | Half a page, a page |
| `zz` `zt` `zb` | Put the cursor's line in the middle, at the top, at the bottom |
| `Tab` `Shift-Tab` | Next, previous cell of a table |
| `i` `a` `I` `A` | Type before or after the cursor, at the start or end of the paragraph |
| `o` `O` | Type in a new paragraph below, above |
| `x` `X` `D` | Delete the character, the one before, the rest of the paragraph |
| `s` `S` `C` | Change the character, the paragraph, the rest of the paragraph |
| `r` | Replace the character with the next one typed |
| `J` | Join the paragraph with the next |
| `d` `c` `y` + a move | Delete, change, copy: `dw`, `c$`, `y3j`, `diw`, `daw`, `dip` |
| `dd` `cc` `yy` | The same for whole paragraphs, with a count as in `3dd` |
| `p` `P` | Paste after, before |
| `u` or `Ctrl-Z`, `Ctrl-R` or `Ctrl-Y` | Undo, redo |
| `v` `V` | Select text, select whole paragraphs |
| `/`, `n` `N`, `*` | Find text, ignoring case; next, previous; find the word under the cursor |
| `K` | Look up the footnote, endnote, comment, link or picture under the cursor |
| `gt` `gT`, `F6` `Shift-F6` | Next, previous part of the document |
| `gO` | Open the outline |
| `Ctrl-S`, `F12` | Save, save as |
| `Ctrl-O`, `F5` | Open another file, reload this one |
| `q` or `Ctrl-Q`, `ZZ`, `ZQ` | Quit, save and quit, quit without saving |
| `?` or `F1` | Help |

There is no `:` command line: everything is a key.

### Insert mode

| Key | Action |
| --- | --- |
| `Esc` or `Ctrl-C` | Back to normal mode |
| `Enter` | New paragraph. In a list, a new item; on an empty item, the end of the list |
| `Backspace` `Delete` | Delete; at the edge of a paragraph, join it with its neighbour |
| arrows, `Home` `End` | Move |
| `Ctrl-W` `Ctrl-U` | Delete the word before the cursor, or everything back to the start |
| `Tab` `Shift-Tab` | A tab; in a table, the next or previous cell |
| `Ctrl-S` | Save, and go on typing |
| `F1` | Help |

New text takes the formatting of the text before it. Everything typed between entering insert
mode and leaving it undoes as one change. Text pasted into the terminal goes in where typing
would, a new paragraph for each line.

### Visual modes

`v` selects text and `V` whole paragraphs. Any move extends the selection.

| Key | Action |
| --- | --- |
| `o` | Jump to the other end of the selection |
| `d` or `x`, `y`, `c` or `s` | Delete, copy, change |
| `gb` `gi` `gu` `gs` | Bold, italic, underline, strikethrough |
| `g0`, `g1`–`g6`, `gl` | Normal text, heading 1–6, list |
| `>` `<` | List level or indent, deeper and back |
| `J` | Join the paragraphs |
| `v` `V` | Switch between the two, or leave |
| `Esc` or `Ctrl-C` | Back to normal mode |

Whatever is copied or deleted also goes to the system clipboard, in terminals that support it.

### Formatting

| Key | Action |
| --- | --- |
| `gb` `gi` `gu` `gs` + a move | Bold, italic, underline, strikethrough: `gbiw` a word, `gi$` to the end, `guip` the paragraph |
| `g1`–`g6` | Make the paragraph a heading of that level |
| `g0` | Make it ordinary text again |
| `gl` | No list, then bullets, then numbers, then no list |
| `>>` `<<` | A list item one level deeper or back; any other paragraph indented half an inch or back |
| `gh` | Pick any paragraph style the document has; type to narrow the list |

A format is switched on unless all of the text already has it, as in Word. Heading styles and
lists a document doesn't have yet are added to it when first used.

### Outline

`gO` opens a pane listing the document's headings.

| Key | Action |
| --- | --- |
| `j` `k`, `gg` `G` | Move through the headings |
| `Enter` | Go to the heading |
| `Esc` or `Tab` | Back to the document, leaving the outline open |
| `gO` or `q` | Close the outline |

### Unsaved changes

Quitting, opening another file or reloading with unsaved changes asks first:

| Key | Action |
| --- | --- |
| `s` | Save, then go on |
| `d` | Discard the changes and go on |
| `Esc` | Stay |

### Mouse

Click to put the cursor somewhere, to pick a tab, or to go to a heading in the outline. The wheel
scrolls three lines.

### File dialog

`Ctrl-O` and a first save open a dialog listing folders and documents.

| Key | Action |
| --- | --- |
| `↑` `↓` | Move through the list |
| `Enter` | Go into the folder, or choose the file |
| typing | A file name; `.docx` is added if it has no extension |
| `Esc` | Cancel |

## What is shown

- **Text** with bold, italic, underline and strikethrough. Fonts, sizes and colours are left to
  the terminal's theme.
- **Headings** in bold cyan, the title centred, and paragraphs centred, right-aligned or indented
  as the document has them.
- **Lists** with their bullets and numbers, counted as Word counts them.
- **Tables** as boxes, with the text of each cell wrapped to its width. `Tab` moves between
  cells, and their text is edited like any other.
- **Pictures** as `[image: description]`, **footnotes** and **endnotes** as `[1]`, **comments** as
  `[c1]`, **links** underlined, page breaks as a rule across the page. `K` shows what is behind
  each.
- **Headers, footers, footnotes, endnotes and comments** on tabs of their own, to read and copy
  from.
- Tracked insertions as ordinary text; tracked deletions are hidden.

## How saving works

tuidoc never writes a document out from scratch. Saving copies the original file part by part,
and within the document copies every paragraph that wasn't edited byte for byte; only the
paragraphs you changed are written anew, keeping their properties and the formatting of each run.
So pictures, styles, headers, footnotes, comments, fields, bookmarks and anything else tuidoc
doesn't understand come through untouched.

A file changed by another program since it was opened is not overwritten: saving says so, and
`F5` reloads it. Saves go to a temporary file first, so a failed save leaves the old file whole.

## Limitations

- **Word documents only.** `.docx`, and `.docm` and `.dotx` which are built the same way. Not
  `.doc`, `.odt` or `.rtf`.
- **Only the body can be edited.** Headers, footers, notes and comments are shown but not
  changed.
- **Tables keep their shape.** Cell text can be edited; rows and columns can't be added or
  removed. A table inside a table cell is shown as `[table]`, and tables laid out in ways tuidoc
  doesn't model are shown as greyed text.
- **Content controls and text boxes** show their text, greyed, and can't be edited.
- **Nothing new but text.** Pictures, links, footnotes and comments can't be inserted.
- **Formatting is basic.** Bold, italic, underline, strikethrough, paragraph styles, lists and
  indent; no fonts, colours, sizes or alignment.
- **Edits aren't tracked.** In a document with change tracking on, tuidoc's edits are written as
  plain changes.
- **Vim's larger machinery is absent:** named registers, `.`, macros, marks and `:` commands.
- **No pages.** The document is one scroll of text; there is no pagination or printing.

## Development

```bash
cargo test
```

The tests exercise four seams: document file I/O, the document model, app state driven by key
events, and the screen rendered to an in-memory terminal. Packages for the tests are built from
hand-written XML in `tests/common`, and [samples/README.md](samples/README.md) describes the
documents written by Word itself.

## License

[MIT](LICENSE)
