#pragma once
#include <stdint.h>
#include <functional>
#include "USB.h"
#include "USBHIDVendor.h"

// Callback function type: receives cmd, p1, p2, p3
typedef std::function<void(uint8_t, uint8_t, uint8_t, uint8_t)> UsbCommandCallback;

// Struct to hold exactly the parsed command parameters
struct HidCommand {
    uint8_t cmd;
    uint8_t p1;
    uint8_t p2;
    uint8_t p3;
};

// Subclass USBHIDVendor to intercept packets before TinyUSB queues them
class CustomUSBHIDVendor : public USBHIDVendor {
public:
    CustomUSBHIDVendor(uint8_t report_size) : USBHIDVendor(report_size) {}
    
    // Override the raw output handler
    void _onOutput(uint8_t report_id, const uint8_t* buffer, uint16_t len) override;
};

// USB HID vendor interface for the XIAO ESP32-S3 (TinyUSB via the Arduino core).
class UsbManager {
public:
    UsbManager();
    ~UsbManager();

    // Initialize USB stack and register callback
    void begin(UsbCommandCallback callback);

    // Send a version response directly to the host
    void sendVersion(uint8_t major, uint8_t minor, uint8_t patch, uint8_t variant);

    // Send a standard status response (5-byte report: status, value, padding)
    void sendResponse(uint8_t status, uint8_t value);

    // Dispatch queued commands to the callback; call from loop()
    void loop();

    QueueHandle_t cmdQueue;

private:
    CustomUSBHIDVendor vendor;
    UsbCommandCallback cmdCallback;
    static UsbManager* instance;
    friend class CustomUSBHIDVendor;
};
