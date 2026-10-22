using System.Globalization;
using System.Text.RegularExpressions;

namespace CSVRescue.Services;

public enum ColumnKind { Text, Number }
public enum DecimalMark { Auto, Dot, Comma }

/// <summary>Why a value would be damaged if Excel opened it as a CSV.</summary>
[Flags]
public enum Risk { None = 0, LeadingZeros = 1, LongNumber = 2, ScientificLook = 4, DateLook = 8, FractionLook = 16 }

public sealed record ColumnInfo(string Name, ColumnKind Kind, DecimalMark Mark, int AtRisk, Risk Risks, int NonEmpty);

public static class ColumnAnalyzer
{
    private static readonly Regex DotNumber = new(@"^-?(?:0|[1-9]\d{0,2}(?:,\d{3})+|[1-9]\d*)(?:\.\d+)?$|^-?0\.\d+$|^-?\.\d+$", RegexOptions.Compiled);
    private static readonly Regex CommaNumber = new(@"^-?(?:0|[1-9]\d{0,2}(?:\.\d{3})+|[1-9]\d*)(?:,\d+)?$|^-?0,\d+$|^-?,\d+$", RegexOptions.Compiled);
    private static readonly Regex Scientific = new(@"^[+-]?\d+(?:[.,]\d+)?[eE][+-]?\d+$", RegexOptions.Compiled);
    private static readonly Regex DateLike = new(@"^\d{1,4}[-/.]\d{1,2}(?:[-/.]\d{1,4})?$", RegexOptions.Compiled);
    private static readonly Regex Fraction = new(@"^\d+\s*/\s*\d+$", RegexOptions.Compiled);

    /// <summary>What Excel would do to this text when it opens a CSV (turn it into a number or a date).</summary>
    public static Risk RiskOf(string v)
    {
        string s = v.Trim();
        if (s.Length == 0) return Risk.None;
        var r = Risk.None;
        if (s.Length > 1 && s[0] == '0' && s.All(char.IsAsciiDigit)) r |= Risk.LeadingZeros;
        if (s.Length > 15 && s.All(char.IsAsciiDigit)) r |= Risk.LongNumber;
        if (Scientific.IsMatch(s)) r |= Risk.ScientificLook;
        if (DateLike.IsMatch(s) && !LooksLikeNumber(s)) r |= Risk.DateLook;
        if (Fraction.IsMatch(s)) r |= Risk.FractionLook;
        return r;
    }

    private static bool LooksLikeNumber(string s) => double.TryParse(s, NumberStyles.Float, CultureInfo.InvariantCulture, out _) && s.Count(c => c == '.') <= 1 && !s.Contains('-', StringComparison.Ordinal) && !s.Contains('/', StringComparison.Ordinal);

    /// <summary>A clean number: no leading zeros, at most 15 digits, no signs other than "-". Returns false for anything else (it stays text).</summary>
    public static bool TryParseNumber(string value, DecimalMark mark, out decimal number, out string invariant)
    {
        number = 0; invariant = "";
        string s = value.Trim().Replace(" ", "").Replace(" ", "").Replace(" ", "").Replace("'", "");
        if (s.Length == 0) return false;
        bool ok = mark == DecimalMark.Comma ? CommaNumber.IsMatch(s) : DotNumber.IsMatch(s);
        if (!ok) return false;
        string norm = mark == DecimalMark.Comma ? s.Replace(".", "").Replace(',', '.') : s.Replace(",", "");
        if (norm.StartsWith('.') ) norm = "0" + norm;
        if (norm.StartsWith("-.", StringComparison.Ordinal)) norm = "-0" + norm[1..];
        if (norm.Count(char.IsAsciiDigit) > 15) return false;
        if (!decimal.TryParse(norm, NumberStyles.Float, CultureInfo.InvariantCulture, out number)) return false;
        invariant = norm;
        return true;
    }

    /// <summary>Which decimal mark the numbers of the file use. A comma followed by something other than three digits at the end is a decimal comma.</summary>
    public static DecimalMark DetectMark(IEnumerable<string> values, char delimiter)
    {
        int comma = 0, dot = 0;
        foreach (var v in values)
        {
            string s = v.Trim();
            if (Regex.IsMatch(s, @"^-?\d+,\d{1,2}$|^-?\d+,\d{4,}$|^-?\d{1,3}(\.\d{3})+,\d+$")) comma++;
            else if (Regex.IsMatch(s, @"^-?\d+\.\d{1,2}$|^-?\d+\.\d{4,}$|^-?\d{1,3}(,\d{3})+\.\d+$")) dot++;
        }
        if (comma == dot) return delimiter == ';' ? DecimalMark.Comma : DecimalMark.Dot;
        return comma > dot ? DecimalMark.Comma : DecimalMark.Dot;
    }

    public static ColumnInfo[] Analyze(IReadOnlyList<string[]> rows, bool hasHeader, DecimalMark requested, char delimiter)
    {
        int width = rows.Count == 0 ? 0 : rows.Max(r => r.Length);
        int first = hasHeader ? 1 : 0;
        var all = rows.Skip(first).SelectMany(r => r).Take(200000);
        var fileMark = requested != DecimalMark.Auto ? requested : DetectMark(all, delimiter);
        var cols = new ColumnInfo[width];
        for (int c = 0; c < width; c++)
        {
            string name = hasHeader && rows.Count > 0 && c < rows[0].Length && rows[0][c].Trim().Length > 0 ? rows[0][c].Trim() : $"Column {c + 1}";
            int nonEmpty = 0, risky = 0;
            Risk risks = Risk.None;
            bool numeric = true;
            for (int r = first; r < rows.Count; r++)
            {
                string v = c < rows[r].Length ? rows[r][c] : "";
                if (v.Trim().Length == 0) continue;
                nonEmpty++;
                var rk = RiskOf(v);
                if (rk != Risk.None) { risky++; risks |= rk; }
                if (numeric && !TryParseNumber(v, fileMark, out _, out _)) numeric = false;
            }
            cols[c] = new ColumnInfo(name, numeric && nonEmpty > 0 && (risks & ~Risk.None) == Risk.None ? ColumnKind.Number : ColumnKind.Text, fileMark, risky, risks, nonEmpty);
        }
        return cols;
    }

    public static string RiskText(Risk r)
    {
        var p = new List<string>();
        if (r.HasFlag(Risk.LeadingZeros)) p.Add("leading zeros");
        if (r.HasFlag(Risk.LongNumber)) p.Add("numbers over 15 digits");
        if (r.HasFlag(Risk.ScientificLook)) p.Add("look like 1E5");
        if (r.HasFlag(Risk.DateLook)) p.Add("look like dates");
        if (r.HasFlag(Risk.FractionLook)) p.Add("look like fractions");
        return string.Join(", ", p);
    }
}
