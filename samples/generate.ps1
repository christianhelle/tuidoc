# Regenerates the sample documents with Microsoft Word, so the tests have files a real
# word processor wrote. Needs Word installed. Run from anywhere: .\samples\generate.ps1
$ErrorActionPreference = 'Stop'
$target = Join-Path $PSScriptRoot 'docx'
New-Item -ItemType Directory -Force -Path $target | Out-Null

$word = New-Object -ComObject Word.Application
$word.Visible = $false
$word.DisplayAlerts = 0
try {
    $doc = $word.Documents.Add()
    $selection = $word.Selection

    $selection.Style = $doc.Styles.Item(-63)    # Title
    $selection.TypeText('Quarterly report')
    $selection.TypeParagraph()

    $selection.Style = $doc.Styles.Item(-2)     # Heading 1
    $selection.TypeText('Summary')
    $selection.TypeParagraph()

    $selection.Style = $doc.Styles.Item(-1)     # Normal
    $selection.TypeText('Sales grew in every region. ')
    $selection.Font.Bold = 1
    $selection.TypeText('The north did best')
    $selection.Font.Bold = 0
    $selection.TypeText(', and the ')
    $selection.Font.Italic = 1
    $selection.TypeText('south')
    $selection.Font.Italic = 0
    $selection.TypeText(' is catching up.')
    $doc.Footnotes.Add($selection.Range, '', 'Figures are unaudited.') | Out-Null
    $selection.EndKey(6) | Out-Null             # wdStory
    $selection.TypeParagraph()

    $selection.TypeText('More at ')
    $doc.Hyperlinks.Add($selection.Range, 'https://example.com/', '', '', 'the example site') | Out-Null
    $selection.EndKey(6) | Out-Null
    $selection.TypeText('.')
    $selection.TypeParagraph()

    $selection.Style = $doc.Styles.Item(-3)     # Heading 2
    $selection.TypeText('Highlights')
    $selection.TypeParagraph()

    $selection.Style = $doc.Styles.Item(-1)
    $selection.Range.ListFormat.ApplyBulletDefault()
    $selection.TypeText('New customers')
    $selection.TypeParagraph()
    $selection.TypeText('Fewer returns')
    $selection.TypeParagraph()
    $selection.Range.ListFormat.RemoveNumbers()

    $selection.Range.ListFormat.ApplyNumberDefault()
    $selection.TypeText('Hire two people')
    $selection.TypeParagraph()
    $selection.TypeText('Open the new office')
    $selection.TypeParagraph()
    $selection.Range.ListFormat.RemoveNumbers()

    $selection.Style = $doc.Styles.Item(-3)
    $selection.TypeText('By region')
    $selection.TypeParagraph()
    $selection.Style = $doc.Styles.Item(-1)

    $table = $doc.Tables.Add($selection.Range, 3, 2)
    $table.Borders.Enable = 1
    $cells = @('Region', 'Sales', 'North', '1,200', 'South', '950')
    for ($i = 0; $i -lt 6; $i++) {
        $table.Cell([math]::Floor($i / 2) + 1, ($i % 2) + 1).Range.Text = $cells[$i]
    }
    $selection.EndKey(6) | Out-Null
    $selection.TypeText('Prepared for the board.')
    $doc.Comments.Add($selection.Paragraphs.Item(1).Range, 'Check the date.') | Out-Null

    $header = $doc.Sections.Item(1).Headers.Item(1).Range
    $header.Text = 'Quarterly report'

    $path = Join-Path $target 'report.docx'
    $doc.SaveAs2([string]$path)
    $doc.Close(0)
    Write-Host "Wrote $path"
}
finally {
    $word.Quit()
}
