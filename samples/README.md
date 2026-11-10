# Sample documents

`docx/` holds documents written by Microsoft Word, so that the tests in `tests/samples.rs` run
against what a real word processor produces as well as against hand-written XML. The tests take
whatever `.docx` files are there, and pass when there are none.

To regenerate them, on Windows with Word installed:

```powershell
.\samples\generate.ps1
```

`report.docx` has a title, headings, bold and italic text, a footnote, a hyperlink, a bulleted and
a numbered list, a table, a comment and a header.
