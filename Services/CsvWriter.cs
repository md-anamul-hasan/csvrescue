using System.IO;
using System.IO.Compression;
using System.Text;

namespace CSVRescue.Services;

public sealed record ExportOptions(char Delimiter, bool Utf8Bom, bool DecimalAsDot, bool ExcelTextProtect, bool HasHeader);

public static class CsvWriter
{
    public const int ExcelMaxRows = 1_048_576, ExcelMaxColumns = 16_384, ExcelMaxCellChars = 32_767;

    public static string ToCsv(IReadOnlyList<string[]> rows, ColumnInfo[] columns, ExportOptions o)
    {
        var sb = new StringBuilder();
        int width = columns.Length;
        for (int r = 0; r < rows.Count; r++)
        {
            bool header = o.HasHeader && r == 0;
            for (int c = 0; c < width; c++)
            {
                string v = c < rows[r].Length ? rows[r][c] : "";
                if (!header) v = Convert(v, columns[c], o);
                if (c > 0) sb.Append(o.Delimiter);
                bool formula = !header && o.ExcelTextProtect && columns[c].Kind == ColumnKind.Text && ColumnAnalyzer.RiskOf(v) != Risk.None;
                sb.Append(formula ? "=\"" + v.Replace("\"", "\"\"") + "\"" : Quote(v, o.Delimiter));
                if (formula) { /* ="00123" is read by Excel as the text 00123 */ }
            }
            sb.Append("\r\n");
        }
        return sb.ToString();
    }

    private static string Convert(string v, ColumnInfo col, ExportOptions o)
    {
        if (col.Kind == ColumnKind.Number && o.DecimalAsDot && ColumnAnalyzer.TryParseNumber(v, col.Mark, out _, out string inv)) return inv;
        return v;
    }

    private static string Quote(string v, char delimiter) =>
        v.Length > 0 && (v.Contains(delimiter) || v.Contains('"') || v.Contains('\n') || v.Contains('\r') || v[0] == ' ' || v[^1] == ' ')
            ? "\"" + v.Replace("\"", "\"\"") + "\"" : v;

    public static void SaveCsv(string path, string text, bool bom) => File.WriteAllText(path, text, new UTF8Encoding(bom));

    // ── Excel (.xlsx) without any library ────────────────────────

    public static void SaveXlsx(string path, IReadOnlyList<string[]> rows, ColumnInfo[] columns, bool hasHeader)
    {
        if (rows.Count > ExcelMaxRows) throw new InvalidOperationException($"Excel holds at most {ExcelMaxRows:N0} rows; this file has {rows.Count:N0}. Save it as CSV instead.");
        if (columns.Length > ExcelMaxColumns) throw new InvalidOperationException($"Excel holds at most {ExcelMaxColumns:N0} columns.");
        string tmp = path + ".tmp";
        try
        {
            using (var fs = File.Create(tmp))
            using (var zip = new ZipArchive(fs, ZipArchiveMode.Create))
            {
                Add(zip, "[Content_Types].xml", """<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>""");
                Add(zip, "_rels/.rels", """<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>""");
                Add(zip, "xl/workbook.xml", """<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Data" sheetId="1" r:id="rId1"/></sheets></workbook>""");
                Add(zip, "xl/_rels/workbook.xml.rels", """<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>""");
                Add(zip, "xl/styles.xml", """<?xml version="1.0" encoding="UTF-8" standalone="yes"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="3"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="49" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/><xf numFmtId="49" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1" applyNumberFormat="1"/></cellXfs></styleSheet>""");
                var entry = zip.CreateEntry("xl/worksheets/sheet1.xml", CompressionLevel.Optimal);
                using var w = new StreamWriter(entry.Open(), new UTF8Encoding(false));
                WriteSheet(w, rows, columns, hasHeader);
            }
            File.Move(tmp, path, overwrite: true);
        }
        finally { if (File.Exists(tmp)) File.Delete(tmp); }
    }

    private static void Add(ZipArchive zip, string name, string content)
    {
        var e = zip.CreateEntry(name, CompressionLevel.Optimal);
        using var w = new StreamWriter(e.Open(), new UTF8Encoding(false));
        w.Write(content);
    }

    private static void WriteSheet(StreamWriter w, IReadOnlyList<string[]> rows, ColumnInfo[] columns, bool hasHeader)
    {
        w.Write("""<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">""");
        if (hasHeader) w.Write("""<sheetViews><sheetView workbookViewId="0"><pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews>""");
        w.Write("<cols>");
        for (int c = 0; c < columns.Length; c++)
        {
            int len = columns[c].Name.Length;
            for (int r = 0; r < Math.Min(rows.Count, 200); r++) if (c < rows[r].Length) len = Math.Max(len, Math.Min(rows[r][c].Length, 60));
            w.Write($"<col min=\"{c + 1}\" max=\"{c + 1}\" width=\"{Math.Clamp(len + 2, 8, 60)}\" customWidth=\"1\"/>");
        }
        w.Write("</cols><sheetData>");
        for (int r = 0; r < rows.Count; r++)
        {
            w.Write($"<row r=\"{r + 1}\">");
            bool header = hasHeader && r == 0;
            for (int c = 0; c < columns.Length; c++)
            {
                string v = c < rows[r].Length ? rows[r][c] : "";
                if (v.Length == 0) continue;
                string cell = CellName(c, r + 1);
                if (!header && columns[c].Kind == ColumnKind.Number && ColumnAnalyzer.TryParseNumber(v, columns[c].Mark, out _, out string inv))
                    w.Write($"<c r=\"{cell}\"><v>{inv}</v></c>");
                else
                    w.Write($"<c r=\"{cell}\" t=\"inlineStr\" s=\"{(header ? 2 : 1)}\"><is><t xml:space=\"preserve\">{Escape(v.Length > ExcelMaxCellChars ? v[..ExcelMaxCellChars] : v)}</t></is></c>");
            }
            w.Write("</row>");
        }
        w.Write("</sheetData></worksheet>");
    }

    public static string CellName(int col, int row)
    {
        var s = new StringBuilder();
        for (int c = col + 1; c > 0; c = (c - 1) / 26) s.Insert(0, (char)('A' + (c - 1) % 26));
        return s.Append(row).ToString();
    }

    private static string Escape(string v)
    {
        var sb = new StringBuilder(v.Length + 8);
        foreach (char ch in v)
        {
            if (ch == '&') sb.Append("&amp;");
            else if (ch == '<') sb.Append("&lt;");
            else if (ch == '>') sb.Append("&gt;");
            else if (ch < 0x20 && ch != '\t' && ch != '\n' && ch != '\r') continue;     // not allowed in XML
            else if (ch is '￾' or '￿') continue;
            else sb.Append(ch);
        }
        return sb.ToString();
    }
}
