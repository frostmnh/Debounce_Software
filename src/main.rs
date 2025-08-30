use evdev::{Device, EventType, KeyCode}; // --- 修正點 1: 移除了 RelativeAxisCode，因為我們無法直接創建它
use nix::unistd::Uid;
use std::process;

const PHYSICAL_MOUSE_PATH: &str = "/dev/input/by-id/usb-Logitech_G102_LIGHTSYNC_Gaming_Mouse_206E36594858-event-mouse";

fn main() {
    if let Err(e) = run() {
        eprintln!("應用程式錯誤: {}", e);
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    if !Uid::current().is_root() {
        eprintln!("警告: 需要 root 權限。");
    }

    println!("嘗試開啟實體滑鼠: {}", PHYSICAL_MOUSE_PATH);
    let mut device = Device::open(PHYSICAL_MOUSE_PATH)?;

    println!("成功開啟: {} ({})",
             device.name().unwrap_or("未知裝置"),
             device.physical_path().unwrap_or("未知路徑")
    );

    println!("----------------------------------------");
    println!("開始監聽事件... 按 Ctrl+C 停止。");
    println!("----------------------------------------");

    loop {
        for event in device.fetch_events()? {
            match event.event_type() {
                EventType::KEY => {
                    // --- 核心修正點 2: 直接使用 KeyCode::new ---
                    // 編譯器告訴我們 KeyCode::new 直接返回 KeyCode，而不是 Option。
                    let key = KeyCode::new(event.code());
                    println!("按鍵事件: {:?} | 值: {}", key, event.value());
                },
                EventType::RELATIVE => {
                    // --- 核心修正點 3: 既然無法轉換，我們先打印原始 code ---
                    // 編譯器告訴我們 RelativeAxisCode 沒有 new 方法。
                    // 為了讓程式能跑起來，我們暫時只打印數字 code。
                    // 這不是最終方案，但這是能讓程式編譯通過的必要步驟。
                    println!("相對移動事件: Code {} | 值: {}", event.code(), event.value());
                },
                EventType::SYNCHRONIZATION => {
                    // 忽略同步事件
                },
                _ => {
                    println!("其他事件: Type {:?} | Code {} | 值: {}", event.event_type(), event.value(), event.code());
                }
            }
        }
    }
}
