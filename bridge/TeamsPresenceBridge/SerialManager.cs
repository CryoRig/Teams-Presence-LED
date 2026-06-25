using System.IO.Ports;

namespace TeamsPresenceBridge;

/// <summary>
/// Manages the serial connection to the ESP32 LED indicator.
/// Supports auto-reconnect on disconnect and heartbeat via PING/PONG.
/// </summary>
public class SerialManager : IDisposable
{
    private readonly string _portName;
    private readonly int _baudRate;
    private SerialPort? _port;

    public bool IsConnected => _port?.IsOpen == true;

    public SerialManager(string portName, int baudRate = 115200)
    {
        _portName = portName;
        _baudRate = baudRate;
    }

    /// <summary>
    /// Opens the serial connection. Returns true on success.
    /// </summary>
    public bool Connect()
    {
        try
        {
            _port?.Dispose();
            _port = new SerialPort(_portName, _baudRate)
            {
                NewLine = "\n",
                ReadTimeout = 2000,
                WriteTimeout = 2000,
                DtrEnable = true,
                RtsEnable = true
            };
            _port.Open();
            Console.WriteLine($"[Serial] Connected to {_portName} at {_baudRate} baud");
            return true;
        }
        catch (Exception ex)
        {
            Console.WriteLine($"[Serial] Failed to connect to {_portName}: {ex.Message}");
            return false;
        }
    }

    /// <summary>
    /// Sends a protocol command (newline-terminated) to the ESP32.
    /// Attempts auto-reconnect on failure.
    /// </summary>
    public void SendCommand(string command)
    {
        if (!EnsureConnected()) return;

        try
        {
            _port!.WriteLine(command);
            Console.WriteLine($"[Serial] Sent: {command}");
        }
        catch (Exception ex)
        {
            Console.WriteLine($"[Serial] Write failed: {ex.Message}");
            TryReconnect();
        }
    }

    /// <summary>
    /// Sends a PING heartbeat and attempts to read the PONG response.
    /// </summary>
    public bool SendPing()
    {
        if (!EnsureConnected()) return false;

        try
        {
            _port!.WriteLine("PING");
            var response = _port.ReadLine().Trim();
            if (response == "PONG")
            {
                return true;
            }

            Console.WriteLine($"[Serial] Unexpected PING response: {response}");
            return false;
        }
        catch (TimeoutException)
        {
            Console.WriteLine("[Serial] PING timed out — no PONG received");
            return false;
        }
        catch (Exception ex)
        {
            Console.WriteLine($"[Serial] PING failed: {ex.Message}");
            TryReconnect();
            return false;
        }
    }

    /// <summary>
    /// Closes the serial port.
    /// </summary>
    public void Disconnect()
    {
        if (_port?.IsOpen == true)
        {
            try
            {
                _port.Close();
                Console.WriteLine("[Serial] Disconnected");
            }
            catch (Exception ex)
            {
                Console.WriteLine($"[Serial] Error during disconnect: {ex.Message}");
            }
        }
    }

    private bool EnsureConnected()
    {
        if (IsConnected) return true;
        return TryReconnect();
    }

    private bool TryReconnect()
    {
        Console.WriteLine("[Serial] Attempting reconnect...");
        Disconnect();
        Thread.Sleep(1000);
        return Connect();
    }

    public void Dispose()
    {
        Disconnect();
        _port?.Dispose();
        GC.SuppressFinalize(this);
    }
}
