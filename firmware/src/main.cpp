#define FASTLED_RMT5_RECYCLE 1  // Fix RMT channel conflict with USB CDC on ESP32-S3

#include <Arduino.h>
#include <FastLED.h>
#include <math.h>
#include <Preferences.h>
#include <soc/rtc_cntl_reg.h>
#include <soc/rtc_cntl_struct.h>
#include <soc/usb_serial_jtag_struct.h>
#include "UsbManager.h"

UsbManager usbManager;


// --- Firmware Version ---
#define FW_VERSION_MAJOR 0
#define FW_VERSION_MINOR 6
#define FW_VERSION_PATCH 2
// Hardware variant byte in the VERSION response (protocol.md). 1 = Seeed XIAO ESP32-S3.
#define HW_VARIANT 1

// --- Configuration ---
#define LED_PIN          2        // GPIO2 (D1 on XIAO ESP32-S3) — avoids strapping pin GPIO1
#define NUM_LEDS         8        // Number of WS2812B LEDs in the chain
#define BRIGHTNESS       255      // Max brightness — protocol RGB values control intensity
#define WATCHDOG_TIMEOUT 60000    // 60 seconds without PING → disconnected state
#define FRAME_MS         17       // ~16.67 ms → 60 FPS (rounded to nearest ms)

// Breathing speed constants (radians per frame at 60 FPS)
// Full sine cycle = 2π radians. At 60 FPS:
//   3-second cycle: 2π / (60 × 3) ≈ 0.0349
//   5-second cycle: 2π / (60 × 5) ≈ 0.0209
#define BREATHE_SPEED_MODERATE 0.0349f
#define BREATHE_SPEED_SLOW     0.0209f

CRGB leds[NUM_LEDS];
CRGB bootColors[NUM_LEDS];

enum State {
    STATE_OFF,
    STATE_SOLID,
    STATE_BREATHE,
    STATE_BREATHE_SLOW,
    STATE_DISCONNECTED,
    STATE_CALIBRATION_TEST
};

struct LedCalibration {
    uint8_t redGain;
    uint8_t greenGain;
    uint8_t blueGain;
    uint8_t gammaTenths;
};

static const uint8_t CALIBRATION_SCHEMA = 1;
static const LedCalibration NEUTRAL_CALIBRATION = {100, 100, 100, 10};
LedCalibration calibration = NEUTRAL_CALIBRATION;
Preferences calibrationPreferences;

State currentState = STATE_OFF;
CRGB targetColor = CRGB::Black;
CRGB lastLogicalColor = CRGB::Black;
State lastCommandedState = STATE_OFF;
CRGB lastCommandedColor = CRGB::Black;
unsigned long lastHeartbeat = 0;
unsigned long lastFrameTime = 0;
unsigned long lastUsbKeepalive = 0;
float breatheAngle = 0.0f;
unsigned long calibrationTestStart = 0;

// Transition state
CRGB previousColor = CRGB::Black;
unsigned long transitionStartTime = 0;
bool isTransitioning = false;
unsigned int transitionDurationMs = 500;


// --- Helper: set all LEDs to a color and show ---
LedCalibration calibrationDefaultsForVariant(uint8_t variant) {
    switch (variant) {
        case 1: return NEUTRAL_CALIBRATION;
        default: return NEUTRAL_CALIBRATION;
    }
}

LedCalibration clampCalibration(LedCalibration value) {
    value.redGain = min(value.redGain, (uint8_t)200);
    value.greenGain = min(value.greenGain, (uint8_t)200);
    value.blueGain = min(value.blueGain, (uint8_t)200);
    value.gammaTenths = constrain(value.gammaTenths, (uint8_t)5, (uint8_t)30);
    return value;
}

void loadCalibration() {
    calibration = calibrationDefaultsForVariant(HW_VARIANT);
    calibrationPreferences.begin("led-cal", true);
    if (calibrationPreferences.getUChar("schema", 0) == CALIBRATION_SCHEMA) {
        calibration.redGain = calibrationPreferences.getUChar("red", calibration.redGain);
        calibration.greenGain = calibrationPreferences.getUChar("green", calibration.greenGain);
        calibration.blueGain = calibrationPreferences.getUChar("blue", calibration.blueGain);
        calibration.gammaTenths = calibrationPreferences.getUChar("gamma", calibration.gammaTenths);
        calibration = clampCalibration(calibration);
    }
    calibrationPreferences.end();
}

void saveCalibration() {
    calibration = clampCalibration(calibration);
    calibrationPreferences.begin("led-cal", false);
    calibrationPreferences.putUChar("schema", CALIBRATION_SCHEMA);
    calibrationPreferences.putUChar("red", calibration.redGain);
    calibrationPreferences.putUChar("green", calibration.greenGain);
    calibrationPreferences.putUChar("blue", calibration.blueGain);
    calibrationPreferences.putUChar("gamma", calibration.gammaTenths);
    calibrationPreferences.end();
}

CRGB calibratedColor(CRGB color) {
    color.r = min((uint16_t)255, ((uint16_t)color.r * calibration.redGain + 50) / 100);
    color.g = min((uint16_t)255, ((uint16_t)color.g * calibration.greenGain + 50) / 100);
    color.b = min((uint16_t)255, ((uint16_t)color.b * calibration.blueGain + 50) / 100);
    if (calibration.gammaTenths != 10) {
        color = applyGamma_video(color, calibration.gammaTenths / 10.0f);
    }
    return color;
}

void showBootColors() {
    for (int i = 0; i < NUM_LEDS; i++) {
        leds[i] = calibratedColor(bootColors[i]);
    }
    FastLED.show();
}

// --- Helper: set all LEDs to a color and show ---
void showSolid(CRGB color, bool force = false) {
    // Cache logical colors so output correction is never applied cumulatively.
    if (!force && color == lastLogicalColor) return;

    lastLogicalColor = color; // Update the cache
    fill_solid(leds, NUM_LEDS, calibratedColor(color));
    FastLED.show();
}

// --- Helper: start a transition if enabled ---
void startStateTransition() {
    if (transitionDurationMs > 0) {
        previousColor = lastLogicalColor;
        transitionStartTime = millis();
        isTransitioning = true;
    }
}

// --- Helper: advance breathe animation by one frame and return color ---
// Wraps breatheAngle to [0, 2π) to prevent float precision loss over time.
CRGB getBreatheColor(float speed, CRGB color) {
    breatheAngle += speed;
    if (breatheAngle >= TWO_PI) breatheAngle -= TWO_PI;
    float scale = (sinf(breatheAngle) + 1.0f) / 2.0f;
    scale = scale * scale; // Approximate gamma 2.0 — simple and effective
    return CRGB(
        (uint8_t)(color.r * scale),
        (uint8_t)(color.g * scale),
        (uint8_t)(color.b * scale)
    );
}

// --- Boot animation: rainbow wave across LEDs ---
void bootAnimation() {
    const int frames = 90;  // ~1.5 seconds at 60 FPS
    for (int f = 0; f < frames; f++) {
        for (int i = 0; i < NUM_LEDS; i++) {
            // Each LED gets a hue offset based on its position + the current frame
            uint8_t hue = (i * 256 / NUM_LEDS) + (f * 4);
            bootColors[i] = CHSV(hue, 255, 255);
        }
        showBootColors();
        delay(FRAME_MS);
    }
    // Fade out
    for (int b = 255; b >= 0; b -= 8) {
        FastLED.setBrightness(b);
        showBootColors();
        delay(10);
    }
    FastLED.setBrightness(0);
    showBootColors();
    // Restore full brightness and clear
    FastLED.setBrightness(BRIGHTNESS);
    showSolid(CRGB::Black, true);
}

// --- HID Callback ---
// Any valid command proves the host is alive: refresh the watchdog and leave the disconnected state.
static void markHostAlive() {
    if (currentState == STATE_DISCONNECTED) {
        startStateTransition();
        currentState = lastCommandedState;
        targetColor = lastCommandedColor;
    }
    lastHeartbeat = millis();
}

void onUsbCommand(uint8_t cmd, uint8_t p1, uint8_t p2, uint8_t p3, uint8_t p4) {
    uint8_t response[2] = {0x02, 0x00}; // OK by default

    switch (cmd) {
        case 0x01: // PING
            response[0] = 0x01; // PONG
            markHostAlive();
            break;
        case 0x02: // OFF
            startStateTransition();
            currentState = STATE_OFF;
            lastCommandedState = STATE_OFF;
            lastCommandedColor = CRGB::Black;
            lastHeartbeat = millis();
            break;
        case 0x03: // SOLID
            startStateTransition();
            targetColor = CRGB(p1, p2, p3);
            currentState = STATE_SOLID;
            lastCommandedState = STATE_SOLID;
            lastCommandedColor = targetColor;
            lastHeartbeat = millis();
            break;
        case 0x04: // BREATHE
            startStateTransition();
            targetColor = CRGB(p1, p2, p3);
            currentState = STATE_BREATHE;
            lastCommandedState = STATE_BREATHE;
            lastCommandedColor = targetColor;
            lastHeartbeat = millis();
            break;
        case 0x05: // BREATHE_SLOW
            startStateTransition();
            targetColor = CRGB(p1, p2, p3);
            currentState = STATE_BREATHE_SLOW;
            lastCommandedState = STATE_BREATHE_SLOW;
            lastCommandedColor = targetColor;
            lastHeartbeat = millis();
            break;
        case 0x06: // BRIGHTNESS
            FastLED.setBrightness(p1);
            showSolid(lastLogicalColor, true); // Re-push the cached color at the new brightness
            markHostAlive();
            break;
        case 0x07: // TRANSITION
            transitionDurationMs = ((uint16_t)p1 << 8) | p2;
            if (transitionDurationMs > 10000) transitionDurationMs = 10000;
            markHostAlive();
            break;
        case 0x08: // RESET
            ESP.restart();
            break;
        case 0x09: // BOOTLOADER
            usbManager.sendResponse(0x02, 0);
            delay(250);
            USB_SERIAL_JTAG.conf0.phy_sel = 0;
            USB_SERIAL_JTAG.conf0.pad_pull_override = 0;
            USB_SERIAL_JTAG.conf0.dp_pullup = 1;
            USB_SERIAL_JTAG.conf0.usb_pad_enable = 1;
            RTCCNTL.usb_conf.sw_hw_usb_phy_sel = 1;
            RTCCNTL.usb_conf.sw_usb_phy_sel = 0;
            REG_WRITE(RTC_CNTL_OPTION1_REG, RTC_CNTL_FORCE_DOWNLOAD_BOOT);
            REG_WRITE(RTC_CNTL_OPTIONS0_REG, RTC_CNTL_SW_SYS_RST);
            for (;;) delay(1000);
        case 0x0A: // VERSION
            usbManager.sendVersion(FW_VERSION_MAJOR, FW_VERSION_MINOR, FW_VERSION_PATCH, HW_VARIANT);
            return; // Skip default 2-byte response
        case 0x0B: // GET_CALIBRATION
            markHostAlive();
            usbManager.sendCalibration(calibration.redGain, calibration.greenGain,
                                       calibration.blueGain, calibration.gammaTenths);
            return;
        case 0x0C: // PREVIEW_CALIBRATION
            calibration = clampCalibration({p1, p2, p3, p4});
            showSolid(lastLogicalColor, true);
            markHostAlive();
            break;
        case 0x0D: // SAVE_CALIBRATION
            saveCalibration();
            markHostAlive();
            break;
        case 0x0E: // CALIBRATION_TEST_PATTERN
            markHostAlive();
            isTransitioning = false;
            calibrationTestStart = millis();
            currentState = STATE_CALIBRATION_TEST;
            break;
        default:
            response[0] = 0xFF; // ERR
            break;
    }

    usbManager.sendResponse(response[0], response[1]);
}

void setup() {
    usbManager.begin(onUsbCommand);

    FastLED.addLeds<WS2812B, LED_PIN, GRB>(leds, NUM_LEDS);
    FastLED.setMaxPowerInVoltsAndMilliamps(5, 480); // Limit to 5V 480mA for USB safety (note: full white will be dimmed by FastLED to meet this budget)
    FastLED.setBrightness(BRIGHTNESS);
    loadCalibration();
    showSolid(CRGB::Black);

    bootAnimation();

    lastHeartbeat = millis();
}

void loop() {
    usbManager.loop();

    unsigned long now = millis();
    if (now - lastUsbKeepalive >= 1000) {
        lastUsbKeepalive = now;
        usbManager.sendResponse(0x00, 0);
    }

    // 2. Watchdog Check — any command resets the timer
    if (currentState != STATE_DISCONNECTED && now - lastHeartbeat > WATCHDOG_TIMEOUT) {
        currentState = STATE_DISCONNECTED;
        breatheAngle = 0;
    }

    // 3. Animation Logic (frame-rate limited)
    if (now - lastFrameTime >= FRAME_MS) {
        lastFrameTime = now;

        CRGB nextColor = CRGB::Black;

        if (currentState == STATE_CALIBRATION_TEST) {
            const unsigned long elapsed = now - calibrationTestStart;
            if (elapsed >= 2800) {
                currentState = lastCommandedState;
                nextColor = currentState == STATE_OFF ? CRGB::Black : lastCommandedColor;
            } else {
                const uint8_t testIndex = elapsed / 700;
                const CRGB testColors[] = {CRGB::Red, CRGB::Green, CRGB::Blue, CRGB::White};
                nextColor = testColors[testIndex];
            }
        } else if (currentState == STATE_BREATHE) {
            nextColor = getBreatheColor(BREATHE_SPEED_MODERATE, targetColor);
        } else if (currentState == STATE_BREATHE_SLOW) {
            nextColor = getBreatheColor(BREATHE_SPEED_SLOW, targetColor);
        } else if (currentState == STATE_DISCONNECTED) {
            nextColor = getBreatheColor(BREATHE_SPEED_MODERATE, CRGB(255, 255, 255));
        } else if (currentState == STATE_SOLID) {
            nextColor = targetColor;
        } else if (currentState == STATE_OFF) {
            nextColor = CRGB::Black;
        }

        if (isTransitioning) {
            unsigned long elapsed = now - transitionStartTime;
            if (elapsed >= transitionDurationMs) {
                isTransitioning = false;
                showSolid(nextColor, true); // Force update at end of transition
            } else {
                uint8_t progress = (elapsed * 255) / transitionDurationMs;
                CRGB blended = blend(previousColor, nextColor, progress);
                showSolid(blended, true); // Force update during transition
            }
        } else {
            showSolid(nextColor);
        }
    }
}