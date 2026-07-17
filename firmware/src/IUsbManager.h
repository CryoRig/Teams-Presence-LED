#pragma once
#include <stdint.h>
#include <functional>

// Callback function type: receives cmd, p1, p2, p3
typedef std::function<void(uint8_t, uint8_t, uint8_t, uint8_t)> UsbCommandCallback;

class IUsbManager {
public:
    virtual ~IUsbManager() = default;

    // Initialize USB stack and register callback
    virtual void begin(UsbCommandCallback callback) = 0;

    // Send a version response directly to the host
    virtual void sendVersion(uint8_t major, uint8_t minor, uint8_t patch) = 0;

    // Send a standard 2-byte response (e.g. 0x02 0x00 for OK)
    virtual void sendResponse(uint8_t status, uint8_t value) = 0;

    // Update internal state if necessary (e.g., polling for TinyUSB)
    virtual void loop() = 0;
};
