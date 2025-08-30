// --- 1. 匯入 (Imports) ---
// 匯入 log crate 提供的日誌宏，用於取代 println!
use log::{info, debug, error, warn};

// 從 evdev crate 匯入我們需要的所有類型和結構
use evdev::{
    uinput::VirtualDevice, // 用於建立和操作虛擬裝置
    AttributeSet,          // 一個集合，用於定義裝置支援哪些按鍵或軸
    Device,                // 代表一個實體的輸入裝置
    InputEvent,            // 代表一個單一的輸入事件 (如滑鼠移動、按鍵按下)
    KeyCode,               // 按鍵事件的代碼 (例如 BTN_LEFT)
    RelativeAxisCode,      // 相對移動軸的代碼 (例如 REL_X)
    EventType,             // 事件的類型 (例如 KEY, RELATIVE)
};
// 從 nix crate 匯入與 Unix 系統互動的功能
use nix::unistd::Uid; // 用於獲取目前使用者的 ID，以檢查 root 權限
// 從標準函式庫 (std) 匯入
use std::process; // 用於在發生無法恢復的錯誤時終止程式
use std::thread;  // 用於讓程式暫停 (sleep)
use std::time::Duration; // 用於定義時間長度 (例如秒、毫秒)

// --- 2. 常數定義 (Constants) ---
// 將實體滑鼠的裝置路徑定義為一個全域常數。
// 這樣可以方便地在程式頂部修改，而不用深入程式碼中尋找。
const PHYSICAL_MOUSE_PATH: &str = "/dev/input/by-id/usb-Logitech_G102_LIGHTSYNC_Gaming_Mouse_206E36594858-event-mouse";

// --- 3. 主函數 (main) ---
// 這是 Rust 程式的入口點。
fn main() {
    // 初始化 env_logger。這一步是日誌系統工作的關鍵。
    // 它會讀取 RUST_LOG 環境變數來設定日誌級別。
    // 如果沒有設定，預設只會顯示 INFO、WARN 和 ERROR 級別的日誌。
    env_logger::init();

    // 呼叫核心邏輯函數 run()，並處理其回傳的 Result。
    // `match` 是 Rust 中處理 Result 和 Enum 的標準方式。
    match run() {
        // 如果 run() 成功完成 (回傳 Ok)，就記錄一條資訊日誌。
        Ok(_) => info!("程式正常結束。"),
        // 如果 run() 失敗 (回傳 Err)，就記錄一條錯誤日誌，並退出程式。
        Err(e) => {
            error!("應用程式錯誤: {}", e);
            process::exit(1); // 使用非零狀態碼退出，表示程式因錯誤而終止。
        }
    }
}

// --- 4. 核心邏輯函數 (run) ---
// 將主要邏輯放在一個獨立的函數中，可以讓 main 函數保持簡潔，
// 並且方便地使用 `?` 運算子來進行錯誤傳播。
// `-> Result<(), Box<dyn std::error::Error>>` 表示此函數可能回傳錯誤。
fn run() -> Result<(), Box<dyn std::error::Error>> {
    // 檢查程式是否以 root 權限運行。
    // 讀取 /dev/input/ 下的裝置和建立 uinput 裝置都需要 root 權限。
    if !Uid::current().is_root() {
        warn!("偵測到非 root 使用者，程式可能無法正常工作。");
        // 回傳一個錯誤，這個錯誤會被 main 函數捕獲並處理。
        return Err("此程式需要 root 權限。".into());
    }

    // --- 步驟 4.1: 開啟並獨佔實體裝置 ---
    info!("嘗試開啟實體滑鼠: {}", PHYSICAL_MOUSE_PATH);
    let mut physical_device = Device::open(PHYSICAL_MOUSE_PATH)?; // `?` 如果失敗，會立即回傳錯誤
    info!(
        "成功開啟實體滑鼠: {} ({})",
          physical_device.name().unwrap_or("未知裝置"),
          physical_device.physical_path().unwrap_or("未知路徑")
    );

    info!("嘗試獨佔 (grab) 實體滑鼠...");
    // `grab()` 是關鍵步驟。它會攔截所有來自此裝置的事件，
    // 使它們只被我們的程式接收，而不會被作業系統的其他部分 (如桌面環境) 接收。
    physical_device.grab()?;
    info!("成功獨佔實體滑鼠。");

    // --- 步驟 4.2: 定義虛擬裝置的能力 ---
    // 我們需要明確告訴系統，我們將要建立的虛擬滑鼠支援哪些功能。
    // 建立一個按鍵集合
    let mut keys = AttributeSet::<KeyCode>::new();
    keys.insert(KeyCode::BTN_LEFT);
    keys.insert(KeyCode::BTN_RIGHT);
    keys.insert(KeyCode::BTN_MIDDLE);
    keys.insert(KeyCode::BTN_SIDE);
    keys.insert(KeyCode::BTN_EXTRA);

    // 建立一個相對移動軸集合
    let mut rel_axes = AttributeSet::<RelativeAxisCode>::new();
    rel_axes.insert(RelativeAxisCode::REL_X); // 水平移動
    rel_axes.insert(RelativeAxisCode::REL_Y); // 垂直移動
    rel_axes.insert(RelativeAxisCode::REL_WHEEL); // 滾輪滾動
    rel_axes.insert(RelativeAxisCode::REL_HWHEEL); // 水平滾輪滾動

    // --- 步驟 4.3: 建立虛擬裝置 ---
    info!("正在建立虛擬滑鼠...");
    // 使用 `builder` 模式來逐步設定虛擬裝置的屬性。
    let mut virtual_device_builder = VirtualDevice::builder()?
    .name("Virtual Debounced Mouse") // 設定裝置名稱
    .with_keys(&keys)?               // 設定支援的按鍵
    .with_relative_axes(&rel_axes)?; // 設定支援的相對軸

    // 為了讓虛擬裝置的行為盡可能接近實體裝置，我們複製其 ID 資訊。
    let input_id = physical_device.input_id();
    // `.clone()` 是必要的，因為 `input_id()` 方法會轉移所有權。
    // 我們複製一份，將複本的所有權交給 builder，原始的 `input_id` 仍可繼續使用。
    virtual_device_builder = virtual_device_builder.input_id(input_id.clone());

    // `build()` 方法最終會根據我們的設定，在系統中建立出 `/dev/uinput` 裝置。
    let mut virtual_device = virtual_device_builder.build()?;
    info!(
        "虛擬滑鼠 'Virtual Debounced Mouse' 已建立 (Vendor: {}, Product: {}).",
          input_id.vendor(), input_id.product()
    );

    // 短暫暫停一秒，給作業系統一些時間來辨識和初始化這個新的虛擬裝置。
    thread::sleep(Duration::from_secs(1));

    // --- 步驟 4.4: 進入主事件迴圈 ---
    info!("----------------------------------------");
    info!("開始監聽並轉發事件... 按 Ctrl+C 停止。");
    info!("----------------------------------------");

    // `loop` 建立一個無限迴圈，程式會一直停在這裡處理事件，直到被手動終止 (Ctrl+C)。
    loop {
        // `fetch_events()` 會從實體裝置讀取一批可用的事件。
        // 這是一個阻塞操作，如果沒有事件，程式會在這裡等待。
        for event in physical_device.fetch_events()? {
            // 對於讀取到的每一個事件，都呼叫 forward_event 函數來處理它。
            forward_event(&mut virtual_device, &event)?;
        }
    }
}

// --- 5. 事件處理函數 (forward_event) ---
// 這個函數負責處理單一事件：記錄日誌，並將其轉發到虛擬裝置。
fn forward_event(
    virtual_device: &mut VirtualDevice,
    event: &InputEvent,
) -> Result<(), Box<dyn std::error::Error>> {
    // 使用 `match` 來根據事件的類型執行不同的日誌記錄邏輯。
    match event.event_type() {
        EventType::KEY => {
            // 對於按鍵事件，使用 KeyCode::new() 將數字代碼轉換為可讀的枚舉名稱。
            let key_code = KeyCode::new(event.code());
            debug!("轉發按鍵事件: {:?} | 值: {}", key_code, event.value());
        },
        EventType::RELATIVE => {
            // 對於相對移動事件，我們已知無法輕易轉換其 code，所以直接記錄原始數字。
            // 常見的 code: 0=REL_X, 1=REL_Y, 8=REL_WHEEL
            debug!("轉發相對移動: Code {} | 值: {}", event.code(), event.value());
        },
        // 同步事件 (SYNCHRONIZATION) 非常頻繁，通常我們不關心它的日誌，所以留空。
        EventType::SYNCHRONIZATION => {},
        // `_` 是一個通配符，匹配任何其他未明確處理的事件類型。
        _ => {
            debug!("轉發其他事件: Type {:?} | Code {} | 值: {}", event.event_type(), event.code(), event.value());
        }
    }

    // `emit()` 是將事件寫入虛擬裝置的核心方法。
    // 它接收一個事件切片 `&[InputEvent]`，可以一次性發送多個事件。
    // `emit` 會自動為我們處理 `SYN_REPORT` 同步事件，非常方便。
    virtual_device.emit(&[event.clone()])?;

    // 如果一切順利，回傳 Ok。
    Ok(())
}
