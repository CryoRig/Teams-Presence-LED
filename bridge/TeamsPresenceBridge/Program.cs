namespace TeamsPresenceBridge;

/// <summary>
/// Factory defaults for the presence-to-color mapping.
/// Used by the Settings UI "Defaults" button to reset all colors.
/// </summary>
public static class DefaultConfig
{
    public static ConfigModel Create() => new()
    {
        ComPort = "AUTO",
        PollIntervalMs = 5000,
        PingIntervalMs = 15000,
        PresenceMap = new Dictionary<string, PresenceEntry>
        {
            ["Available"] = new() { Command = "SOLID", R = 0, G = 200, B = 0 },
            ["Busy"] = new() { Command = "SOLID", R = 200, G = 0, B = 0 },
            ["DoNotDisturb"] = new() { Command = "SOLID", R = 200, G = 0, B = 0 },
            ["Away"] = new() { Command = "BREATHE_SLOW", R = 255, G = 120, B = 0 },
            ["BeRightBack"] = new() { Command = "BREATHE_SLOW", R = 255, G = 80, B = 0 },
            ["Offline"] = new() { Command = "OFF", R = 0, G = 0, B = 0 },
            ["Unknown"] = new() { Command = "BREATHE", R = 80, G = 80, B = 80 }
        },
        Watchdog = new() { Command = "BREATHE_SLOW", R = 255, G = 255, B = 255 }
    };
}
