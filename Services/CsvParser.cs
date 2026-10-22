using System.Text;

namespace CSVRescue.Services;

public sealed record ParseResult(List<string[]> Rows, int RaggedRows, int EmptyLines, bool HadStrayQuotes, bool EndedInsideQuote);

/// <summary>Reads delimited text: quoted fields, doubled quotes, line breaks inside quotes, CRLF / LF / CR.</summary>
public static class CsvParser
{
    public static readonly char[] Candidates = [',', ';', '\t', '|'];

    public static ParseResult Parse(string text, char delimiter, int maxRows = int.MaxValue, bool skipEmptyLines = true)
    {
        var rows = new List<string[]>();
        var fields = new List<string>();
        var sb = new StringBuilder();
        bool inQuotes = false, fieldQuoted = false, stray = false, any = false;
        int empty = 0;

        void EndField() { fields.Add(sb.ToString()); sb.Clear(); fieldQuoted = false; }
        void EndRow()
        {
            EndField();
            bool blank = fields.Count == 1 && fields[0].Length == 0 && !any;
            if (blank) { empty++; if (!skipEmptyLines) rows.Add([""]); }
            else rows.Add(fields.ToArray());
            fields.Clear();
            any = false;
        }

        for (int i = 0; i < text.Length && rows.Count < maxRows; i++)
        {
            char c = text[i];
            if (inQuotes)
            {
                if (c == '"')
                {
                    if (i + 1 < text.Length && text[i + 1] == '"') { sb.Append('"'); i++; }
                    else inQuotes = false;
                }
                else sb.Append(c);
                continue;
            }
            if (c == '"')
            {
                if (sb.Length == 0 && !fieldQuoted) { inQuotes = true; fieldQuoted = true; any = true; }
                else { sb.Append('"'); stray = true; }                          // a quote in the middle of an unquoted field
            }
            else if (c == delimiter) { any = true; EndField(); }
            else if (c == '\r') { if (i + 1 < text.Length && text[i + 1] == '\n') i++; EndRow(); }
            else if (c == '\n') EndRow();
            else
            {
                if (fieldQuoted) { stray = true; }                              // text after the closing quote: keep it
                sb.Append(c);
                any = true;
            }
        }
        bool endedInQuote = inQuotes;
        if (rows.Count < maxRows && (sb.Length > 0 || fields.Count > 0 || fieldQuoted)) EndRow();

        int width = rows.Count == 0 ? 0 : rows.GroupBy(r => r.Length).OrderByDescending(g => g.Count()).ThenByDescending(g => g.Key).First().Key;
        int ragged = rows.Count(r => r.Length != width);
        return new ParseResult(rows, ragged, empty, stray, endedInQuote);
    }

    /// <summary>
    /// Picks the delimiter that splits the first records into the same number of columns (two or more) most consistently.
    /// A single-column file gets a comma.
    /// </summary>
    public static char DetectDelimiter(string text)
    {
        string sample = text.Length > 200_000 ? text[..200_000] : text;
        char best = ',';
        double bestScore = 0;
        foreach (char d in Candidates)
        {
            var rows = Parse(sample, d, maxRows: 200).Rows;
            if (rows.Count == 0) continue;
            var groups = rows.GroupBy(r => r.Length).OrderByDescending(g => g.Count()).ThenByDescending(g => g.Key).First();
            if (groups.Key < 2) continue;
            double consistency = (double)groups.Count() / rows.Count;
            double score = consistency * Math.Log(groups.Key + 1) * (consistency >= 0.8 ? 2 : 1);
            if (score > bestScore + 1e-9) { best = d; bestScore = score; }
        }
        return best;
    }

    public static string Name(char d) => d switch { ',' => "Comma", ';' => "Semicolon", '\t' => "Tab", '|' => "Pipe", _ => d.ToString() };
}
