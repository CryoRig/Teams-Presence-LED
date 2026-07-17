#pragma once
#include "IUsbManager.h"
#include "USB.h"
#include "USBHIDVendor.h"

class Esp32UsbManager : public IUsbManager {
public:
    Esp32UsbManager();
    ~Esp32UsbManager() override;

    void begin(UsbCommandCallback callback) override;
    void sendVersion(uint8_t major, uint8_t minor, uint8_t patch) override;
    void sendResponse(uint8_t status, uint8_t value) override;
    void loop() override;

private:
    static void vendorEventCb(void* arg, esp_event_base_t event_base, int32_t event_id, void* event_data);
    
    USBHIDVendor vendor;
    UsbCommandCallback cmdCallback;
    static Esp32UsbManager* instance;
};
