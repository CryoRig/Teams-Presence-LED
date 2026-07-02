#define FASTLED_RMT5_RECYCLE 1  // Fix RMT channel conflict with USB CDC on ESP32-S3

#include <Arduino.h>
#include <FastLED.h>
#include <math.h>

// --- Configuration ---
#define LED_PIN          2        // GPIO2 (D1 on XIAO ESP32-S3) — avoids strapping pin GPIO1
#define NUM_LEDS         8        // Number of WS2812B LEDs in the chain
#define BRIGHTNESS       255      // Max brightness — protocol RGB values control intensity
#define WATCHDOG_TIMEOUT 30000    // 30 seconds without PING → disconnected state
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
unsigned long lastHeartbeat = 0;
unsigned long lastFrameTime = 0;
float breatheAngle = 0.0f;

// --- Serial input buffer (non-blocking, length-guarded) ---
char serialBuf[SERIAL_BUF_SIZE];
int  serialBufLen = 0;

// --- Helper: set all LEDs to a color and show ---
void showSolid(CRGB color) {
    fill_solid(leds, NUM_LEDS, color);
    FastLED.show();
}

// --- Helper: advance breathe animation by one frame ---
// Wraps breatheAngle to [0, 2π) to prevent float precision loss over time.
void applyBreathe(float speed, CRGB color) {
    breatheAngle += speed;
    if (breatheAngle >= TWO_PI) breatheAngle -= TWO_PI;
    float scale = (sinf(breatheAngle) + 1.0f) / 2.0f;
    showSolid(CRGB(
        (uint8_t)(color.r * scale),
        (uint8_t)(color.g * scale),
        (uint8_t)(color.b * scale)
    ));
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

void setup() {
    Serial.begin(115200);
    unsigned long serialStart = millis();
    while (!Serial && millis() - serialStart < 3000) { ; }

    FastLED.addLeds<WS2812B, LED_PIN, GRB>(leds, NUM_LEDS);
    FastLED.setBrightness(BRIGHTNESS);
    showSolid(CRGB::Black);

    bootAnimation();

    lastHeartbeat = millis();
    Serial.println("\n--- XIAO ESP32-S3 PRESENCE INDICATOR v0.1.0 ---");
}

// --- Helper: read one complete line from Serial without blocking ---
// Returns true and populates 'out' when a '\n'-terminated line is ready.
// Silently discards lines that exceed SERIAL_BUF_SIZE to prevent heap growth.
bool readSerialLine(String &out) {
    while (Serial.available() > 0) {
        char c = (char)Serial.read();
        if (c == '\r') continue;          // strip CR from CRLF hosts
        if (c == '\n') {
            serialBuf[serialBufLen] = '\0';
            out = String(serialBuf);
            out.trim();
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
    String incoming;
    if (readSerialLine(incoming)) {
        if (incoming.length() == 0) {
            // Ignore empty lines
        } else if (incoming == "HELP" || incoming == "?") {
            Serial.println("--- Teams Presence LED v0.1.0 ---");
            Serial.println("--- Commands ---");
            Serial.println("PING               : Heartbeat to keep connection alive");
            Serial.println("OFF                : Turn off all LEDs");
            Serial.println("SOLID:R,G,B        : Set solid color (0-255)");
            Serial.println("BREATHE:R,G,B      : Moderate pulsing color");
            Serial.println("BREATHE_SLOW:R,G,B : Slow pulsing color");
            Serial.println("RESET              : Reboot the microcontroller");
            Serial.println("HELP or ?          : Show this help message");
        } else if (incoming == "PING") {
            Serial.println("PONG");
            // If we were disconnected, return to idle (OFF) state
            if (currentState == STATE_DISCONNECTED) {
                currentState = STATE_OFF;
                showSolid(CRGB::Black);
            }
        } else if (incoming == "RESET") {
            Serial.println("REBOOTING");
            Serial.flush();
            ESP.restart();
        } else if (incoming == "OFF") {
            currentState = STATE_OFF;
            showSolid(CRGB::Black);
        } else if (incoming.startsWith("SOLID:")) {
            int r, g, b;
            if (sscanf(incoming.c_str(), "SOLID:%d,%d,%d", &r, &g, &b) == 3) {
                if (r >= 0 && r <= 255 && g >= 0 && g <= 255 && b >= 0 && b <= 255) {
                    targetColor = CRGB(r, g, b);
                    currentState = STATE_SOLID;
                    showSolid(targetColor);
                }
            }
        } else if (incoming.startsWith("BREATHE_SLOW:")) {
            int r, g, b;
            if (sscanf(incoming.c_str(), "BREATHE_SLOW:%d,%d,%d", &r, &g, &b) == 3) {
                if (r >= 0 && r <= 255 && g >= 0 && g <= 255 && b >= 0 && b <= 255) {
                    targetColor = CRGB(r, g, b);
                    currentState = STATE_BREATHE_SLOW;
                    breatheAngle = 0;
                }
            }
        } else if (incoming.startsWith("BREATHE:")) {
            int r, g, b;
            if (sscanf(incoming.c_str(), "BREATHE:%d,%d,%d", &r, &g, &b) == 3) {
                if (r >= 0 && r <= 255 && g >= 0 && g <= 255 && b >= 0 && b <= 255) {
                    targetColor = CRGB(r, g, b);
                    currentState = STATE_BREATHE;
                    breatheAngle = 0;
                }
            }
        }
        // Any valid (non-empty) command proves the host is alive — reset watchdog
        lastHeartbeat = now;
    }

    // 2. Watchdog Check — any command resets the timer
    if (currentState != STATE_DISCONNECTED && now - lastHeartbeat > WATCHDOG_TIMEOUT) {
        currentState = STATE_DISCONNECTED;
        breatheAngle = 0;
    }

    // 3. Animation Logic (frame-rate limited)
    if (now - lastFrameTime >= FRAME_MS) {
        lastFrameTime = now;

        if (currentState == STATE_BREATHE) {
            applyBreathe(BREATHE_SPEED_MODERATE, targetColor);
        } else if (currentState == STATE_BREATHE_SLOW) {
            applyBreathe(BREATHE_SPEED_SLOW, targetColor);
        } else if (currentState == STATE_DISCONNECTED) {
            applyBreathe(BREATHE_SPEED_MODERATE, CRGB(255, 255, 255));
        }
    }
}