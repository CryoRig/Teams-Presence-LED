using System.Net.Http;
using System.Text.Json;

namespace TeamsPresenceBridge;

/// <summary>
/// Tracks how the bridge is connected to Teams.
/// </summary>
public enum TeamsConnectionState
{
    None,
    Api,
    LogParsing
}

/// <summary>
/// Polls the local Teams presence API to retrieve the current user status.
/// Tries known ports (8124, 8125) used by certified peripherals.
/// </summary>
public class TeamsClient : IDisposable
{
    private static readonly int[] KnownPorts = [8124, 8125];
    private readonly HttpClient _http;
    private int? _activePort;
    private bool _isFallbackActive;
    private TeamsLogWatcher? _logWatcher;

    /// <summary>
    /// Current connection method used to reach Teams.
    /// </summary>
    public TeamsConnectionState ConnectionState { get; private set; } = TeamsConnectionState.None;

    public TeamsClient()
    {
        _http = new HttpClient { Timeout = TimeSpan.FromSeconds(3) };
    }

    /// <summary>
    /// Attempts to read the current presence state from the Teams local API,
    /// falling back to logfile parsing if the API is not responsive.
    /// Returns the presence string (e.g. "Available", "Busy") or null on failure.
    /// </summary>
    public async Task<string?> GetPresenceAsync()
    {
        if (_isFallbackActive)
        {
            return _logWatcher?.GetPresence();
        }

        // If we previously found a working port, try it first
        if (_activePort.HasValue)
        {
            var result = await TryGetPresenceFromPortAsync(_activePort.Value);
            if (result is not null)
            {
                ConnectionState = TeamsConnectionState.Api;
                return result;
            }
            _activePort = null; // Port stopped responding, re-scan
        }

        // Scan known ports
        foreach (var port in KnownPorts)
        {
            var result = await TryGetPresenceFromPortAsync(port);
            if (result is not null)
            {
                _activePort = port;
                ConnectionState = TeamsConnectionState.Api;
                Console.WriteLine($"[TeamsClient] Found Teams API on port {port}");
                return result;
            }
        }

        // HTTP API is not available, trigger fallback
        Console.WriteLine("[TeamsClient] Local HTTP API not responding. Falling back to logfile parsing.");
        _isFallbackActive = true;
        ConnectionState = TeamsConnectionState.LogParsing;
        _logWatcher = new TeamsLogWatcher();
        return _logWatcher.GetPresence();
    }

    private async Task<string?> TryGetPresenceFromPortAsync(int port)
    {
        try
        {
            var url = $"http://localhost:{port}/presenceState";
            var response = await _http.GetStringAsync(url);

            using var doc = JsonDocument.Parse(response);
            // The local API typically returns an object with an "availability" or "state" field
            if (doc.RootElement.TryGetProperty("availability", out var availability))
            {
                return availability.GetString();
            }
            if (doc.RootElement.TryGetProperty("state", out var state))
            {
                return state.GetString();
            }
            if (doc.RootElement.TryGetProperty("presenceState", out var presenceState))
            {
                return presenceState.GetString();
            }

            Console.WriteLine($"[TeamsClient] Unexpected response structure from port {port}");
            return null;
        }
        catch (HttpRequestException)
        {
            return null;
        }
        catch (TaskCanceledException)
        {
            return null;
        }
        catch (JsonException ex)
        {
            Console.WriteLine($"[TeamsClient] Failed to parse response from port {port}: {ex.Message}");
            return null;
        }
    }

    public void Dispose()
    {
        _http.Dispose();
        GC.SuppressFinalize(this);
    }
}
