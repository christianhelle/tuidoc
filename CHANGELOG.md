# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-11-11

### Added

- Viewing documents: `tdoc <file.docx>` shows a Word document wrapped to the
  terminal, with bold, italic, underline and strikethrough, headings, lists
  with their bullets and numbers, and tables drawn as boxes. Pictures,
  footnotes, comments and page breaks show as placeholders.
- Vim-style moving: `h` `j` `k` `l`, words, paragraphs, headings with `[[` and
  `]]`, `gg` and `G`, `f` and `t`, paging and counts. `j` and `k` go by screen
  line, and the line numbers show each line's distance from the cursor.
- Editing: insert mode, the `d` `c` `y` operators with moves and the `iw`,
  `aw` and `ip` objects, `x` `r` `J` `o` `O`, visual modes with `v` and `V`,
  paste, and undo and redo. Text keeps its formatting through every edit, and
  the text of table cells is edited like any other.
- Formatting: `gb` `gi` `gu` `gs` with a move, or on a selection, switch bold,
  italic, underline and strikethrough. `g1` to `g6` make headings and `g0`
  ordinary text, `gl` goes from no list to bullets to numbers, `>>` and `<<`
  change a list item's level or a paragraph's indent, and `gh` picks any
  paragraph style the document has.
- Saving that leaves the rest alone: `Ctrl-S` rewrites only the paragraphs
  that were edited and copies everything else byte for byte, so pictures,
  styles, headers and whatever tuidoc doesn't show survive. A file changed by
  another program is not overwritten.
- New documents: `tdoc` without a file starts an empty document, and `F12`
  saves under a new name.
- The other parts of a document: headers, footers, footnotes, endnotes and
  comments each have a tab, reached with `gt` and `gT`, to read and copy from.
- Looking things up: `K` shows the footnote, comment, link address or picture
  description under the cursor.
- An outline: `gO` lists the headings in a pane beside the text, and `Enter`
  goes to one.
- Search: `/` finds text ignoring case, `n` and `N` repeat it, and `*` finds
  the word under the cursor. `Ctrl-G` goes to a paragraph by number.
- Opening and reloading from inside: `Ctrl-O` browses for another document
  and `F5` reloads the file, each asking first about unsaved changes.
- The clipboard: copied and deleted text goes to the system clipboard in
  terminals that support it, and text pasted into the terminal goes in where
  typing would.
- Mouse: click to place the cursor, pick a tab or go to a heading, and scroll
  with the wheel.
- Help: `?` lists the keys of the mode you are in.

[Unreleased]: https://github.com/christianhelle/tuidoc/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/christianhelle/tuidoc/releases/tag/v0.1.0
