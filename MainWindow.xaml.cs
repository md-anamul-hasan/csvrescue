using System;
using System.Windows;
using System.Windows.Interop;
using CSVRescue.Services;
using CSVRescue.ViewModels;

namespace CSVRescue;

public partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();
        Mdah.Windowing.MaximizeFix.Attach(this);
        Mdah.Windowing.ResponsiveScale.Attach(this, Body);
        // "CSVRescue.exe <file>" (or Open with / drag onto the exe) opens the file at once
        Loaded += (_, _) =>
        {
            var args = Environment.GetCommandLineArgs();
            if (args.Length > 1 && DataContext is MainViewModel vm) vm.Open(args[1]);
        };
    }

    protected override void OnSourceInitialized(EventArgs e)
    {
        base.OnSourceInitialized(e);
        NativeMethods.EnableWindows11DarkTheme(new WindowInteropHelper(this).Handle);
    }

    private void OnDragOverFile(object sender, DragEventArgs e)
    {
        e.Effects = e.Data.GetDataPresent(DataFormats.FileDrop) ? DragDropEffects.Copy : DragDropEffects.None;
        e.Handled = true;
    }

    private void OnDropFile(object sender, DragEventArgs e)
    {
        if (e.Data.GetData(DataFormats.FileDrop) is string[] { Length: > 0 } files && DataContext is MainViewModel vm)
            vm.Open(files[0]);
    }
}
