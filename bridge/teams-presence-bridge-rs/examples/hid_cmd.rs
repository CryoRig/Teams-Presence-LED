//! Dev helper: send a single protocol command to the Teams Presence Bridge device.
//!
//!   cargo run --example hid_cmd -- <command> [p1] [p2] [p3]
//!
//! Commands: ping | off | solid R G B | breathe R G B | breathe_slow R G B |
//!           brightness N | transition MS | reset | bootloader | version
//!
//! Watchdog-recovery test (ISSUES.md O2): stop the bridge, then
//!   cargo run --example hid_cmd -- solid 0 200 0      # green
//!   (wait > 60 s until the strip pulses white = disconnected)
//!   cargo run --example hid_cmd -- brightness 191     # non-PING command
//!   -> expected: white pulse stops, green fades back in.
use std::time::{Duration, Instant};

const REPORT_ID: u8 = 0x06;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(name) = args.first() else {
        eprintln!("usage: hid_cmd <command> [p1] [p2] [p3]  (see source for the list)");
        std::process::exit(2);
    };
    let p = |i: usize| -> u8 {
        args.get(i)
            .map(|s| s.parse::<u16>().unwrap_or_else(|_| panic!("bad numeric arg '{}'", s)))
            .map(|v| u8::try_from(v).unwrap_or_else(|_| panic!("arg {} out of range 0-255", i)))
            .unwrap_or(0)
    };

    let (cmd, p1, p2, p3) = match name.as_str() {
        "ping" => (0x01, 0, 0, 0),
        "off" => (0x02, 0, 0, 0),
        "solid" => (0x03, p(1), p(2), p(3)),
        "breathe" => (0x04, p(1), p(2), p(3)),
        "breathe_slow" => (0x05, p(1), p(2), p(3)),
        "brightness" => (0x06, p(1), 0, 0),
        "transition" => {
            let ms: u16 = args.get(1).and_then(|s| s.parse().ok()).expect("transition needs ms");
            (0x07, (ms >> 8) as u8, (ms & 0xFF) as u8, 0)
        }
        "reset" => (0x08, 0, 0, 0),
        "bootloader" => (0x09, 0, 0, 0),
        "version" => (0x0A, 0, 0, 0),
        other => {
            eprintln!("unknown command '{}'", other);
            std::process::exit(2);
        }
    };

    let api = hidapi::HidApi::new().expect("hidapi init");
    let info = api
        .device_list()
        .find(|d| d.usage_page() == 0xFF00 && d.product_string().is_some_and(|s| s.contains("Teams Presence Bridge")))
        .expect("Teams Presence Bridge not found");
    println!("Device VID:{:04X} PID:{:04X}", info.vendor_id(), info.product_id());
    let dev = info.open_device(&api).expect("open");

    dev.write(&[REPORT_ID, cmd, p1, p2, p3, 0]).expect("write");
    println!("Sent {} -> cmd 0x{:02X} [{}, {}, {}]", name, cmd, p1, p2, p3);

    // Show the device's reply (OK / PONG / VERSION), if any arrives within 500 ms.
    let mut buf = [0u8; 6];
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        if let Ok(n) = dev.read_timeout(&mut buf, 50)
            && n > 1
            && buf[0] == REPORT_ID
        {
            match buf[1] {
                0x01 => println!("Reply: PONG"),
                0x02 => println!("Reply: OK"),
                0x0A if n >= 6 => println!("Reply: VERSION v{}.{}.{} variant {}", buf[2], buf[3], buf[4], buf[5]),
                0xFF => println!("Reply: ERROR (unknown command)"),
                other => println!("Reply: 0x{:02X}", other),
            }
            return;
        }
    }
    println!("(no reply — expected for reset/bootloader)");
}
