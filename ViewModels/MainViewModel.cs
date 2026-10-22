using System.Collections.ObjectModel;
using System.IO;
using System.Windows.Input;
using CSVRescue.Services;
using Microsoft.Win32;

namespace CSVRescue.ViewModels;

public sealed class ColumnRow : ViewModelBase
{
    private ColumnKind _kind;
    public ColumnRow(int index, ColumnInfo info) { Index = index; Name = info.Name; _kind = info.Kind; Risk = info.AtRisk > 0 ? $"{info.AtRisk:N0} cell(s): {ColumnAnalyzer.RiskText(info.Risks)}" : string.Empty; }
    public int Index { get; }
    public string Name { get; }
    public string Risk { get; }
    public bool HasRisk => Risk.Length > 0;
    public ColumnKind Kind { get => _kind; set { if (SetProperty(ref _kind, value)) OnPropertyChanged(nameof(KindText)); } }
    public string KindText => Kind == ColumnKind.Number ? "Number" : "Text";
}

public sealed record PreviewRow(string[] Cells, bool IsHeader);

public sealed class MainViewModel : ShellViewModel
{
    public const int PreviewRows = 60;
    public static readonly string[] EncodingChoices = ["Auto", .. CsvDecoder.Choices.Select(c => c.Name)];
    private static readonly char?[] DelimiterByIndex = [null, ',', ';', '\t', '|'];

    private byte[]? _bytes;
    private CsvDocument? _doc;
    private string _path = string.Empty, _statusText = "Ready";
    private int _encodingIndex, _delimiterIndex, _decimalIndex, _outDelimiterIndex;
    private bool _hasHeader = true, _repair = true, _utf8Bom = true, _decimalAsDot, _excelProtect, _loading, _isBusy;

    public MainViewModel()
    {
        BrowseCommand = new RelayCommand(Browse);
        ToggleKindCommand = new RelayCommand(p => { if (p is ColumnRow c && _doc != null) { var k = c.Kind == ColumnKind.Number ? ColumnKind.Text : ColumnKind.Number; c.Kind = k; _doc.SetKind(c.Index, k); }});
        ExportCsvCommand = new RelayCommand(() => Export(xlsx: false));
        ExportXlsxCommand = new RelayCommand(() => Export(xlsx: true));
    }

    public string StatusText { get => _statusText; private set => SetProperty(ref _statusText, value); }
    public bool IsBusy { get => _isBusy; private set { if (SetProperty(ref _isBusy, value)) System.Windows.Input.CommandManager.InvalidateRequerySuggested(); } }
    public string FilePath { get => _path; set => SetProperty(ref _path, value); }
    public string[] Encodings => EncodingChoices;
    public int EncodingIndex { get => _encodingIndex; set { if (SetProperty(ref _encodingIndex, value)) Reload(); } }
    public int DelimiterIndex { get => _delimiterIndex; set { if (SetProperty(ref _delimiterIndex, value)) Reload(); } }
    public int DecimalIndex { get => _decimalIndex; set { if (SetProperty(ref _decimalIndex, value)) Reload(); } }
    public bool HasHeader { get => _hasHeader; set { if (SetProperty(ref _hasHeader, value)) Reload(); } }
    public bool Repair { get => _repair; set { if (SetProperty(ref _repair, value)) Reload(); } }

    public int OutDelimiterIndex { get => _outDelimiterIndex; set => SetProperty(ref _outDelimiterIndex, value); }
    public bool Utf8Bom { get => _utf8Bom; set => SetProperty(ref _utf8Bom, value); }
    public bool DecimalAsDot { get => _decimalAsDot; set => SetProperty(ref _decimalAsDot, value); }
    public bool ExcelProtect { get => _excelProtect; set => SetProperty(ref _excelProtect, value); }

    public ObservableCollection<ColumnRow> Columns { get; } = new();
    public ObservableCollection<PreviewRow> Preview { get; } = new();

    public ICommand BrowseCommand { get; }
    public ICommand ToggleKindCommand { get; }
    public ICommand ExportCsvCommand { get; }
    public ICommand ExportXlsxCommand { get; }

    public bool HasFile => _doc != null;
    public string EncodingInfo => _doc == null ? string.Empty : $"Encoding: {_doc.Decoded.EncodingName}. {_doc.Decoded.Note}";
    public string FormatInfo => _doc == null ? string.Empty
        : $"Delimiter: {CsvParser.Name(_doc.Delimiter).ToLowerInvariant()}{(_doc.DelimiterDetected ? " (detected)" : "")}  ·  decimal mark: {(_doc.Columns.FirstOrDefault()?.Mark == DecimalMark.Comma ? "comma" : "dot")}  ·  {_doc.DataRows:N0} rows × {_doc.Columns.Length:N0} columns";
    public string Problems
    {
        get
        {
            if (_doc == null) return string.Empty;
            var p = new List<string>();
            if (_doc.RepairedMarkers > 0) p.Add($"Repaired {_doc.RepairedMarkers:N0} garbled letter group(s) (text saved as UTF-8 twice, such as é shown as Ã©).");
            if (_doc.Parsed.RaggedRows > 0) p.Add($"{_doc.Parsed.RaggedRows:N0} row(s) have a different number of columns than most rows.");
            if (_doc.Parsed.EmptyLines > 0) p.Add($"{_doc.Parsed.EmptyLines:N0} empty line(s) skipped.");
            if (_doc.Parsed.HadStrayQuotes) p.Add("Quotation marks in the middle of fields were kept as text.");
            if (_doc.Parsed.EndedInsideQuote) p.Add("The file ends inside a quoted field: a closing quote is missing.");
            int risky = _doc.Columns.Count(c => c.Kind == ColumnKind.Text && c.AtRisk > 0);
            if (risky > 0) p.Add($"Excel would change {_doc.CellsAtRisk:N0} cell(s) in {risky} column(s) when it opens this file as CSV (leading zeros, long numbers, date-like or 1E5-like values). CSVRescue keeps them as text.");
            return p.Count == 0 ? "No problems found." : string.Join("\n", p);
        }
    }
    public bool CanExportXlsx => _doc != null && _doc.Rows.Count <= CsvWriter.ExcelMaxRows;

    private void Browse()
    {
        var dlg = new OpenFileDialog { Title = "Choose a CSV or text file", Filter = "CSV and text (*.csv;*.txt;*.tsv;*.tab;*.dat)|*.csv;*.txt;*.tsv;*.tab;*.dat|All files (*.*)|*.*" };
        if (dlg.ShowDialog() == true) Open(dlg.FileName);
    }

    public void Open(string path)
    {
        FilePath = path;
        try
        {
            var info = new FileInfo(path);
            if (!info.Exists) { ShowBanner("That file does not exist.", "Warning"); return; }
            if (info.Length > CsvDocument.MaxBytes) { ShowBanner($"The file is {info.Length / 1024 / 1024:N0} MB; CSVRescue opens files up to {CsvDocument.MaxBytes / 1024 / 1024} MB.", "Warning"); return; }
            _bytes = File.ReadAllBytes(path);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            ShowBanner("Could not read the file: " + ex.Message, "Error");
            return;
        }
        _loading = true;
        EncodingIndex = 0; DelimiterIndex = 0; DecimalIndex = 0;
        _loading = false;
        Reload();
    }

    private CsvSettings Settings() => new(EncodingChoices[Math.Clamp(EncodingIndex, 0, EncodingChoices.Length - 1)], DelimiterByIndex[Math.Clamp(DelimiterIndex, 0, 4)],
        (DecimalMark)Math.Clamp(DecimalIndex, 0, 2), HasHeader, Repair);

    private async void Reload()
    {
        if (_loading || _bytes == null) return;
        var bytes = _bytes; var path = FilePath; var settings = Settings();
        IsBusy = true;
        StatusText = "Reading…";
        try
        {
            _doc = await Task.Run(() => CsvDocument.FromBytes(path, bytes, settings));
            Columns.Clear();
            for (int i = 0; i < _doc.Columns.Length; i++) Columns.Add(new ColumnRow(i, _doc.Columns[i]));
            Preview.Clear();
            foreach (var (row, i) in _doc.Rows.Take(PreviewRows).Select((r, i) => (r, i)))
                Preview.Add(new PreviewRow(Enumerable.Range(0, _doc.Columns.Length).Select(c => c < row.Length ? row[c] : "").ToArray(), HasHeader && i == 0));
            foreach (var n in new[] { nameof(HasFile), nameof(EncodingInfo), nameof(FormatInfo), nameof(Problems), nameof(CanExportXlsx) }) OnPropertyChanged(n);
            StatusText = $"{_doc.DataRows:N0} rows × {_doc.Columns.Length} columns";
        }
        catch (Exception ex) when (ex is IOException or InvalidDataException or ArgumentException or NotSupportedException)
        {
            StatusText = "Could not read the file";
            ShowBanner("Could not read the file: " + ex.Message, "Error");
        }
        finally { IsBusy = false; }
    }

    private void Export(bool xlsx)
    {
        if (_doc == null) return;
        var dlg = new SaveFileDialog
        {
            Title = xlsx ? "Save as Excel file" : "Save as CSV",
            FileName = Path.GetFileName(_doc.SuggestedName(xlsx ? ".xlsx" : ".csv")),
            InitialDirectory = Path.GetDirectoryName(_doc.Path),
            Filter = xlsx ? "Excel workbook (*.xlsx)|*.xlsx" : "CSV (*.csv)|*.csv",
        };
        if (dlg.ShowDialog() != true) return;
        if (_doc.IsSameFile(dlg.FileName)) { ShowBanner("CSVRescue never overwrites the file it opened. Choose another name.", "Warning"); return; }
        try
        {
            var doc = _doc;
            if (xlsx) CsvWriter.SaveXlsx(dlg.FileName, doc.Rows, doc.Columns, HasHeader);
            else
            {
                char d = new[] { ',', ';', '\t' }[Math.Clamp(OutDelimiterIndex, 0, 2)];
                CsvWriter.SaveCsv(dlg.FileName, CsvWriter.ToCsv(doc.Rows, doc.Columns, new ExportOptions(d, Utf8Bom, DecimalAsDot, ExcelProtect, HasHeader)), Utf8Bom);
            }
            ShowBanner("Saved " + Path.GetFileName(dlg.FileName), "Success");
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or InvalidOperationException)
        {
            ShowBanner("Could not save: " + ex.Message, "Error");
        }
    }
}
