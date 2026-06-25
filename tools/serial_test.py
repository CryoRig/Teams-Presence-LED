import serial
import serial.tools.list_ports
import threading
import sys
import argparse
import time
from datetime import datetime

def read_from_port(ser):
    """Continuously reads from the serial port and prints to stdout."""
    while True:
        try:
            if not ser.is_open:
                print("[Reader] Port closed, stopping reader thread.")
                break
            if ser.in_waiting > 0:
                line = ser.readline().decode('utf-8', errors='replace').strip()
                if line:
                    timestamp = datetime.now().strftime("%H:%M:%S.%f")[:-3]
                    print(f"[{timestamp}] <ESP32>: {line}")
            else:
                time.sleep(0.01)  # Avoid busy-waiting when no data is available
        except serial.SerialException as e:
            print(f"[Reader] Serial port disconnected: {e}")
            break
        except Exception as e:
            print(f"[Reader] Error reading serial: {e}")
            break

def main():
    parser = argparse.ArgumentParser(description="Serial Interface Test Tool for ESP32")
    parser.add_argument("port", help="The COM port to connect to (e.g., COM3)")
    parser.add_argument("-b", "--baud", type=int, default=115200, help="Baud rate (default: 115200)")
    args = parser.parse_args()

    try:
        ser = serial.Serial(args.port, args.baud, timeout=1)
        print(f"--- Connected to {args.port} at {args.baud} baud ---")
        print("--- Type commands and press Enter (Ctrl+C to exit) ---")
    except Exception as e:
        print(f"Failed to connect to {args.port}: {e}")
        sys.exit(1)

    # Start the background thread for reading serial input
    thread = threading.Thread(target=read_from_port, args=(ser,), daemon=True)
    thread.start()

    try:
        while True:
            # Get user input from stdin
            cmd = input()
            if cmd:
                # Ensure command ends with \n as per protocol
                formatted_cmd = cmd if cmd.endswith('\n') else cmd + '\n'
                ser.write(formatted_cmd.encode('utf-8'))
    except KeyboardInterrupt:
        print("\n--- Closing connection ---")
    except serial.SerialException as e:
        print(f"Serial error: {e}")
    except Exception as e:
        print(f"Error: {e}")
    finally:
        if ser.is_open:
            ser.close()

if __name__ == "__main__":
    main()