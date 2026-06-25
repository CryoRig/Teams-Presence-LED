using System.IO;
using System.Text.RegularExpressions;

namespace TeamsPresenceBridge;

/// <summary>
/// Monitors Microsoft Teams presence by parsing the newest log file.
/// </summary>
public class TeamsLogWatcher
{
    private readonly string _logDirectory;
    private string? _currentFilePath;
    private long _lastPosition;
    private string? _lastPresence;

    private static readonly Regex StatusRegex = new(
        @"UserPresenceAction: \{cloud_context: https://teams\.microsoft\.com, availability: (\w+)\}",
        RegexOptions.Compiled
    );

    public TeamsLogWatcher()
    {
        _logDirectory = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "Packages",
            "MSTeams_8wekyb3d8bbwe",
            "LocalCache",
            "Microsoft",
            "MSTeams",
            "Logs"
        );
        Console.WriteLine($"[TeamsLogWatcher] Initialized using log directory: {_logDirectory}");
    }

    /// <summary>
    /// Reads the log file to detect presence changes.
    /// Returns the last known presence status, or null if not yet determined.
    /// </summary>
    public string? GetPresence()
    {
        if (!Directory.Exists(_logDirectory))
        {
            Console.Error.WriteLine($"[TeamsLogWatcher] Log directory does not exist: {_logDirectory}");
            return null;
        }

        var dirInfo = new DirectoryInfo(_logDirectory);
        var newestFile = dirInfo.GetFiles("MSTeams_*.log")
            .OrderByDescending(f => f.LastWriteTime)
            .FirstOrDefault();

        if (newestFile == null)
        {
            return _lastPresence;
        }

        // Switch file / rotation check
        if (_currentFilePath == null || newestFile.FullName != _currentFilePath)
        {
            if (_currentFilePath != null)
            {
                Console.WriteLine($"[TeamsLogWatcher] Log rotated -> {newestFile.Name}");
            }
            _currentFilePath = newestFile.FullName;
            _lastPosition = 0;
        }

        try
        {
            // Open with FileShare.ReadWrite so we don't lock MSTeams out of its logs
            using var stream = new FileStream(_currentFilePath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite);
            stream.Seek(_lastPosition, SeekOrigin.Begin);

            using var reader = new StreamReader(stream, System.Text.Encoding.UTF8, true);
            string? line;
            while ((line = reader.ReadLine()) != null)
            {
                var match = StatusRegex.Match(line);
                if (match.Success)
                {
                    var newStatus = match.Groups[1].Value;
                    if (newStatus != "PresenceUnknown" && newStatus != _lastPresence)
                    {
                        _lastPresence = newStatus;
                    }
                }
            }
            _lastPosition = stream.Position;
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"[TeamsLogWatcher] Error reading log file: {ex.Message}");
        }

        return _lastPresence;
    }
}
