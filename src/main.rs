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
use std::collections::HashMap; // <--- 匯入 HashMap
use std::process; // 用於在發生無法恢復的錯誤時終止程式
use std::thread;  // 用於讓程式暫停 (sleep)
// --- 已修正：將 Duration 和 Instant 合併到一行 ---
use std::time::{Duration, Instant}; // 用於定義時間長度 和 計時

// --- 2. 常數與設定 (Constants & Configuration) ---
const PHYSICAL_MOUSE_PATH: &str = "/dev/input/by-id/usb-Logitech_G102_LIGHTSYNC_Gaming_Mouse_206E36594858-event-mouse";

// --- 新增：定義去抖動設定 ---
// 1. 全域預設的去抖動延遲
const DEFAULT_DEBOUNCE_DURATION: Duration = Duration::from_millis(50); // 50 毫秒

// --- 新增：定義按鍵狀態的結構體 ---
// 這個結構體用於儲存每個我們關心的按鍵的當前狀態
#[derive(Debug)] // 加上 Debug derive 以便我們可以列印它來除錯
struct ButtonState {
    is_pressed: bool,          // 記錄按鍵當前是否被認為是「已按下」狀態
    last_press_time: Instant,  // 記錄上一次有效按下的時間點
    debounce_duration: Duration, // 此特定按鍵的去抖動延遲
}

// --- 3. 主函數 (main) ---
fn main() {
    // 初始化 env_logger。這一步是日誌系統工作的關鍵。
    env_logger::init();

    // 呼叫核心邏輯函數 run()，並處理其回傳的 Result。
    match run() {
        Ok(_) => info!("程式正常結束。"),
        Err(e) => {
            error!("應用程式錯誤: {}", e);
            process::exit(1); // 使用非零狀態碼退出，表示程式因錯誤而終止。
        }
    }
}

// --- 4. 核心邏輯函數 (run) ---
fn run() -> Result<(), Box<dyn std::error::Error>> {
    if !Uid::current().is_root() {
        warn!("偵測到非 root 使用者，程式可能無法正常工作。");
        return Err("此程式需要 root 權限。".into());
    }

    info!("嘗試開啟實體滑鼠: {}", PHYSICAL_MOUSE_PATH);
    let mut physical_device = Device::open(PHYSICAL_MOUSE_PATH)?;
    info!(
        "成功開啟實體滑鼠: {} ({})",
          physical_device.name().unwrap_or("未知裝置"),
          physical_device.physical_path().unwrap_or("未知路徑")
    );

    info!("嘗試獨佔 (grab) 實體滑鼠...");
    physical_device.grab()?;
    info!("成功獨佔實體滑鼠。");

    let mut keys = AttributeSet::<KeyCode>::new();
    keys.insert(KeyCode::BTN_LEFT);
    keys.insert(KeyCode::BTN_RIGHT);
    keys.insert(KeyCode::BTN_MIDDLE);
    keys.insert(KeyCode::BTN_SIDE);
    keys.insert(KeyCode::BTN_EXTRA);

    let mut rel_axes = AttributeSet::<RelativeAxisCode>::new();
    rel_axes.insert(RelativeAxisCode::REL_X);
    rel_axes.insert(RelativeAxisCode::REL_Y);
    rel_axes.insert(RelativeAxisCode::REL_WHEEL);
    rel_axes.insert(RelativeAxisCode::REL_HWHEEL);

    info!("正在建立虛擬滑鼠...");
    let mut virtual_device_builder = VirtualDevice::builder()?
    .name("Virtual Debounced Mouse")
    .with_keys(&keys)?
    .with_relative_axes(&rel_axes)?;

    let input_id = physical_device.input_id();
    virtual_device_builder = virtual_device_builder.input_id(input_id.clone());
    let mut virtual_device = virtual_device_builder.build()?;
    info!(
        "虛擬滑鼠 'Virtual Debounced Mouse' 已建立 (Vendor: {}, Product: {}).",
          input_id.vendor(), input_id.product()
    );
    thread::sleep(Duration::from_secs(1));

    // --- 初始化按鍵狀態管理 ---
    info!("正在初始化按鍵去抖動狀態...");
    let mut button_states: HashMap<KeyCode, ButtonState> = HashMap::new();

    let debounced_buttons = [
        KeyCode::BTN_LEFT, KeyCode::BTN_RIGHT, KeyCode::BTN_MIDDLE,
        KeyCode::BTN_SIDE, KeyCode::BTN_EXTRA
    ];

    let custom_debounce_durations: HashMap<KeyCode, Duration> = [
        (KeyCode::BTN_SIDE, Duration::from_millis(350)),
        (KeyCode::BTN_EXTRA, Duration::from_millis(300)),
    ].iter().cloned().collect();

    for &key_code in &debounced_buttons {
        let debounce_duration = custom_debounce_durations
        .get(&key_code)
        .cloned()
        .unwrap_or(DEFAULT_DEBOUNCE_DURATION);

        button_states.insert(
            key_code,
            ButtonState {
                is_pressed: false,
                last_press_time: Instant::now() - Duration::from_secs(3600),
                             debounce_duration,
            },
        );
        info!(
            "  - 按鍵 {:?}: 去抖動延遲設定為 {} ms",
            key_code,
            debounce_duration.as_millis()
        );
    }
    info!("按鍵狀態初始化完成。");

    // --- 進入主事件迴圈 ---
    info!("----------------------------------------");
    info!("開始監聽並轉發事件... 按 Ctrl+C 停止。");
    info!("----------------------------------------");

    loop {
        for event in physical_device.fetch_events()? {
            handle_event(&mut virtual_device, &event, &mut button_states)?;
        }
    }
}

// --- 新的事件處理函數，包含去抖動邏輯 ---
fn handle_event(
    virtual_device: &mut VirtualDevice,
    event: &InputEvent,
    button_states: &mut HashMap<KeyCode, ButtonState>,
) -> Result<(), Box<dyn std::error::Error>> {
    if event.event_type() == EventType::KEY {
        let key_code = KeyCode::new(event.code());

        if let Some(state) = button_states.get_mut(&key_code) {
            let now = Instant::now();

            match event.value() {
                1 => { // 按下
                    if now.duration_since(state.last_press_time) > state.debounce_duration {
                        debug!(
                            ">>> 有效按下 {:?} (延遲: {}ms, 傳遞)",
                               key_code,
                               state.debounce_duration.as_millis()
                        );
                        state.last_press_time = now;
                        state.is_pressed = true;
                        virtual_device.emit(&[event.clone()])?;
                    } else {
                        debug!(
                            "--- 抖動按下 {:?} (延遲: {}ms, 忽略)",
                               key_code,
                               state.debounce_duration.as_millis()
                        );
                    }
                }
                0 => { // 釋放
                    if state.is_pressed {
                        debug!("<<< 有效釋放 {:?} (傳遞)", key_code);
                        state.is_pressed = false;
                        virtual_device.emit(&[event.clone()])?;
                    } else {
                        debug!("--- 抖動釋放 {:?} (忽略)", key_code);
                    }
                }
                _ => { // 其他 (例如重複事件)
                    virtual_device.emit(&[event.clone()])?;
                }
            }
            return Ok(());
        }
    }

    // 對於非受控按鍵事件，或非按鍵事件 (如滑鼠移動)，直接轉發
    match event.event_type() {
        EventType::SYNCHRONIZATION => {}
        EventType::RELATIVE => {
            // 為了避免日誌過於雜亂，可以將相對移動事件的日誌級別設為 trace
            // 但目前用 debug 也可以
            debug!("轉發相對移動: Code {} | 值: {}", event.code(), event.value());
        }
        _ => {
            debug!("轉發其他事件: Type {:?} | Code {} | 值: {}", event.event_type(), event.code(), event.value());
        }
    }
    virtual_device.emit(&[event.clone()])?;

    Ok(())
}
