#ifdef ARDUINO_ARCH_ESP32
#include "Esp32UsbManager.h"

Esp32UsbManager* Esp32UsbManager::instance = nullptr;

void CustomUSBHIDVendor::_onOutput(uint8_t report_id, const uint8_t* buffer, uint16_t len) {
    // If len == 6, TinyUSB did not strip the Report ID. If len == 5, it did.
    // We check if the first byte is the Report ID (0x06).
    uint8_t offset = (len >= 6 && buffer[0] == 0x06) ? 1 : 0;
    
    // We need at least 4 bytes of payload (cmd, p1, p2, p3)
    if (len >= offset + 4) {
        HidCommand cmd;
        cmd.cmd = buffer[offset];
        cmd.p1 = buffer[offset + 1];
        cmd.p2 = buffer[offset + 2];
        cmd.p3 = buffer[offset + 3];
        
        if (Esp32UsbManager::instance && Esp32UsbManager::instance->cmdQueue) {
            // Push directly to our thread-safe queue. This completely bypasses
            // TinyUSB's rx_queue and avoids the ephemeral pointer bugs of the esp_event system.
            xQueueSendFromISR(Esp32UsbManager::instance->cmdQueue, &cmd, NULL);
        }
    }
}

Esp32UsbManager::Esp32UsbManager() : vendor(5) {
    instance = this;
    // 20 commands should be more than enough to handle rapid burst of commands
    cmdQueue = xQueueCreate(20, sizeof(HidCommand));
}

Esp32UsbManager::~Esp32UsbManager() {
    instance = nullptr;
    if (cmdQueue) {
        vQueueDelete(cmdQueue);
    }
}

void Esp32UsbManager::begin(UsbCommandCallback callback) {
    cmdCallback = callback;
    vendor.begin();
    USB.productName(USB_PRODUCT);
    USB.manufacturerName(USB_MANUFACTURER);
    #if defined(USB_VID) && defined(USB_PID)
    USB.VID(USB_VID);
    USB.PID(USB_PID);
    #endif
    USB.begin();
}

void Esp32UsbManager::sendVersion(uint8_t major, uint8_t minor, uint8_t patch, uint8_t variant) {
    uint8_t ver_response[5] = {0x0A, major, minor, patch, variant};
    vendor.write(ver_response, sizeof(ver_response));
}

void Esp32UsbManager::sendResponse(uint8_t status, uint8_t value) {
    uint8_t response[5] = {status, value, 0, 0, 0};
    vendor.write(response, sizeof(response));
}

void Esp32UsbManager::loop() {
    HidCommand cmd;
    // Read from our safe queue and dispatch
    while (cmdQueue && xQueueReceive(cmdQueue, &cmd, 0) == pdTRUE) {
        if (cmdCallback) {
            cmdCallback(cmd.cmd, cmd.p1, cmd.p2, cmd.p3);
        }
    }
}
#endif
