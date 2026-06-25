using System.Text.Json.Serialization;

namespace TeamsPresenceBridge;

/// <summary>
/// A single presence-to-LED mapping entry storing the command type and RGB color.
/// </summary>
public record PresenceEntry
{
    [JsonPropertyName("command")]
    public string Command { get; init; } = "";

    [JsonPropertyName("r")]
    public int R { get; init; }

    [JsonPropertyName("g")]
    public int G { get; init; }

    [JsonPropertyName("b")]
    public int B { get; init; }

    /// <summary>
    /// Builds the full serial command string, e.g. "SOLID:0,200,0" or "OFF".
    /// </summary>
    public string ToSerialCommand()
    {
        if (Command.Equals("OFF", StringComparison.OrdinalIgnoreCase))
            return "OFF";
        return $"{Command}:{R},{G},{B}";
    }
}

public record ConfigModel
{
    [JsonPropertyName("comPort")]
    public string ComPort { get; init; } = "AUTO";

    [JsonPropertyName("pollIntervalMs")]
    public int PollIntervalMs { get; init; } = 5000;

    [JsonPropertyName("pingIntervalMs")]
    public int PingIntervalMs { get; init; } = 15000;

    [JsonPropertyName("presenceMap")]
    public Dictionary<string, PresenceEntry> PresenceMap { get; init; } = new();

    [JsonPropertyName("watchdog")]
    public PresenceEntry Watchdog { get; init; } = new() { Command = "BREATHE_SLOW", R = 255, G = 255, B = 255 };
}
