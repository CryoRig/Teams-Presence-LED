#ifdef ARDUINO_ARCH_ESP32
#include "Esp32UsbManager.h"

Esp32UsbManager* Esp32UsbManager::instance = nullptr;

Esp32UsbManager::Esp32UsbManager() : vendor(5) {
    instance = this;
}

Esp32UsbManager::~Esp32UsbManager() {
    instance = nullptr;
}

void Esp32UsbManager::begin(UsbCommandCallback callback) {
    cmdCallback = callback;
    vendor.onEvent(vendorEventCb);
    vendor.begin();
    // Use the compiler defines from platformio.ini for manufacturer/product
    // If you need to set VID/PID explicitly on ESP32, it's done via build flags,
    // but we can also set it if the API allows. For ESP32 Arduino Core 2.x/3.x,
    // USB VID/PID is usually set by build flags (e.g., -D USB_VID=0x1209).
    USB.productName(USB_PRODUCT);
    USB.manufacturerName(USB_MANUFACTURER);
    // USB.VID(0x1209); USB.PID(0x0005); // Usually set by build_flags
    USB.begin();
}

void Esp32UsbManager::sendVersion(uint8_t major, uint8_t minor, uint8_t patch, uint8_t variant) {
    uint8_t ver_response[5] = {0x0A, major, minor, patch, variant};
    vendor.write(ver_response, sizeof(ver_response));
}

void Esp32UsbManager::sendResponse(uint8_t status, uint8_t value) {
    uint8_t response[2] = {status, value};
    vendor.write(response, sizeof(response));
}

void Esp32UsbManager::loop() {
    // Nothing to do for ESP32, uses event callbacks
}

void Esp32UsbManager::vendorEventCb(void* arg, esp_event_base_t event_base, int32_t event_id, void* event_data) {
    if (event_base == ARDUINO_USB_HID_VENDOR_EVENTS && event_id == ARDUINO_USB_HID_VENDOR_OUTPUT_EVENT) {
        arduino_usb_hid_vendor_event_data_t* p = (arduino_usb_hid_vendor_event_data_t*)event_data;
        if (p->len < 4) return;

        if (instance && instance->cmdCallback) {
            instance->cmdCallback(p->buffer[0], p->buffer[1], p->buffer[2], p->buffer[3]);
        }
    }
}
#endif
