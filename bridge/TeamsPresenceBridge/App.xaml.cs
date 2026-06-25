using System.IO;
using System.Text.Json;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Threading;
using Hardcodet.Wpf.TaskbarNotification;

namespace TeamsPresenceBridge;

/// <summary>
/// WPF application that lives entirely in the system tray.
/// Manages the bridge loop, tray icon, context menu, and settings window.
/// </summary>
public partial class App : Application
{
    private TaskbarIcon? _trayIcon;
    private CancellationTokenSource? _cts;
    private SerialManager? _serialManager;
    private TeamsClient? _teamsClient;
    private ConfigModel _config = null!;

    // Status indicators in the context menu
    private Border? _espStatusBorder;
    private TextBlock? _espStatusText;
    private Border? _teamsStatusBorder;
    private TextBlock? _teamsStatusText;

    private DispatcherTimer? _statusTimer;
    private SettingsWindow? _settingsWindow;

    // Track previous state to avoid redundant icon recreation
    private bool _lastEspState;
    private TeamsConnectionState _lastTeamsState = TeamsConnectionState.None;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        if (!LoadConfig())
        {
            MessageBox.Show(
                "Failed to load config.json.\nMake sure it exists alongside the executable.",
                "Teams Presence Bridge",
                MessageBoxButton.OK,
                MessageBoxImage.Error);
            Shutdown(1);
            return;
        }

        CreateTrayIcon();
        StartBridge();

        // Periodically refresh the status indicators
        _statusTimer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(2) };
        _statusTimer.Tick += (_, _) => UpdateStatusIndicators();
        _statusTimer.Start();

        UpdateStatusIndicators();
    }

    // ------------------------------------------------------------------
    //  Configuration
    // ------------------------------------------------------------------

    private bool LoadConfig()
    {
        try
        {
            var configPath = Path.Combine(AppContext.BaseDirectory, "config.json");
            if (!File.Exists(configPath)) return false;
            var json = File.ReadAllText(configPath);
            _config = JsonSerializer.Deserialize<ConfigModel>(json)
                ?? throw new InvalidOperationException("Deserialization returned null");
            return true;
        }
        catch
        {
            return false;
        }
    }

    // ------------------------------------------------------------------
    //  System Tray Icon & Context Menu
    // ------------------------------------------------------------------

    private void CreateTrayIcon()
    {
        var contextMenu = new ContextMenu();
        var statusStyle = CreateStatusMenuItemStyle();

        // ── Row 1: ESP32 connection status ──
        _espStatusText = new TextBlock
        {
            Text = "ESP32: Disconnected",
            Foreground = Brushes.White,
            FontWeight = FontWeights.SemiBold,
            FontSize = 12,
            FontFamily = new FontFamily("Segoe UI")
        };
        _espStatusBorder = new Border
        {
            Background = new SolidColorBrush(Color.FromRgb(180, 30, 30)),
            CornerRadius = new CornerRadius(3),
            Padding = new Thickness(12, 6, 12, 6),
            Child = _espStatusText
        };
        contextMenu.Items.Add(new MenuItem { Header = _espStatusBorder, Style = statusStyle });

        // ── Row 2: Teams connection status ──
        _teamsStatusText = new TextBlock
        {
            Text = "Teams: Not Connected",
            Foreground = Brushes.White,
            FontWeight = FontWeights.SemiBold,
            FontSize = 12,
            FontFamily = new FontFamily("Segoe UI")
        };
        _teamsStatusBorder = new Border
        {
            Background = new SolidColorBrush(Color.FromRgb(180, 30, 30)),
            CornerRadius = new CornerRadius(3),
            Padding = new Thickness(12, 6, 12, 6),
            Child = _teamsStatusText
        };
        contextMenu.Items.Add(new MenuItem { Header = _teamsStatusBorder, Style = statusStyle });

        contextMenu.Items.Add(new Separator());

        // ── Row 3: Settings ──
        var settingsItem = new MenuItem { Header = "⚙  Settings…", FontSize = 12 };
        settingsItem.Click += (_, _) => OpenSettings();
        contextMenu.Items.Add(settingsItem);

        // ── Row 4: Exit ──
        var exitItem = new MenuItem { Header = "✕  Exit", FontSize = 12 };
        exitItem.Click += (_, _) => ExitApplication();
        contextMenu.Items.Add(exitItem);

        // ── Tray icon ──
        _trayIcon = new TaskbarIcon
        {
            Icon = CreateIcon(System.Drawing.Color.Gray),
            ToolTipText = "Teams Presence LED Bridge",
            ContextMenu = contextMenu
        };
    }

    /// <summary>
    /// Creates a minimal MenuItem style that strips away all default chrome
    /// (icon gutter, hover highlight, submenu arrow) so the item acts as a
    /// pure display-only status indicator.
    /// </summary>
    private static Style CreateStatusMenuItemStyle()
    {
        var style = new Style(typeof(MenuItem));

        var template = new ControlTemplate(typeof(MenuItem));
        var cp = new FrameworkElementFactory(typeof(ContentPresenter));
        cp.SetValue(ContentPresenter.ContentSourceProperty, "Header");
        cp.SetValue(FrameworkElement.MarginProperty, new Thickness(2));
        template.VisualTree = cp;

        style.Setters.Add(new Setter(Control.TemplateProperty, template));
        style.Setters.Add(new Setter(UIElement.FocusableProperty, false));
        style.Setters.Add(new Setter(UIElement.IsHitTestVisibleProperty, false));

        return style;
    }

    /// <summary>
    /// Generates a 16×16 tray icon — a solid colored circle.
    /// </summary>
    private static System.Drawing.Icon CreateIcon(System.Drawing.Color color)
    {
        var bitmap = new System.Drawing.Bitmap(16, 16);
        using (var g = System.Drawing.Graphics.FromImage(bitmap))
        {
            g.Clear(System.Drawing.Color.Transparent);
            g.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
            using var brush = new System.Drawing.SolidBrush(color);
            g.FillEllipse(brush, 1, 1, 14, 14);
        }
        return System.Drawing.Icon.FromHandle(bitmap.GetHicon());
    }

    // ------------------------------------------------------------------
    //  Status Indicators
    // ------------------------------------------------------------------

    private void UpdateStatusIndicators()
    {
        // ESP32
        bool espConnected;
        try { espConnected = _serialManager?.IsConnected == true; }
        catch { espConnected = false; }

        if (espConnected)
        {
            _espStatusBorder!.Background = new SolidColorBrush(Color.FromRgb(30, 150, 30));
            _espStatusText!.Text = "ESP32: Connected";
        }
        else
        {
            _espStatusBorder!.Background = new SolidColorBrush(Color.FromRgb(180, 30, 30));
            _espStatusText!.Text = "ESP32: Disconnected";
        }

        // Teams
        var teamsState = _teamsClient?.ConnectionState ?? TeamsConnectionState.None;
        switch (teamsState)
        {
            case TeamsConnectionState.Api:
                _teamsStatusBorder!.Background = new SolidColorBrush(Color.FromRgb(30, 150, 30));
                _teamsStatusText!.Text = "Teams: API Connected";
                break;
            case TeamsConnectionState.LogParsing:
                _teamsStatusBorder!.Background = new SolidColorBrush(Color.FromRgb(200, 170, 0));
                _teamsStatusText!.Text = "Teams: Log Parsing";
                break;
            default:
                _teamsStatusBorder!.Background = new SolidColorBrush(Color.FromRgb(180, 30, 30));
                _teamsStatusText!.Text = "Teams: Not Connected";
                break;
        }

        // Update tray icon color only when state actually changes
        if (espConnected != _lastEspState || teamsState != _lastTeamsState)
        {
            var oldIcon = _trayIcon!.Icon;
            _trayIcon.Icon = CreateIcon(espConnected
                ? System.Drawing.Color.FromArgb(30, 150, 30)
                : System.Drawing.Color.FromArgb(180, 30, 30));
            oldIcon?.Dispose();
            _lastEspState = espConnected;
            _lastTeamsState = teamsState;
        }
    }

    // ------------------------------------------------------------------
    //  Bridge Loop
    // ------------------------------------------------------------------

    private void StartBridge()
    {
        _cts = new CancellationTokenSource();
        _teamsClient = new TeamsClient();

        // Try initial serial connection
        var comPort = ResolveComPort();
        if (comPort != null)
        {
            _serialManager = new SerialManager(comPort);
            _serialManager.Connect();
        }

        Task.Run(() => BridgeLoopAsync(_cts.Token));
    }

    private string? ResolveComPort()
    {
        var port = _config.ComPort;
        if (port != "AUTO") return port;

        var ports = System.IO.Ports.SerialPort.GetPortNames();
        return ports.Length > 0 ? ports[^1] : null;
    }

    private async Task BridgeLoopAsync(CancellationToken ct)
    {
        string? previousPresence = null;
        var lastPingTime = DateTime.UtcNow;

        while (!ct.IsCancellationRequested)
        {
            try
            {
                // Create serial manager lazily if ESP32 wasn't available at startup
                if (_serialManager == null)
                {
                    var comPort = ResolveComPort();
                    if (comPort != null)
                    {
                        _serialManager = new SerialManager(comPort);
                        _serialManager.Connect();
                    }
                }

                // Poll Teams presence
                var presence = await _teamsClient!.GetPresenceAsync();

                if (presence != null && presence != previousPresence)
                {
                    if (_config.PresenceMap.TryGetValue(presence, out var entry))
                        _serialManager?.SendCommand(entry.ToSerialCommand());
                    else
                        _serialManager?.SendCommand("BREATHE:80,80,80");

                    previousPresence = presence;
                }

                // Heartbeat
                if (_serialManager?.IsConnected == true &&
                    (DateTime.UtcNow - lastPingTime).TotalMilliseconds >= _config.PingIntervalMs)
                {
                    _serialManager.SendPing();
                    lastPingTime = DateTime.UtcNow;
                }

                await Task.Delay(_config.PollIntervalMs, ct);
            }
            catch (OperationCanceledException) { break; }
            catch
            {
                // Avoid tight loop on repeated errors
                try { await Task.Delay(2000, ct); }
                catch { break; }
            }
        }
    }

    // ------------------------------------------------------------------
    //  Settings
    // ------------------------------------------------------------------

    private void OpenSettings()
    {
        if (_settingsWindow is { IsVisible: true })
        {
            _settingsWindow.Activate();
            return;
        }

        _settingsWindow = new SettingsWindow(_config);
        _settingsWindow.ConfigSaved += newConfig => { _config = newConfig; };
        _settingsWindow.Show();
    }

    // ------------------------------------------------------------------
    //  Shutdown
    // ------------------------------------------------------------------

    private void ExitApplication()
    {
        _cts?.Cancel();
        _statusTimer?.Stop();
        _serialManager?.Dispose();
        _teamsClient?.Dispose();
        _trayIcon?.Dispose();
        Shutdown();
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _trayIcon?.Dispose();
        base.OnExit(e);
    }
}
