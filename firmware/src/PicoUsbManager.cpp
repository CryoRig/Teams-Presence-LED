#if defined(ARDUINO_ARCH_RP2040) || defined(ARDUINO_ARCH_RP2350)
#include "PicoUsbManager.h"
#include <Arduino.h>

PicoUsbManager* PicoUsbManager::instance = nullptr;

#define HID_REPORT_ID_VENDOR 6

// Custom HID Report Descriptor matching the ESP32 USBHIDVendor exactly
uint8_t const desc_hid_report[] = {
    0x06, 0x00, 0xFF,  // Usage Page (Vendor Defined 0xFF00)
    0x09, 0x01,        // Usage (0x01)
    0xA1, 0x01,        // Collection (Application)
    0x85, HID_REPORT_ID_VENDOR, // Report ID (6)
    // Input report
    0x15, 0x00,        // Logical Minimum (0)
    0x26, 0xFF, 0x00,  // Logical Maximum (255)
    0x75, 0x08,        // Report Size (8 bits)
    0x95, 0x05,        // Report Count (5 bytes)
    0x81, 0x02,        // Input (Data, Variable, Absolute)
    // Output report
    0x15, 0x00,        // Logical Minimum (0)
    0x26, 0xFF, 0x00,  // Logical Maximum (255)
    0x75, 0x08,        // Report Size (8 bits)
    0x95, 0x05,        // Report Count (5 bytes)
    0x91, 0x02,        // Output (Data, Variable, Absolute)
    0xC0               // End Collection
};

PicoUsbManager::PicoUsbManager() {
    instance = this;
}

PicoUsbManager::~PicoUsbManager() {
    instance = nullptr;
}

void PicoUsbManager::begin(UsbCommandCallback callback) {
    cmdCallback = callback;
    
    // Set custom VID and PID via TinyUSB descriptors
    TinyUSBDevice.setID(0x1209, 0x0005);
    TinyUSBDevice.setManufacturerDescriptor("Sim-Lab");
    TinyUSBDevice.setProductDescriptor("Teams Presence Bridge");

    usb_hid.setReportDescriptor(desc_hid_report, sizeof(desc_hid_report));
    usb_hid.setReportCallback(NULL, setReportCb);
    usb_hid.begin();
    
    if (TinyUSBDevice.mounted()) { TinyUSBDevice.detach(); delay(10); TinyUSBDevice.attach(); }
    // Wait for TinyUSB to be ready if needed, or just proceed
}

void PicoUsbManager::sendVersion(uint8_t major, uint8_t minor, uint8_t patch, uint8_t variant) {
    uint8_t response[5] = {0x0A, major, minor, patch, variant};
    usb_hid.sendReport(HID_REPORT_ID_VENDOR, response, sizeof(response));
}

void PicoUsbManager::sendResponse(uint8_t status, uint8_t value) {
    uint8_t response[5] = {status, value, 0, 0, 0};
    usb_hid.sendReport(HID_REPORT_ID_VENDOR, response, sizeof(response));
}

void PicoUsbManager::loop() {
    // TinyUSB background tasks are handled automatically in earlephilhower core
    // via interrupts or yield(), but we can call TinyUSB_Device_Task() if needed.
    // Generally not required in the loop for this core.
}

void PicoUsbManager::setReportCb(uint8_t report_id, hid_report_type_t report_type, uint8_t const* buffer, uint16_t bufsize) {
    if (report_type == HID_REPORT_TYPE_OUTPUT && report_id == HID_REPORT_ID_VENDOR) {
        if (bufsize >= 4 && instance && instance->cmdCallback) {
            instance->cmdCallback(buffer[0], buffer[1], buffer[2], buffer[3]);
        }
    }
}

#endif
