using System.IO;
using System.IO.Compression;
using System.Text;
using System.Xml.Linq;
using CSVRescue.Services;

namespace CSVRescue.Tests;

public class CsvTests : IDisposable
{
    private readonly string _dir = Path.Combine(Path.GetTempPath(), "csvrescue-test-" + Guid.NewGuid().ToString("N")[..8]);
    public CsvTests() { Directory.CreateDirectory(_dir); Encoding.RegisterProvider(CodePagesEncodingProvider.Instance); }
    public void Dispose() { try { Directory.Delete(_dir, true); } catch { /* temp */ } }

    private static byte[] B(Encoding e, string s, bool bom = false) => (bom ? e.GetPreamble() : []).Concat(e.GetBytes(s)).ToArray();
    private static Encoding Win1252 => Encoding.GetEncoding(1252);

    // ── Encoding ─────────────────────────────────────────────────

    [Theory]
    [InlineData("UTF-8", true)]
    [InlineData("UTF-16 LE", true)]
    [InlineData("UTF-16 BE", true)]
    [InlineData("UTF-16 LE", false)]
    [InlineData("UTF-16 BE", false)]
    public void DetectsUnicodeEncodingsWithAndWithoutBom(string name, bool bom)
    {
        Encoding e = name switch { "UTF-8" => new UTF8Encoding(true), "UTF-16 LE" => new UnicodeEncoding(false, bom), _ => new UnicodeEncoding(true, bom) };
        string text = "Name;Price\r\nCafé;1,5\r\nGrüße;2,5\r\n";
        var d = CsvDecoder.Decode(B(e, text, bom));
        Assert.Equal(text, d.Text);
        Assert.Equal(name, d.EncodingName);
        Assert.Equal(bom, d.HadBom);
    }

    [Fact]
    public void ValidUtf8WithoutBomIsUtf8AndInvalidFallsBackToWindows1252()
    {
        var utf8 = CsvDecoder.Decode(B(new UTF8Encoding(false), "Café;Äpfel"));
        Assert.Equal("UTF-8", utf8.EncodingName);
        Assert.Equal("Café;Äpfel", utf8.Text);

        var legacy = CsvDecoder.Decode(B(Win1252, "Café;Äpfel;€5"));
        Assert.StartsWith("Windows-1252", legacy.EncodingName);
        Assert.Equal("Café;Äpfel;€5", legacy.Text);
        Assert.Contains("Not valid UTF-8", legacy.Note);
    }

    [Fact]
    public void PlainAsciiAndEmptyFilesAreSafe()
    {
        Assert.Equal("a,b\n1,2\n", CsvDecoder.Decode("a,b\n1,2\n"u8.ToArray()).Text);
        Assert.Equal("", CsvDecoder.Decode([]).Text);
    }

    [Fact]
    public void ChosenEncodingOverridesTheGuess()
    {
        var bytes = B(Encoding.GetEncoding(1251), "Привет;мир");
        var d = CsvDecoder.Decode(bytes, "Windows-1251 (Cyrillic)");
        Assert.Equal("Привет;мир", d.Text);
        var jp = CsvDecoder.Decode(B(Encoding.GetEncoding(932), "日本語;テスト"), "Shift-JIS (Japanese)");
        Assert.Equal("日本語;テスト", jp.Text);
        Assert.Equal("é", CsvDecoder.Decode(B(new UTF8Encoding(true), "é", true), "UTF-8").Text);       // the BOM is not left in the text
    }

    [Theory]
    [InlineData("CafÃ©", "Café")]
    [InlineData("naÃ¯ve and rÃ©sumÃ©", "naïve and résumé")]
    [InlineData("Itâ€™s fine", "It’s fine")]
    [InlineData("MÃ¼ller GmbH", "Müller GmbH")]
    [InlineData("Café stays; CafÃ© is repaired", "Café stays; Café is repaired")]
    [InlineData("Plain text 123", "Plain text 123")]
    [InlineData("Ärger", "Ärger")]
    public void RepairsTextSavedAsUtf8Twice(string input, string expected)
    {
        Assert.Equal(expected, CsvDecoder.RepairDoubleEncoding(input));
    }

    [Fact]
    public void RepairLeavesWhatItCannotReverseCleanAlone()
    {
        string odd = "Ã followed by nothing useful Ã!";
        Assert.Equal(odd, CsvDecoder.RepairDoubleEncoding(odd));
    }

    // ── Parser ───────────────────────────────────────────────────

    [Fact]
    public void ParsesQuotesDoubledQuotesAndLineBreaksInsideQuotes()
    {
        string text = "id,note,qty\r\n1,\"He said \"\"hi\"\", then left\",5\r\n2,\"line one\r\nline two\",6\r\n";
        var p = CsvParser.Parse(text, ',');
        Assert.Equal(3, p.Rows.Count);
        Assert.Equal(["1", "He said \"hi\", then left", "5"], p.Rows[1]);
        Assert.Equal("line one\r\nline two", p.Rows[2][1]);
        Assert.Equal(0, p.RaggedRows);
        Assert.False(p.HadStrayQuotes);
    }

    [Theory]
    [InlineData("a,b\r\n1,2\r\n")]
    [InlineData("a,b\n1,2\n")]
    [InlineData("a,b\r1,2\r")]
    [InlineData("a,b\r\n1,2")]
    [InlineData("a,b\n1,2\r\n")]
    public void AllLineEndingsAndAMissingFinalNewlineGiveTwoRows(string text)
    {
        var p = CsvParser.Parse(text, ',');
        Assert.Equal(2, p.Rows.Count);
        Assert.Equal(["1", "2"], p.Rows[1]);
    }

    [Fact]
    public void EmptyFieldsEmptyLinesAndRaggedRowsAreCounted()
    {
        var p = CsvParser.Parse("a,b,c\n1,,3\n\n,,\n4,5\n6,7,8,9\n", ',');
        Assert.Equal(5, p.Rows.Count);
        Assert.Equal(["1", "", "3"], p.Rows[1]);
        Assert.Equal(["", "", ""], p.Rows[2]);
        Assert.Equal(1, p.EmptyLines);
        Assert.Equal(2, p.RaggedRows);
    }

    [Fact]
    public void StrayAndMissingQuotesAreReportedNotFatal()
    {
        var stray = CsvParser.Parse("a,b\n5\" pipe,x\n", ',');
        Assert.Equal("5\" pipe", stray.Rows[1][0]);
        Assert.True(stray.HadStrayQuotes);
        var open = CsvParser.Parse("a,b\n\"never closed,x\n1,2\n", ',');
        Assert.True(open.EndedInsideQuote);
        Assert.Equal(2, open.Rows.Count);
    }

    [Fact]
    public void QuotedEmptyStringAndTrailingDelimiterAreFields()
    {
        var p = CsvParser.Parse("a;b;c;\n\"\";x;;\n", ';');
        Assert.Equal(["a", "b", "c", ""], p.Rows[0]);
        Assert.Equal(["", "x", "", ""], p.Rows[1]);
    }

    [Theory]
    [InlineData("id,name,qty\n1,Bolt,5\n2,Nut,7\n3,Washer,9\n", ',')]
    [InlineData("id;name;qty\n1;Bolt;5,5\n2;Nut;7,25\n3;Washer;9\n", ';')]
    [InlineData("id\tname\tqty\n1\tBolt\t5\n2\tNut\t7\n", '\t')]
    [InlineData("id|name|qty\n1|Bolt|5\n2|Nut|7\n", '|')]
    [InlineData("id;name;note\n1;Bolt;\"a, b, c\"\n2;Nut;\"d, e\"\n", ';')]
    [InlineData("id,name,note\n1,Bolt,\"a; b; c\"\n2,Nut,\"d; e\"\n", ',')]
    public void DetectsTheDelimiter(string text, char expected) => Assert.Equal(expected, CsvParser.DetectDelimiter(text));

    [Fact]
    public void SingleColumnFileGetsACommaAndNoFileCrashes()
    {
        Assert.Equal(',', CsvParser.DetectDelimiter("a\nb\nc\n"));
        Assert.Equal(',', CsvParser.DetectDelimiter(""));
        Assert.Empty(CsvParser.Parse("", ',').Rows);
    }

    [Fact]
    public void LargeFileParsesQuickly()
    {
        var sb = new StringBuilder("id;part;qty;price\n");
        for (int i = 0; i < 200_000; i++) sb.Append(i).Append(";00").Append(i).Append(';').Append(i % 50).Append(';').Append(i % 97).Append(",5\n");
        var sw = System.Diagnostics.Stopwatch.StartNew();
        char d = CsvParser.DetectDelimiter(sb.ToString());
        var p = CsvParser.Parse(sb.ToString(), d);
        sw.Stop();
        Assert.Equal(';', d);
        Assert.Equal(200_001, p.Rows.Count);
        Assert.True(sw.Elapsed < TimeSpan.FromSeconds(8), sw.Elapsed.ToString());
    }

    // ── Numbers and risk ─────────────────────────────────────────

    [Theory]
    [InlineData("00123", Risk.LeadingZeros)]
    [InlineData("12345678901234567", Risk.LongNumber)]
    [InlineData("1E5", Risk.ScientificLook)]
    [InlineData("2.5e-3", Risk.ScientificLook)]
    [InlineData("3-4", Risk.DateLook)]
    [InlineData("1/2", Risk.DateLook | Risk.FractionLook)]
    [InlineData("1.5.2020", Risk.DateLook)]
    [InlineData("12.5", Risk.None)]
    [InlineData("ABC-123", Risk.None)]
    [InlineData("0", Risk.None)]
    [InlineData("0.5", Risk.None)]
    [InlineData("", Risk.None)]
    public void RiskOfFindsWhatExcelWouldDamage(string value, Risk expected) => Assert.Equal(expected, ColumnAnalyzer.RiskOf(value));

    [Theory]
    [InlineData("1234", DecimalMark.Dot, "1234")]
    [InlineData("1,234.56", DecimalMark.Dot, "1234.56")]
    [InlineData("-12.5", DecimalMark.Dot, "-12.5")]
    [InlineData(".5", DecimalMark.Dot, "0.5")]
    [InlineData("1.234,56", DecimalMark.Comma, "1234.56")]
    [InlineData("12,5", DecimalMark.Comma, "12.5")]
    [InlineData("-0,5", DecimalMark.Comma, "-0.5")]
    [InlineData("1 234,5", DecimalMark.Comma, "1234.5")]
    [InlineData("1'234.50", DecimalMark.Dot, "1234.50")]
    public void ParsesCleanNumbers(string v, DecimalMark mark, string invariant)
    {
        Assert.True(ColumnAnalyzer.TryParseNumber(v, mark, out _, out string inv));
        Assert.Equal(invariant, inv);
    }

    [Theory]
    [InlineData("007", DecimalMark.Dot)]
    [InlineData("1e5", DecimalMark.Dot)]
    [InlineData("+5", DecimalMark.Dot)]
    [InlineData("1234567890123456", DecimalMark.Dot)]
    [InlineData("12,5", DecimalMark.Dot)]
    [InlineData("12.5", DecimalMark.Comma)]
    [InlineData("1,23,45", DecimalMark.Dot)]
    [InlineData("abc", DecimalMark.Dot)]
    [InlineData("", DecimalMark.Dot)]
    [InlineData("12-3", DecimalMark.Dot)]
    public void RefusesAnythingThatIsNotACleanNumber(string v, DecimalMark mark) => Assert.False(ColumnAnalyzer.TryParseNumber(v, mark, out _, out _));

    [Fact]
    public void DecimalMarkIsDetectedFromTheValues()
    {
        Assert.Equal(DecimalMark.Comma, ColumnAnalyzer.DetectMark(["1,5", "2,25", "10,75"], ','));
        Assert.Equal(DecimalMark.Dot, ColumnAnalyzer.DetectMark(["1.5", "2.25", "1,234.50"], ';'));
        Assert.Equal(DecimalMark.Comma, ColumnAnalyzer.DetectMark(["1.234,56", "5"], ','));
        Assert.Equal(DecimalMark.Comma, ColumnAnalyzer.DetectMark(["12", "40"], ';'));          // no evidence: a semicolon file is probably European
        Assert.Equal(DecimalMark.Dot, ColumnAnalyzer.DetectMark(["12", "40"], ','));
    }

    // ── A whole file ─────────────────────────────────────────────

    private static byte[] EuropeanFile() => B(Win1252,
        "Teil-Nr;Bezeichnung;Menge;Preis;Datum\r\n" +
        "00123;Schraube M8 × 20;100;0,35;1-2\r\n" +
        "00456;Mutter \"M8\";2500;1.250,5;3-4\r\n" +
        "12345678901234567;Unterlegscheibe;7;12,00;5-6\r\n" +
        "1E5;Bolzen, lang;1;2,5;7-8\r\n");

    [Fact]
    public void ReadsAEuropeanWindowsFileExactly()
    {
        var doc = CsvDocument.FromBytes("t.csv", EuropeanFile(), new CsvSettings());
        Assert.StartsWith("Windows-1252", doc.Decoded.EncodingName);
        Assert.Equal(';', doc.Delimiter);
        Assert.True(doc.DelimiterDetected);
        Assert.Equal(5, doc.Rows.Count);
        Assert.Equal(4, doc.DataRows);
        Assert.Equal(["Teil-Nr", "Bezeichnung", "Menge", "Preis", "Datum"], doc.Columns.Select(c => c.Name));
        Assert.Equal("Schraube M8 × 20", doc.Rows[1][1]);
        Assert.Equal("Mutter \"M8\"", doc.Rows[2][1]);
        Assert.Equal("Bolzen, lang", doc.Rows[4][1]);
        Assert.Equal([ColumnKind.Text, ColumnKind.Text, ColumnKind.Number, ColumnKind.Number, ColumnKind.Text], doc.Columns.Select(c => c.Kind));
        Assert.Equal(DecimalMark.Comma, doc.Columns[3].Mark);
        Assert.Equal(4, doc.Columns[0].AtRisk);                 // 00123, 00456, the 17-digit number, 1E5
        Assert.Equal(4, doc.Columns[4].AtRisk);                 // all four look like dates
        Assert.True(doc.CellsAtRisk >= 8);
    }

    [Fact]
    public void ChangingTheSettingsRereadsTheSameBytes()
    {
        var bytes = EuropeanFile();
        var wrong = CsvDocument.FromBytes("t.csv", bytes, new CsvSettings(Delimiter: ',', Mark: DecimalMark.Dot, HasHeader: false));
        Assert.Equal(',', wrong.Delimiter);
        Assert.False(wrong.DelimiterDetected);
        Assert.Equal(5, wrong.DataRows);
        Assert.Equal("Column 1", wrong.Columns[0].Name);
        Assert.All(wrong.Columns, c => Assert.Equal(ColumnKind.Text, c.Kind));      // a semicolon file split at commas is mostly one wide column of text
        var forced = CsvDocument.FromBytes("t.csv", bytes, new CsvSettings(Encoding: "ISO-8859-1 (Latin 1)"));
        Assert.Equal("ISO-8859-1 (Latin 1)", forced.Decoded.EncodingName);
    }

    [Fact]
    public void DoubleEncodedFileIsRepairedOnlyWhenAsked()
    {
        var bytes = B(new UTF8Encoding(false), "name;city\nMÃ¼ller;KÃ¶ln\n");
        Assert.Equal("Müller", CsvDocument.FromBytes("t.csv", bytes, new CsvSettings()).Rows[1][0]);
        var off = CsvDocument.FromBytes("t.csv", bytes, new CsvSettings(RepairDoubleEncoding: false));
        Assert.Equal("MÃ¼ller", off.Rows[1][0]);
        Assert.True(CsvDocument.FromBytes("t.csv", bytes, new CsvSettings()).RepairedMarkers > 0);
    }

    [Fact]
    public void SourceFileIsNeverChangedAndOutputCannotReplaceIt()
    {
        string path = Path.Combine(_dir, "source.csv");
        File.WriteAllBytes(path, EuropeanFile());
        var before = File.ReadAllBytes(path);
        var doc = CsvDocument.Load(path);
        string xlsx = doc.SuggestedName(".xlsx");
        CsvWriter.SaveXlsx(xlsx, doc.Rows, doc.Columns, true);
        CsvWriter.SaveCsv(doc.SuggestedName(".csv"), CsvWriter.ToCsv(doc.Rows, doc.Columns, new ExportOptions(',', true, false, false, true)), true);
        Assert.Equal(before, File.ReadAllBytes(path));
        Assert.True(doc.IsSameFile(path.ToUpperInvariant()));
        Assert.False(doc.IsSameFile(xlsx));
        Assert.EndsWith("source-clean.xlsx", xlsx);
    }

    [Fact]
    public void MissingAndOversizedFilesAreReported()
    {
        Assert.Throws<FileNotFoundException>(() => CsvDocument.Load(Path.Combine(_dir, "nope.csv")));
        string empty = Path.Combine(_dir, "empty.csv");
        File.WriteAllBytes(empty, []);
        var doc = CsvDocument.Load(empty);
        Assert.Empty(doc.Rows);
        Assert.Empty(doc.Columns);
    }

    // ── Writing ──────────────────────────────────────────────────

    [Fact]
    public void CsvOutputQuotesAndRoundTrips()
    {
        var doc = CsvDocument.FromBytes("t.csv", EuropeanFile(), new CsvSettings());
        string csv = CsvWriter.ToCsv(doc.Rows, doc.Columns, new ExportOptions(',', true, false, false, true));
        var back = CsvParser.Parse(csv, ',').Rows;
        Assert.Equal(doc.Rows.Count, back.Count);
        for (int r = 0; r < back.Count; r++) Assert.Equal(doc.Rows[r], back[r]);        // every cell identical, including quotes and commas

        string semi = CsvWriter.ToCsv(doc.Rows, doc.Columns, new ExportOptions(';', true, false, false, true));
        Assert.Equal(doc.Rows[2][1], CsvParser.Parse(semi, ';').Rows[2][1]);
    }

    [Fact]
    public void DecimalCommasBecomeDotsOnlyInNumberColumns()
    {
        var doc = CsvDocument.FromBytes("t.csv", EuropeanFile(), new CsvSettings());
        var rows = CsvParser.Parse(CsvWriter.ToCsv(doc.Rows, doc.Columns, new ExportOptions(',', false, true, false, true)), ',').Rows;
        Assert.Equal("0.35", rows[1][3]);
        Assert.Equal("1250.5", rows[2][3]);
        Assert.Equal("Bolzen, lang", rows[4][1]);
        Assert.Equal("00123", rows[1][0]);                   // text column untouched
        Assert.Equal("Preis", rows[0][3]);                   // header untouched
    }

    [Fact]
    public void ExcelProtectionWritesRiskyTextAsFormulaText()
    {
        var doc = CsvDocument.FromBytes("t.csv", EuropeanFile(), new CsvSettings());
        string csv = CsvWriter.ToCsv(doc.Rows, doc.Columns, new ExportOptions(',', false, false, true, true));
        Assert.Contains("=\"00123\"", csv);
        Assert.Contains("=\"1E5\"", csv);
        Assert.Contains("\r\nTeil-Nr,", "\r\nTeil-Nr,Bezeichnung".Insert(0, "\r\n"));
        Assert.StartsWith("Teil-Nr,Bezeichnung", csv);        // the header is never wrapped
        Assert.DoesNotContain("=\"Schraube", csv);            // safe text is not wrapped
    }

    private static XDocument Sheet(string path)
    {
        using var zip = ZipFile.OpenRead(path);
        using var s = zip.GetEntry("xl/worksheets/sheet1.xml")!.Open();
        return XDocument.Load(s);
    }

    [Fact]
    public void XlsxHasTheRightPartsTextCellsAndNumbers()
    {
        var doc = CsvDocument.FromBytes("t.csv", EuropeanFile(), new CsvSettings());
        string path = Path.Combine(_dir, "out.xlsx");
        CsvWriter.SaveXlsx(path, doc.Rows, doc.Columns, true);

        using (var zip = ZipFile.OpenRead(path))
            foreach (var part in new[] { "[Content_Types].xml", "_rels/.rels", "xl/workbook.xml", "xl/_rels/workbook.xml.rels", "xl/styles.xml", "xl/worksheets/sheet1.xml" })
                Assert.NotNull(zip.GetEntry(part));

        XNamespace ns = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
        var cells = Sheet(path).Descendants(ns + "c").ToDictionary(c => (string)c.Attribute("r")!);
        Assert.Equal("00123", cells["A2"].Descendants(ns + "t").Single().Value);
        Assert.Equal("inlineStr", (string)cells["A2"].Attribute("t")!);
        Assert.Equal("1", (string)cells["A2"].Attribute("s")!);                 // text format
        Assert.Equal("2", (string)cells["A1"].Attribute("s")!);                 // bold header
        Assert.Equal("12345678901234567", cells["A4"].Descendants(ns + "t").Single().Value);
        Assert.Equal("1E5", cells["A5"].Descendants(ns + "t").Single().Value);
        Assert.Equal("100", cells["C2"].Element(ns + "v")!.Value);
        Assert.Null(cells["C2"].Attribute("t"));
        Assert.Equal("0.35", cells["D2"].Element(ns + "v")!.Value);
        Assert.Equal("1250.5", cells["D3"].Element(ns + "v")!.Value);
        Assert.Equal("1-2", cells["E2"].Descendants(ns + "t").Single().Value);
        Assert.Equal("Mutter \"M8\"", cells["B3"].Descendants(ns + "t").Single().Value);
        Assert.NotNull(Sheet(path).Descendants(ns + "pane").SingleOrDefault());
    }

    [Fact]
    public void XlsxEscapesXmlAndDropsInvalidCharacters()
    {
        var rows = new List<string[]> { new[] { "h" }, new[] { "a & b < c > d \"q\" \u0001 \u0007 tab\there" }, new[] { "line1\nline2" } };
        var cols = new[] { new ColumnInfo("h", ColumnKind.Text, DecimalMark.Dot, 0, Risk.None, 2) };
        string path = Path.Combine(_dir, "esc.xlsx");
        CsvWriter.SaveXlsx(path, rows, cols, true);
        XNamespace ns = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
        var t = Sheet(path).Descendants(ns + "t").Select(x => x.Value).ToList();
        Assert.Equal("a & b < c > d \"q\"   tab\there", t[1]);
        Assert.DoesNotContain('\u0001', t[1]);
        Assert.Equal("line1\nline2", t[2]);
    }

    [Fact]
    public void XlsxCellNamesAndLimits()
    {
        Assert.Equal("A1", CsvWriter.CellName(0, 1));
        Assert.Equal("Z9", CsvWriter.CellName(25, 9));
        Assert.Equal("AA10", CsvWriter.CellName(26, 10));
        Assert.Equal("AZ1", CsvWriter.CellName(51, 1));
        Assert.Equal("BA1", CsvWriter.CellName(52, 1));
        Assert.Equal("XFD1", CsvWriter.CellName(16383, 1));

        var rows = Enumerable.Range(0, CsvWriter.ExcelMaxRows + 1).Select(_ => new[] { "x" }).ToList();
        var cols = new[] { new ColumnInfo("c", ColumnKind.Text, DecimalMark.Dot, 0, Risk.None, 1) };
        var ex = Assert.Throws<InvalidOperationException>(() => CsvWriter.SaveXlsx(Path.Combine(_dir, "big.xlsx"), rows, cols, false));
        Assert.Contains("1,048,576", ex.Message);
        Assert.False(File.Exists(Path.Combine(_dir, "big.xlsx")));
        Assert.False(File.Exists(Path.Combine(_dir, "big.xlsx.tmp")));
    }

    [Fact]
    public void NumberColumnWithABadValueStaysTextSoNothingIsLost()
    {
        var doc = CsvDocument.FromBytes("t.csv", "qty;note\n5;a\n6;b\nseven;c\n"u8.ToArray(), new CsvSettings());
        Assert.Equal(ColumnKind.Text, doc.Columns[0].Kind);
        doc.SetKind(0, ColumnKind.Number);                                  // forced by the user: bad cells are written as text, not dropped
        string path = Path.Combine(_dir, "forced.xlsx");
        CsvWriter.SaveXlsx(path, doc.Rows, doc.Columns, true);
        XNamespace ns = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
        var cells = Sheet(path).Descendants(ns + "c").ToDictionary(c => (string)c.Attribute("r")!);
        Assert.Equal("5", cells["A2"].Element(ns + "v")!.Value);
        Assert.Equal("seven", cells["A4"].Descendants(ns + "t").Single().Value);
    }
}
