#define FASTLED_RMT5_RECYCLE 1  // Fix RMT channel conflict with USB CDC on ESP32-S3

#include <Arduino.h>
#include <FastLED.h>
#include <Wifi.h>
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

void setup() {
    // Explicitly disable unused radios
    WiFi.mode(WIFI_OFF);
    btStop();

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
            Serial.println("PING               : Heartbeat to keep connection alive");
            Serial.println("OFF                : Turn off all LEDs");
            Serial.println("SOLID:R,G,B        : Set solid color (0-255)");
            Serial.println("BREATHE:R,G,B      : Moderate pulsing color");
            Serial.println("BREATHE_SLOW:R,G,B : Slow pulsing color");
            Serial.println("BRIGHTNESS:N       : Set global brightness (0-255)");
            Serial.println("TRANSITION:N       : Set transition duration in ms (0-10000)");
            Serial.println("RESET              : Reboot the microcontroller");
            Serial.println("HELP or ?          : Show this help message");
            lastHeartbeat = now;
        } else if (strcmp(serialBuf, "PING") == 0) {
            Serial.println("PONG");
            // If we were disconnected, return to idle (OFF) state
            if (currentState == STATE_DISCONNECTED) {
                startStateTransition();
                currentState = STATE_OFF;
            }
            lastHeartbeat = now;
        } else if (strcmp(serialBuf, "RESET") == 0) {
            Serial.println("REBOOTING");
            Serial.flush();
            ESP.restart();
        } else if (strncmp(serialBuf, "BRIGHTNESS:", 11) == 0) {
            int val;
            if (sscanf(serialBuf, "BRIGHTNESS:%d", &val) == 1) {
                if (val >= 0 && val <= 255) {
                    FastLED.setBrightness(val);
                    lastHardwareColor = CRGB::Black; // Invalidate cache to force refresh
                    FastLED.show();
                    Serial.println("OK");
                    lastHeartbeat = now;
                }
            }
        } else if (strncmp(serialBuf, "TRANSITION:", 11) == 0) {
            unsigned int val;
            if (sscanf(serialBuf, "TRANSITION:%u", &val) == 1) {
                if (val <= 10000) {
                    transitionDurationMs = val;
                } else {
                    transitionDurationMs = 10000;
                }
                Serial.println("OK");
                lastHeartbeat = now;
            }
        } else if (strcmp(serialBuf, "OFF") == 0) {
            startStateTransition();
            currentState = STATE_OFF;
            Serial.println("OK");
            lastHeartbeat = now;
        } else if (strncmp(serialBuf, "SOLID:", 6) == 0) {
            int r, g, b;
            if (sscanf(serialBuf, "SOLID:%d,%d,%d", &r, &g, &b) == 3) {
                if (r >= 0 && r <= 255 && g >= 0 && g <= 255 && b >= 0 && b <= 255) {
                    startStateTransition();
                    targetColor = CRGB(r, g, b);
                    currentState = STATE_SOLID;
                    Serial.println("OK");
                    lastHeartbeat = now;
                }
            }
        } else if (strncmp(serialBuf, "BREATHE_SLOW:", 13) == 0) {
            int r, g, b;
            if (sscanf(serialBuf, "BREATHE_SLOW:%d,%d,%d", &r, &g, &b) == 3) {
                if (r >= 0 && r <= 255 && g >= 0 && g <= 255 && b >= 0 && b <= 255) {
                    startStateTransition();
                    targetColor = CRGB(r, g, b);
                    currentState = STATE_BREATHE_SLOW;
                    Serial.println("OK");
                    lastHeartbeat = now;
                }
            }
        } else if (strncmp(serialBuf, "BREATHE:", 8) == 0) {
            int r, g, b;
            if (sscanf(serialBuf, "BREATHE:%d,%d,%d", &r, &g, &b) == 3) {
                if (r >= 0 && r <= 255 && g >= 0 && g <= 255 && b >= 0 && b <= 255) {
                    startStateTransition();
                    targetColor = CRGB(r, g, b);
                    currentState = STATE_BREATHE;
                    Serial.println("OK");
                    lastHeartbeat = now;
                }
            }
        } else {
            Serial.print("ERR:UNKNOWN_CMD:");
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