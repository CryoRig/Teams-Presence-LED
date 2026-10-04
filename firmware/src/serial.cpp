#include <Arduino.h>
#if defined(CONFIG_IDF_TARGET_ESP32C3)
#include <HWCDC.h>
#endif
#ifdef USE_NEOPIXEL
#include <Adafruit_NeoPixel.h>
#else
#include <FastLED.h>
#endif

#ifndef LED_PIN
#define LED_PIN 2
#endif

#define NUM_LEDS 8

#ifdef USE_NEOPIXEL
struct CRGB {
    uint8_t r, g, b;
    constexpr CRGB(uint8_t red = 0, uint8_t green = 0, uint8_t blue = 0)
        : r(red), g(green), b(blue) {}
    CRGB nscale8_video(uint8_t scale) {
        r = (uint16_t)r * scale / 255;
        g = (uint16_t)g * scale / 255;
        b = (uint16_t)b * scale / 255;
        return *this;
    }
    bool operator!=(const CRGB& other) const {
        return r != other.r || g != other.g || b != other.b;
    }
    static const CRGB Black;
    static const CRGB White;
};
const CRGB CRGB::Black(0, 0, 0);
const CRGB CRGB::White(255, 255, 255);

CRGB blend(CRGB first, CRGB second, uint8_t amount) {
    return CRGB(first.r + ((int16_t)second.r - first.r) * amount / 255,
                first.g + ((int16_t)second.g - first.g) * amount / 255,
                first.b + ((int16_t)second.b - first.b) * amount / 255);
}

void fill_solid(CRGB *pixels, uint8_t count, CRGB color) {
    for (uint8_t index = 0; index < count; ++index) pixels[index] = color;
}

Adafruit_NeoPixel strip(NUM_LEDS, LED_PIN, NEO_GRB + NEO_KHZ800);
struct {
    void addLeds(CRGB *, uint8_t) { strip.begin(); }
    void setBrightness(uint8_t value) { strip.setBrightness(value); }
    void setMaxPowerInVoltsAndMilliamps(uint8_t, uint16_t) {}
    void clear(bool) { strip.clear(); strip.show(); }
    void show();
} FastLED;
#endif

CRGB leds[NUM_LEDS];
#ifdef USE_NEOPIXEL
void decltype(FastLED)::show() {
    for (uint8_t index = 0; index < NUM_LEDS; ++index) {
        strip.setPixelColor(index, leds[index].r, leds[index].g, leds[index].b);
    }
    strip.show();
}
#endif
CRGB targetColor = CRGB::Black;
CRGB previousColor = CRGB::Black;
uint8_t currentMode = 0x02;
uint8_t brightness = 191;
bool outputChanged = true;
uint16_t transitionDuration = 500;
unsigned long transitionStart = 0;
unsigned long lastCommand = 0;
unsigned long lastFrame = 0;
uint8_t packet[7];
uint8_t packetLength = 0;

void sendPacket(uint8_t status, uint8_t first = 0, uint8_t second = 0,
                uint8_t third = 0, uint8_t fourth = 0) {
    const uint8_t checksum = 0xA5 ^ status ^ first ^ second ^ third ^ fourth;
    const uint8_t response[] = {0xA5, status, first, second, third, fourth, checksum};
    Serial.write(response, sizeof(response));
}

CRGB displayedColor(unsigned long now) {
    if (currentMode == 0x02) return CRGB::Black;
    if (currentMode == 0x03) return targetColor;
    const uint16_t period = currentMode == 0x05 ? 5000 : 3000;
    const uint16_t phase = (now % period) * 510UL / period;
    const uint8_t level = phase < 255 ? phase : 510 - phase;
    return targetColor.nscale8_video(level);
}

void handlePacket(const uint8_t *payload) {
    const uint8_t command = payload[0];
    const unsigned long now = millis();
    switch (command) {
        case 0x01:
            sendPacket(0x01);
            break;
        case 0x02:
        case 0x03:
        case 0x04:
        case 0x05:
            previousColor = displayedColor(now);
            targetColor = CRGB(payload[1], payload[2], payload[3]);
            currentMode = command;
            transitionStart = now;
            break;
        case 0x06:
            brightness = payload[1];
            FastLED.setBrightness(brightness);
            outputChanged = true;
            break;
        case 0x07:
            transitionDuration = ((uint16_t)payload[1] << 8) | payload[2];
            if (transitionDuration > 10000) transitionDuration = 10000;
            break;
        case 0x08:
            break;
        case 0x0A:
            sendPacket(0x0A, 0, 6, 2, 2);
            break;
        case 0x0B:
        case 0x0C:
        case 0x0D:
        case 0x0E:
            sendPacket(0xFF);
            break;
        default:
            return;
    }
    lastCommand = now;
}

void setup() {
    Serial.begin(115200);
#ifdef USE_NEOPIXEL
    FastLED.addLeds(leds, NUM_LEDS);
#else
    FastLED.addLeds<WS2812B, LED_PIN, GRB>(leds, NUM_LEDS);
#endif
    FastLED.setBrightness(brightness);
    FastLED.setMaxPowerInVoltsAndMilliamps(5, 480);
    FastLED.clear(true);
    lastCommand = millis();
}

void loop() {
    while (Serial.available()) {
        const uint8_t received = Serial.read();
        if (packetLength == 0 && received != 0xA5) continue;
        packet[packetLength++] = received;
        if (packetLength == sizeof(packet)) {
            uint8_t checksum = 0;
            for (uint8_t index = 0; index < sizeof(packet) - 1; ++index) checksum ^= packet[index];
            if (checksum == packet[6]) handlePacket(packet + 1);
            packetLength = 0;
        }
    }

    const unsigned long now = millis();
    if (now - lastFrame < 17) return;
    lastFrame = now;

    CRGB color;
    if (now - lastCommand > 60000) {
        const uint8_t level = ((now % 3000) * 510UL / 3000);
        color = CRGB::White;
        color.nscale8_video(level < 255 ? level : 510 - level);
    } else {
        color = displayedColor(now);
        const unsigned long elapsed = now - transitionStart;
        if (elapsed < transitionDuration) {
            color = blend(previousColor, color, elapsed * 255UL / transitionDuration);
        }
    }
    if (outputChanged || leds[0] != color) {
        fill_solid(leds, NUM_LEDS, color);
        FastLED.show();
        outputChanged = false;
    }
}