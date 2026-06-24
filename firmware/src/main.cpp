#include <Arduino.h>

void setup() {
  Serial.begin(115200);
  while (!Serial) {
    ; // Wait for native USB serial to be ready
  }
  delay(1000); 
  Serial.println("\n--- XIAO ESP32-S3 BOOT ---");
}

void loop() {
  if (Serial.available() > 0) {
    String incoming = Serial.readStringUntil('\n');
    incoming.trim();

    if (incoming == "PING") {
      Serial.println("PONG");
    } else if (incoming != "") {
      Serial.print("Unknown Command: ");
      Serial.println(incoming);
    }
  }
}