using System.ComponentModel;
using System.IO;
using System.Text.Json;
using System.Windows;
using System.Windows.Media;

namespace TeamsPresenceBridge;

/// <summary>
/// Settings window that lets the user pick a color for each Teams presence state
/// and for the watchdog (disconnected) LED animation.
/// </summary>
public partial class SettingsWindow : Window
{
    private readonly ConfigModel _originalConfig;
    private List<ColorEntry> _entries = new();

    /// <summary>
    /// Raised when the user saves — carries the updated <see cref="ConfigModel"/>.
    /// </summary>
    public event Action<ConfigModel>? ConfigSaved;

    public SettingsWindow(ConfigModel config)
    {
        InitializeComponent();
        _originalConfig = config;
        PopulateEntries(config);
    }

    private void PopulateEntries(ConfigModel config)
    {
        _entries = new List<ColorEntry>();

        foreach (var kvp in config.PresenceMap)
        {
            _entries.Add(new ColorEntry
            {
                Key = kvp.Key,
                DisplayName = FormatName(kvp.Key),
                Command = kvp.Value.Command,
                R = (byte)Math.Clamp(kvp.Value.R, 0, 255),
                G = (byte)Math.Clamp(kvp.Value.G, 0, 255),
                B = (byte)Math.Clamp(kvp.Value.B, 0, 255)
            });
        }

        _entries.Add(new ColorEntry
        {
            Key = "__watchdog__",
            DisplayName = "Watchdog",
            Command = config.Watchdog.Command,
            R = (byte)Math.Clamp(config.Watchdog.R, 0, 255),
            G = (byte)Math.Clamp(config.Watchdog.G, 0, 255),
            B = (byte)Math.Clamp(config.Watchdog.B, 0, 255)
        });

        ColorList.ItemsSource = _entries;
    }

    private static string FormatName(string key) => key switch
    {
        "DoNotDisturb" => "Do Not Disturb",
        "BeRightBack" => "Be Right Back",
        _ => key
    };

    // ------------------------------------------------------------------
    //  Event handlers
    // ------------------------------------------------------------------

    private void OnPickColor(object sender, RoutedEventArgs e)
    {
        if (sender is not FrameworkElement { Tag: ColorEntry entry }) return;

        var picker = new ColorPickerDialog(entry.R, entry.G, entry.B)
        {
            Owner = this
        };
        picker.ShowDialog();

        if (picker.Accepted)
        {
            entry.R = picker.SelectedColor.R;
            entry.G = picker.SelectedColor.G;
            entry.B = picker.SelectedColor.B;
        }
    }

    private void OnDefaults(object sender, RoutedEventArgs e)
    {
        PopulateEntries(DefaultConfig.Create());
    }

    private void OnSave(object sender, RoutedEventArgs e)
    {
        var newMap = new Dictionary<string, PresenceEntry>();
        PresenceEntry? watchdog = null;

        foreach (var entry in _entries)
        {
            var pe = new PresenceEntry
            {
                Command = entry.Command,
                R = entry.R,
                G = entry.G,
                B = entry.B
            };

            if (entry.Key == "__watchdog__")
                watchdog = pe;
            else
                newMap[entry.Key] = pe;
        }

        var newConfig = _originalConfig with
        {
            PresenceMap = newMap,
            Watchdog = watchdog ?? _originalConfig.Watchdog
        };

        // Persist to disk
        var configPath = Path.Combine(AppContext.BaseDirectory, "config.json");
        var options = new JsonSerializerOptions { WriteIndented = true };
        File.WriteAllText(configPath, JsonSerializer.Serialize(newConfig, options));

        ConfigSaved?.Invoke(newConfig);
        Close();
    }

    private void OnCancel(object sender, RoutedEventArgs e)
    {
        Close();
    }
}

/// <summary>
/// View model for a single color row in the settings list.
/// Implements <see cref="INotifyPropertyChanged"/> so the color swatch
/// updates immediately after the user picks a new color.
/// </summary>
public class ColorEntry : INotifyPropertyChanged
{
    public string Key { get; init; } = "";
    public string DisplayName { get; init; } = "";
    public string Command { get; init; } = "";

    private byte _r, _g, _b;

    public byte R
    {
        get => _r;
        set { _r = value; OnPropertyChanged(nameof(R)); OnPropertyChanged(nameof(ColorBrush)); }
    }

    public byte G
    {
        get => _g;
        set { _g = value; OnPropertyChanged(nameof(G)); OnPropertyChanged(nameof(ColorBrush)); }
    }

    public byte B
    {
        get => _b;
        set { _b = value; OnPropertyChanged(nameof(B)); OnPropertyChanged(nameof(ColorBrush)); }
    }

    public SolidColorBrush ColorBrush => new(Color.FromRgb(R, G, B));

    public event PropertyChangedEventHandler? PropertyChanged;

    protected void OnPropertyChanged(string propertyName)
        => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
}
