using System.Windows;
using System.Windows.Media;

namespace TeamsPresenceBridge;

/// <summary>
/// A simple WPF modal color picker with R/G/B sliders and live preview.
/// </summary>
public partial class ColorPickerDialog : Window
{
    /// <summary>The selected color after the dialog is accepted.</summary>
    public Color SelectedColor { get; private set; }

    /// <summary>True if the user clicked OK.</summary>
    public bool Accepted { get; private set; }

    public ColorPickerDialog(byte r, byte g, byte b)
    {
        InitializeComponent();

        SliderR.Value = r;
        SliderG.Value = g;
        SliderB.Value = b;

        UpdatePreview();
    }

    private void OnSliderChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
    {
        UpdatePreview();
    }

    private void UpdatePreview()
    {
        // Guard against calls before InitializeComponent completes
        if (PreviewBorder == null) return;

        var r = (byte)SliderR.Value;
        var g = (byte)SliderG.Value;
        var b = (byte)SliderB.Value;

        PreviewBorder.Background = new SolidColorBrush(Color.FromRgb(r, g, b));
        ValueR.Text = r.ToString();
        ValueG.Text = g.ToString();
        ValueB.Text = b.ToString();
    }

    private void OnOk(object sender, RoutedEventArgs e)
    {
        SelectedColor = Color.FromRgb(
            (byte)SliderR.Value,
            (byte)SliderG.Value,
            (byte)SliderB.Value);
        Accepted = true;
        Close();
    }

    private void OnCancel(object sender, RoutedEventArgs e)
    {
        Accepted = false;
        Close();
    }
}
