#define FASTLED_RMT5_RECYCLE 1  // Fix RMT channel conflict with USB CDC on ESP32-S3

#include <Arduino.h>
#include <FastLED.h>
#include <WiFi.h>
#include <math.h>
#include "USB.h"
#include "USBHIDVendor.h"
#include <soc/rtc_cntl_reg.h>

USBHIDVendor Vendor(5); // 5 bytes payload


// --- Configuration ---
#define LED_PIN          2        // GPIO2 (D1 on XIAO ESP32-S3) — avoids strapping pin GPIO1
#define NUM_LEDS         8        // Number of WS2812B LEDs in the chain
#define BRIGHTNESS       255      // Max brightness — protocol RGB values control intensity
#define WATCHDOG_TIMEOUT 60000    // 60 seconds without PING → disconnected state
#define FRAME_MS         17       // ~16.67 ms → 60 FPS (rounded to nearest ms)
#define SERIAL_BUF_SIZE  64       // Max serial command length (bytes)

// Breathing speed constants (radians per frame at 60 FPS)
// Full sine cycle = 2π radians. At 60 FPS:
//   3-second cycle: 2π / (60 × 3) ≈ 0.0349
//   5-second cycle: 2π / (60 × 5) ≈ 0.0209
#define BREATHE_SPEED_MODERATE 0.0349f
#define BREATHE_SPEED_SLOW     0.0209f

CRGB leds[NUM_LEDS];

enum State {
    STATE_OFF,
    STATE_SOLID,
    STATE_BREATHE,
    STATE_BREATHE_SLOW,
    STATE_DISCONNECTED
};

State currentState = STATE_OFF;
CRGB targetColor = CRGB::Black;
CRGB lastHardwareColor = CRGB::Black;
unsigned long lastHeartbeat = 0;
unsigned long lastFrameTime = 0;
float breatheAngle = 0.0f;

// Transition state
CRGB previousColor = CRGB::Black;
unsigned long transitionStartTime = 0;
bool isTransitioning = false;
unsigned int transitionDurationMs = 500;

// --- Serial input buffer (non-blocking, length-guarded) ---
char serialBuf[SERIAL_BUF_SIZE];
int  serialBufLen = 0;

// --- Helper: set all LEDs to a color and show ---
void showSolid(CRGB color, bool force = false) {
    // If the requested color is already on the strip, do nothing unless forced
    if (!force && color == lastHardwareColor) return; 

    lastHardwareColor = color; // Update the cache
    fill_solid(leds, NUM_LEDS, color);
    FastLED.show();
}

// --- Helper: start a transition if enabled ---
void startStateTransition() {
    if (transitionDurationMs > 0) {
        previousColor = lastHardwareColor;
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
            leds[i] = CHSV(hue, 255, 255);
        }
        FastLED.show();
        delay(FRAME_MS);
    }
    // Fade out
    for (int b = 255; b >= 0; b -= 8) {
        FastLED.setBrightness(b);
        FastLED.show();
        delay(10);
    }
    // Restore full brightness and clear
    FastLED.setBrightness(BRIGHTNESS);
    showSolid(CRGB::Black);
}

// --- HID Callback ---
static void vendor_event_cb(void* arg, esp_event_base_t event_base, int32_t event_id, void* event_data) {
    if(event_base == ARDUINO_USB_HID_VENDOR_EVENTS && event_id == ARDUINO_USB_HID_VENDOR_OUTPUT_EVENT){
        arduino_usb_hid_vendor_event_data_t * p = (arduino_usb_hid_vendor_event_data_t *)event_data;
        if (p->len < 4) return;

        uint8_t cmd   = p->buffer[0];
        uint8_t p1    = p->buffer[1];
        uint8_t p2    = p->buffer[2];
        uint8_t p3    = p->buffer[3];

        uint8_t response[2] = {0x02, 0x00}; // OK by default

        switch (cmd) {
            case 0x01: // PING
                response[0] = 0x01; // PONG
                if (currentState == STATE_DISCONNECTED) {
                    startStateTransition();
                    currentState = STATE_OFF;
                }
                lastHeartbeat = millis();
                break;
            case 0x02: // OFF
                startStateTransition();
                currentState = STATE_OFF;
                lastHeartbeat = millis();
                break;
            case 0x03: // SOLID
                startStateTransition();
                targetColor = CRGB(p1, p2, p3);
                currentState = STATE_SOLID;
                lastHeartbeat = millis();
                break;
            case 0x04: // BREATHE
                startStateTransition();
                targetColor = CRGB(p1, p2, p3);
                currentState = STATE_BREATHE;
                lastHeartbeat = millis();
                break;
            case 0x05: // BREATHE_SLOW
                startStateTransition();
                targetColor = CRGB(p1, p2, p3);
                currentState = STATE_BREATHE_SLOW;
                lastHeartbeat = millis();
                break;
            case 0x06: // BRIGHTNESS
                FastLED.setBrightness(p1);
                FastLED.show();
                lastHeartbeat = millis();
                break;
            case 0x07: // TRANSITION
                transitionDurationMs = ((uint16_t)p1 << 8) | p2;
                if (transitionDurationMs > 10000) transitionDurationMs = 10000;
                lastHeartbeat = millis();
                break;
            case 0x08: // RESET
                Serial.println("REBOOTING (via HID)");
                Serial.flush();
                break;
            case 0x09: // BOOTLOADER
                REG_WRITE(RTC_CNTL_OPTION1_REG, RTC_CNTL_FORCE_DOWNLOAD_BOOT);
                ESP.restart();
                break;
            default:
                response[0] = 0xFF; // ERR
                break;
        }

        Vendor.write(response, sizeof(response));
    }
}

void setup() {
    // Explicitly disable unused radios
    WiFi.mode(WIFI_OFF);
    btStop();

    Vendor.onEvent(vendor_event_cb);
    Vendor.begin();
    USB.begin();

    Serial.begin(115200);
    unsigned long serialStart = millis();
    while (!Serial && millis() - serialStart < 3000) { ; }

    FastLED.addLeds<WS2812B, LED_PIN, GRB>(leds, NUM_LEDS);
    FastLED.setBrightness(BRIGHTNESS);
    showSolid(CRGB::Black);

    bootAnimation();

    lastHeartbeat = millis();
    Serial.println("\n--- XIAO ESP32-S3 PRESENCE INDICATOR v0.2.0 ---");
}

// --- Helper: read one complete line from Serial without blocking ---
// Returns true when a '\n'-terminated line is ready.
// Silently discards lines that exceed SERIAL_BUF_SIZE to prevent heap growth.
bool readSerialLine() {
    while (Serial.available() > 0) {
        char c = (char)Serial.read();
        if (c == '\r') continue;          // strip CR from CRLF hosts
        if (c == '\n') {
            serialBuf[serialBufLen] = '\0';
            serialBufLen = 0;
            return true;
        }
        if (serialBufLen < SERIAL_BUF_SIZE - 1) {
            serialBuf[serialBufLen++] = c;
        }
        // Overflow: discard character (buffer flushes on next '\n')
    }
    return false;
}

void loop() {
    unsigned long now = millis();

    // 1. Handle Serial Input
    if (readSerialLine()) {
        if (serialBuf[0] == '\0') {
            // Ignore empty lines
        } else if (strcmp(serialBuf, "HELP") == 0 || strcmp(serialBuf, "?") == 0) {
            Serial.println("--- Teams Presence LED v0.2.0 ---");
            Serial.println("--- Commands ---");
            Serial.println("HELP or ?          : Show this help message");
            Serial.println("RESET              : Reboot the microcontroller");
            Serial.println("NOTE: Communication has migrated to USB HID. Other serial commands are ignored.");
        } else if (strcmp(serialBuf, "RESET") == 0) {
            Serial.println("REBOOTING");
            Serial.flush();
            ESP.restart();
        } else {
            Serial.print("ERR:USE_HID:");
            Serial.println(serialBuf);
        }
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

        if (currentState == STATE_BREATHE) {
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