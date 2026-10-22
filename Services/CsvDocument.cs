using System.IO;

namespace CSVRescue.Services;

/// <param name="Encoding">"Auto" or a name from <see cref="CsvDecoder.Choices"/>.</param>
/// <param name="Delimiter">null = detect.</param>
public sealed record CsvSettings(string Encoding = "Auto", char? Delimiter = null, DecimalMark Mark = DecimalMark.Auto, bool HasHeader = true, bool RepairDoubleEncoding = true);

/// <summary>A text file opened for cleaning. Everything is worked out from the bytes; the file on disk is never written.</summary>
public sealed class CsvDocument
{
    public const long MaxBytes = 300L * 1024 * 1024;

    public required string Path { get; init; }
    public required Decoded Decoded { get; init; }
    public required string Text { get; init; }
    public required char Delimiter { get; init; }
    public required ParseResult Parsed { get; init; }
    public required ColumnInfo[] Columns { get; set; }
    public required CsvSettings Settings { get; init; }
    public int RepairedMarkers { get; init; }
    public bool DelimiterDetected { get; init; }

    public IReadOnlyList<string[]> Rows => Parsed.Rows;
    public int DataRows => Math.Max(0, Rows.Count - (Settings.HasHeader ? 1 : 0));
    public int CellsAtRisk => Columns.Where(c => c.Kind == ColumnKind.Text).Sum(c => c.AtRisk);

    public static CsvDocument Load(string path, CsvSettings? settings = null)
    {
        settings ??= new CsvSettings();
        var info = new FileInfo(path);
        if (!info.Exists) throw new FileNotFoundException("That file does not exist.", path);
        if (info.Length > MaxBytes) throw new IOException($"The file is {info.Length / 1024 / 1024:N0} MB; CSVRescue opens files up to {MaxBytes / 1024 / 1024} MB.");
        return FromBytes(path, File.ReadAllBytes(path), settings);
    }

    public static CsvDocument FromBytes(string path, byte[] data, CsvSettings settings)
    {
        var decoded = CsvDecoder.Decode(data, settings.Encoding);
        string text = decoded.Text;
        int markers = 0;
        if (settings.RepairDoubleEncoding)
        {
            markers = CsvDecoder.MojibakeCount(text);
            if (markers > 0) text = CsvDecoder.RepairDoubleEncoding(text);
        }
        bool detected = settings.Delimiter == null;
        char delimiter = settings.Delimiter ?? CsvParser.DetectDelimiter(text);
        var parsed = CsvParser.Parse(text, delimiter);
        var cols = ColumnAnalyzer.Analyze(parsed.Rows, settings.HasHeader, settings.Mark, delimiter);
        return new CsvDocument { Path = path, Decoded = decoded, Text = text, Delimiter = delimiter, Parsed = parsed, Columns = cols, Settings = settings, RepairedMarkers = markers, DelimiterDetected = detected };
    }

    public void SetKind(int column, ColumnKind kind)
    {
        var c = Columns[column];
        Columns[column] = c with { Kind = kind };
    }

    public string SuggestedName(string extension)
    {
        string dir = System.IO.Path.GetDirectoryName(Path) ?? "";
        string name = System.IO.Path.GetFileNameWithoutExtension(Path) + "-clean" + extension;
        return System.IO.Path.Combine(dir, name);
    }

    /// <summary>The output may never replace the source file.</summary>
    public bool IsSameFile(string other) =>
        string.Equals(System.IO.Path.GetFullPath(Path), System.IO.Path.GetFullPath(other), StringComparison.OrdinalIgnoreCase);
}
