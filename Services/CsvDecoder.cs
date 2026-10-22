using System.Text;
using System.Text.RegularExpressions;

namespace CSVRescue.Services;

public sealed record Decoded(string Text, string EncodingName, bool HadBom, string Note);

/// <summary>Finds out how a text file is encoded and turns its bytes into text. The file itself is never changed.</summary>
public static class CsvDecoder
{
    static CsvDecoder() => Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);

    /// <summary>The encodings offered in the picker, in the order shown. "Auto" is not a real encoding.</summary>
    public static readonly (string Name, int CodePage)[] Choices =
    [
        ("UTF-8", 65001), ("UTF-16 LE", 1200), ("UTF-16 BE", 1201), ("Windows-1252 (Western Europe)", 1252), ("ISO-8859-1 (Latin 1)", 28591),
        ("Windows-1250 (Central Europe)", 1250), ("ISO-8859-2 (Central Europe)", 28592), ("Windows-1251 (Cyrillic)", 1251), ("Windows-1253 (Greek)", 1253),
        ("Windows-1254 (Turkish)", 1254), ("Windows-1257 (Baltic)", 1257), ("Shift-JIS (Japanese)", 932), ("GBK (Simplified Chinese)", 936),
        ("Big5 (Traditional Chinese)", 950), ("EUC-KR (Korean)", 949), ("IBM437 (old DOS)", 437),
    ];

    public static Decoded Decode(byte[] data, string? forced = null)
    {
        if (forced != null && forced != "Auto")
        {
            int cp = Choices.First(c => c.Name == forced).CodePage;
            return FromCodePage(data, cp, forced, "Chosen by you.");
        }
        if (data.Length >= 3 && data[0] == 0xEF && data[1] == 0xBB && data[2] == 0xBF) return Make(data[3..], Encoding.UTF8, "UTF-8", true, "UTF-8 with a byte order mark.");
        if (data.Length >= 4 && data[0] == 0xFF && data[1] == 0xFE && data[2] == 0 && data[3] == 0) return Make(data[4..], new UTF32Encoding(false, false), "UTF-32 LE", true, "UTF-32 with a byte order mark.");
        if (data.Length >= 2 && data[0] == 0xFF && data[1] == 0xFE) return Make(data[2..], Encoding.Unicode, "UTF-16 LE", true, "UTF-16 with a byte order mark.");
        if (data.Length >= 2 && data[0] == 0xFE && data[1] == 0xFF) return Make(data[2..], Encoding.BigEndianUnicode, "UTF-16 BE", true, "UTF-16 with a byte order mark.");

        int zerosEven = 0, zerosOdd = 0, probe = Math.Min(data.Length, 65536);
        for (int i = 0; i < probe; i++) if (data[i] == 0) { if (i % 2 == 0) zerosEven++; else zerosOdd++; }
        if (probe >= 4 && zerosOdd > probe / 8 && zerosOdd > zerosEven * 4) return Make(data, Encoding.Unicode, "UTF-16 LE", false, "UTF-16 without a byte order mark (most bytes are zero).");
        if (probe >= 4 && zerosEven > probe / 8 && zerosEven > zerosOdd * 4) return Make(data, Encoding.BigEndianUnicode, "UTF-16 BE", false, "UTF-16 without a byte order mark (most bytes are zero).");

        try
        {
            string text = new UTF8Encoding(false, true).GetString(data);
            bool ascii = data.All(b => b < 0x80);
            return new Decoded(text, "UTF-8", false, ascii ? "Plain ASCII text (valid as UTF-8 and as every Western encoding)." : "Valid UTF-8.");
        }
        catch (DecoderFallbackException)
        {
            return Make(data, Encoding.GetEncoding(1252), "Windows-1252 (Western Europe)", false, "Not valid UTF-8, so the old Windows Western encoding is assumed. Pick another encoding if letters look wrong.");
        }
    }

    private static Decoded FromCodePage(byte[] data, int codePage, string name, string note)
    {
        int skip = 0;
        if (codePage == 65001 && data.Length >= 3 && data[0] == 0xEF && data[1] == 0xBB && data[2] == 0xBF) skip = 3;
        if ((codePage == 1200 && data.Length >= 2 && data[0] == 0xFF && data[1] == 0xFE) || (codePage == 1201 && data.Length >= 2 && data[0] == 0xFE && data[1] == 0xFF)) skip = 2;
        var enc = codePage == 65001 ? new UTF8Encoding(false, false) : Encoding.GetEncoding(codePage);
        return new Decoded(enc.GetString(data, skip, data.Length - skip), name, skip > 0, note);
    }

    private static Decoded Make(byte[] data, Encoding enc, string name, bool bom, string note) => new(enc.GetString(data), name, bom, note);

    // ── Text that was saved as UTF-8 twice ───────────────────────

    private static readonly Regex MojibakeMarker = new("[ÃÂ][\u0080-¿]|â€|Ã[ -ÿ]", RegexOptions.Compiled);

    public static int MojibakeCount(string text) => MojibakeMarker.Matches(text).Count;

    /// <summary>
    /// Repairs "é shown as Ã©": the text was UTF-8, was read as Windows-1252 and saved as UTF-8 again. Returns the input when
    /// reversing the mistake is not clean (so a file that legitimately contains such letters is not damaged).
    /// </summary>
    public static string RepairDoubleEncoding(string text)
    {
        if (MojibakeCount(text) == 0) return text;
        try
        {
            var cp1252 = Encoding.GetEncoding(1252, EncoderFallback.ExceptionFallback, DecoderFallback.ExceptionFallback);
            var sb = new StringBuilder(text.Length);
            int i = 0;
            // repair run by run, so clean text around the damaged words is left alone
            foreach (Match m in Regex.Matches(text, "(?:[ÃÂâ][\u0080-ÿŒœŠšŸŽžƒˆ˜–—‘-„†-•…‰‹›€™]+)+"))
            {
                sb.Append(text, i, m.Index - i);
                i = m.Index + m.Length;
                try { sb.Append(new UTF8Encoding(false, true).GetString(cp1252.GetBytes(m.Value))); }
                catch (Exception ex) when (ex is EncoderFallbackException or DecoderFallbackException or ArgumentException) { sb.Append(m.Value); }
            }
            sb.Append(text, i, text.Length - i);
            return sb.ToString();
        }
        catch (Exception ex) when (ex is EncoderFallbackException or DecoderFallbackException or ArgumentException) { return text; }
    }
}
