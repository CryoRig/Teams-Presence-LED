#pragma once
#include "IUsbManager.h"
#include "Adafruit_TinyUSB.h"

class PicoUsbManager : public IUsbManager {
public:
    PicoUsbManager();
    ~PicoUsbManager() override;

    void begin(UsbCommandCallback callback) override;
    void sendVersion(uint8_t major, uint8_t minor, uint8_t patch, uint8_t variant) override;
    void sendResponse(uint8_t status, uint8_t value) override;
    void loop() override;

private:
    static void setReportCb(uint8_t report_id, hid_report_type_t report_type, uint8_t const* buffer, uint16_t bufsize);
    
    Adafruit_USBD_HID usb_hid;
    UsbCommandCallback cmdCallback;
    static PicoUsbManager* instance;
};
