#pragma once
#include "IUsbManager.h"
#include "USB.h"
#include "USBHIDVendor.h"

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

class Esp32UsbManager : public IUsbManager {
public:
    Esp32UsbManager();
    ~Esp32UsbManager() override;

    void begin(UsbCommandCallback callback) override;
    void sendVersion(uint8_t major, uint8_t minor, uint8_t patch, uint8_t variant) override;
    void sendResponse(uint8_t status, uint8_t value) override;
    void loop() override;

    QueueHandle_t cmdQueue;

private:
    CustomUSBHIDVendor vendor;
    UsbCommandCallback cmdCallback;
    static Esp32UsbManager* instance;
    friend class CustomUSBHIDVendor;
};
