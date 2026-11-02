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
};

// Device -> host status byte (first payload byte of an input report).
enum Status : uint8_t {
    STATUS_KEEPALIVE   = 0x00,
    STATUS_PONG        = 0x01,
    STATUS_OK          = 0x02,
    STATUS_VERSION     = CMD_VERSION,
    STATUS_CALIBRATION = CMD_GET_CALIBRATION,
    STATUS_ERROR       = 0xFF,
};

// CMD_CALIBRATION_TEST P1 selectors.
enum CalibrationTestPattern : uint8_t {
    CAL_TEST_WHITE    = 0,
    CAL_TEST_MID_GRAY = 1,
    // Exit the pattern and reload the saved profile from NVS.
    CAL_TEST_EXIT     = 0xFF,
};

static const uint16_t MAX_TRANSITION_MS = 10000;

} // namespace proto
