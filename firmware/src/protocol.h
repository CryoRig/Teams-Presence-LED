#pragma once
#include <stdint.h>

// USB HID vendor protocol shared by firmware and bridge; keep in sync with docs/protocol.md
// and bridge/teams-presence-bridge-rs/src/hid.rs.

namespace proto {

// Host -> device commands (first payload byte).
enum Command : uint8_t {
    CMD_PING                = 0x01,
    CMD_OFF                 = 0x02,
    CMD_SOLID               = 0x03,
    CMD_BREATHE             = 0x04,
    CMD_BREATHE_SLOW        = 0x05,
    CMD_BRIGHTNESS          = 0x06,
    CMD_TRANSITION          = 0x07,
    CMD_RESET               = 0x08,
    CMD_BOOTLOADER          = 0x09,
    CMD_VERSION             = 0x0A,
    CMD_GET_CALIBRATION     = 0x0B,
    CMD_PREVIEW_CALIBRATION = 0x0C,
    CMD_SAVE_CALIBRATION    = 0x0D,
    CMD_CALIBRATION_TEST    = 0x0E,
    CMD_GET_LED_COUNT       = 0x0F,
    CMD_SET_LED_COUNT       = 0x10,
    CMD_GET_LED_TYPE        = 0x11,
    CMD_SET_LED_TYPE        = 0x12,
};

// Device -> host status byte (first payload byte of an input report).
enum Status : uint8_t {
    STATUS_KEEPALIVE   = 0x00,
    STATUS_PONG        = 0x01,
    STATUS_OK          = 0x02,
    STATUS_VERSION     = CMD_VERSION,
    STATUS_CALIBRATION = CMD_GET_CALIBRATION,
    STATUS_LED_COUNT   = CMD_GET_LED_COUNT,
    STATUS_LED_TYPE    = CMD_GET_LED_TYPE,
    STATUS_ERROR       = 0xFF,
};

// CMD_CALIBRATION_TEST P1 selectors.
enum CalibrationTestPattern : uint8_t {
    CAL_TEST_WHITE    = 0,
    CAL_TEST_MID_GRAY = 1,
    // Exit the pattern and reload the saved profile from NVS.
    CAL_TEST_EXIT     = 0xFF,
};

// CMD_SET_LED_COUNT P2 selectors.
enum LedCountApply : uint8_t {
    LED_COUNT_PREVIEW = 0,
    LED_COUNT_PERSIST = 1,
};

// Addressable LED chipsets (CMD_SET_LED_TYPE P1 / GET reply). IDs are a protocol contract: append only.
enum LedType : uint8_t {
    LED_TYPE_WS2812B      = 0,
    LED_TYPE_SK6812_RGBW  = 1, // cool white (~6000 K)
    LED_TYPE_SK6812_RGBNW = 2, // neutral white (~4500 K)
    LED_TYPE_SK6812_RGBWW = 3, // warm white (~3000 K)
    LED_TYPE_COUNT,
};

// CMD_SET_LED_TYPE P2 selectors.
enum LedTypeApply : uint8_t {
    LED_TYPE_PREVIEW = 0,
    LED_TYPE_PERSIST = 1,
};

// Flag bits in the last byte of the CMD_GET_LED_TYPE reply.
enum LedTypeFlags : uint8_t {
    LED_TYPE_FLAG_RESTART_REQUIRED = 0x01,
};

static const uint16_t MAX_TRANSITION_MS = 10000;

} // namespace proto
