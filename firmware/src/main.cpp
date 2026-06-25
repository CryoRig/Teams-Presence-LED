#define FASTLED_RMT5_RECYCLE 1  // Fix RMT channel conflict with USB CDC on ESP32-S3

#include <Arduino.h>
#include <FastLED.h>

// --- Configuration ---
#define LED_PIN     2              // GPIO2 (D1 on XIAO ESP32-S3) — avoids strapping pin GPIO1
#define NUM_LEDS    8              // Number of WS2812B LEDs in the chain
#define BRIGHTNESS  255            // Max brightness — protocol RGB values control intensity
#define WATCHDOG_TIMEOUT 30000     // 30 seconds without PING → disconnected state
#define FPS         60             // Animation frame rate limit

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
float breatheAngle = 0;

// --- Helper: set all LEDs to a color and show ---
void showSolid(CRGB color) {
    fill_solid(leds, NUM_LEDS, color);
    FastLED.show();
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
        delay(1000 / FPS);
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
    Serial.println("\n--- XIAO ESP32-S3 PRESENCE INDICATOR ---");
}

void loop() {
    unsigned long now = millis();

    // 1. Handle Serial Input
    if (Serial.available() > 0) {
        String incoming = Serial.readStringUntil('\n');
        incoming.trim();

        if (incoming.length() == 0) {
            // Ignore empty lines
        } else if (incoming == "PING") {
            Serial.println("PONG");
            lastHeartbeat = now;
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
        // Unknown commands are silently ignored per protocol spec
    }

    // 2. Watchdog Check — only PING resets the timer
    if (currentState != STATE_DISCONNECTED && now - lastHeartbeat > WATCHDOG_TIMEOUT) {
        currentState = STATE_DISCONNECTED;
        breatheAngle = 0;
    }

    // 3. Animation Logic (frame-rate limited)
    if (now - lastFrameTime >= (1000 / FPS)) {
        lastFrameTime = now;

        if (currentState == STATE_BREATHE) {
            breatheAngle += BREATHE_SPEED_MODERATE;
            float scale = (sin(breatheAngle) + 1.0f) / 2.0f;
            CRGB color = CRGB(
                (uint8_t)(targetColor.r * scale),
                (uint8_t)(targetColor.g * scale),
                (uint8_t)(targetColor.b * scale)
            );
            showSolid(color);
        } else if (currentState == STATE_BREATHE_SLOW) {
            breatheAngle += BREATHE_SPEED_SLOW;
            float scale = (sin(breatheAngle) + 1.0f) / 2.0f;
            CRGB color = CRGB(
                (uint8_t)(targetColor.r * scale),
                (uint8_t)(targetColor.g * scale),
                (uint8_t)(targetColor.b * scale)
            );
            showSolid(color);
        } else if (currentState == STATE_DISCONNECTED) {
            breatheAngle += BREATHE_SPEED_MODERATE;
            float scale = (sin(breatheAngle) + 1.0f) / 2.0f;
            uint8_t v = (uint8_t)(255 * scale);
            showSolid(CRGB(v, v, v));
        }
    }
}