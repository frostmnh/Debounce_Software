// --- 1. 匯入 (Imports) ---
use log::{info, debug, error, warn, trace};
use evdev::{
    uinput::VirtualDevice,
    AttributeSet, Device, InputEvent, KeyCode, RelativeAxisCode, EventType,
};
use nix::unistd::Uid;
use std::collections::HashMap;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// --- 2. 常數與設定 (Constants & Configuration) ---
const PHYSICAL_MOUSE_PATH: &str = "/dev/input/by-id/usb-Logitech_G102_LIGHTSYNC_Gaming_Mouse_206E36594858-event-mouse";
const DEFAULT_DEBOUNCE_DURATION: Duration = Duration::from_millis(50);
const RECONNECT_DELAY: Duration = Duration::from_secs(3); // 斷線後重試的延遲

// --- 3. 按鍵狀態結構體 (ButtonState Struct) ---
#[derive(Debug)]
struct ButtonState {
    is_pressed: bool,
    last_press_time: Instant,
    debounce_duration: Duration,
}

// --- 4. 主函數 (main) ---
fn main() {
    env_logger::init();

    // --- 新增：設定 Ctrl+C 處理 ---
    // 建立一個原子布林值 `running`，用 Arc 包裹以便在執行緒間安全共享。
    // AtomicBool 可以在多個執行緒中被修改而不會產生資料競爭。
    let running = Arc::new(AtomicBool::new(true));
    // 複製一份 Arc，這樣我們就可以把它的所有權移到 handler 中，而主執行緒仍然保留一份。
    let r = running.clone();

    // `ctrlc::set_handler` 會設定一個新的處理函數，當 Ctrl+C (SIGINT) 被接收時觸發。
    ctrlc::set_handler(move || {
        info!("\n接收到 Ctrl-C 訊號！準備優雅地關閉...");
        // `store(false, ...)` 將原子布林值設為 false。
        // `Ordering::SeqCst` 提供了最強的順序保證，確保所有執行緒都能看到這個變更。
        r.store(false, Ordering::SeqCst);
    }).expect("設定 Ctrl-C 處理器時發生錯誤");

    info!("程式啟動。按 Ctrl+C 停止。");

    // 檢查 Root 權限
    if !Uid::current().is_root() {
        error!("此程式需要 root 權限才能運作。");
        process::exit(1);
    }

    // --- 修改：將核心邏輯放入一個可以被 Ctrl+C 控制的迴圈中 ---
    // `running.load(...)` 讀取原子布林值的當前狀態。
    // 只要 `running` 是 true，迴圈就會繼續。
    while running.load(Ordering::SeqCst) {
        // 呼叫新的 `connect_and_run_loop` 函數，它會嘗試連接並處理事件。
        // 這個函數在裝置斷開時會回傳 Ok，讓外層迴圈可以再次嘗試連接。
        // 只有在發生無法恢復的錯誤時，它才會回傳 Err。
        if let Err(e) = connect_and_run_loop(&running) {
            error!("發生無法恢復的錯誤: {}", e);
            break; // 發生嚴重錯誤，跳出主迴圈
        }

        // 如果是因為裝置斷開而從 `connect_and_run_loop` 返回，
        // 並且程式仍在 `running` 狀態，我們會在這裡等待一下再重試。
        if running.load(Ordering::SeqCst) {
            warn!(
                "與裝置的連線中斷。將在 {} 秒後嘗試重新連接...",
                RECONNECT_DELAY.as_secs()
            );
            thread::sleep(RECONNECT_DELAY);
        }
    }

    info!("程式已停止。");
}

// --- 5. 新增：連接與事件處理迴圈函數 ---
// 這個函數包含了單次「連接 -> 執行 -> 斷開」的完整生命週期。
// 它接收 `running` 狀態的引用，以便在內部可以檢查是否應該提前終止。
fn connect_and_run_loop(running: &Arc<AtomicBool>) -> Result<(), Box<dyn std::error::Error>> {
    // --- 步驟 5.1: 連接實體裝置 ---
    info!("嘗試開啟實體滑鼠: {}", PHYSICAL_MOUSE_PATH);
    // 這裡的 `?` 如果失敗（例如檔案不存在），錯誤會被傳播回 main 函數的迴圈，
    // 但因為我們將其視為可恢復的錯誤，所以 main 迴圈會捕獲它並重試。
    let mut physical_device = match Device::open(PHYSICAL_MOUSE_PATH) {
        Ok(dev) => {
            info!(
                "成功開啟實體滑鼠: {} ({})",
                  dev.name().unwrap_or("未知裝置"),
                  dev.physical_path().unwrap_or("未知路徑")
            );
            dev
        },
        Err(e) => {
            // 如果裝置打不開，這是一個「可恢復」的錯誤，所以我們只回傳 Ok，
            // 讓外層迴圈知道這次嘗試失敗了，然後等待重試。
            warn!("無法開啟實體滑鼠: {}。等待重新連接...", e);
            return Ok(());
        }
    };

    // --- 步驟 5.2: 獨佔裝置與建立虛擬裝置 (與之前類似) ---
    info!("嘗試獨佔 (grab) 實體滑鼠...");
    physical_device.grab()?;
    info!("成功獨佔實體滑鼠。");

    // 建立虛擬裝置... (這部分程式碼不變)
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
    let input_id = physical_device.input_id().clone();
    let mut virtual_device = VirtualDevice::builder()?
    .name("Virtual Debounced Mouse")
    .with_keys(&keys)?
    .with_relative_axes(&rel_axes)?
    .input_id(input_id)
    .build()?;
    info!("虛擬滑鼠 'Virtual Debounced Mouse' 已建立。");
    thread::sleep(Duration::from_secs(1));

    // --- 步驟 5.3: 初始化按鍵狀態 (與之前相同) ---
    let mut button_states = initialize_button_states();

    // --- 步驟 5.4: 內部事件迴圈 ---
    info!("----------------------------------------");
    info!("開始監聽並轉發事件...");
    info!("----------------------------------------");

    // 只要程式還在 `running` 狀態，就持續讀取事件
    while running.load(Ordering::SeqCst) {
        // `fetch_events()` 會阻塞等待事件。
        match physical_device.fetch_events() {
            Ok(events) => {
                for event in events {
                    handle_event(&mut virtual_device, &event, &mut button_states)?;
                }
            },
            Err(e) => {
                // `fetch_events` 失敗最常見的原因是裝置被拔除 (IO Error)。
                // 這是一個我們預期會發生的情況。
                if e.kind() == std::io::ErrorKind::NotFound || e.kind() == std::io::ErrorKind::BrokenPipe {
                    warn!("實體滑鼠裝置已斷開 ({})。", e);
                } else {
                    error!("讀取事件時發生非預期錯誤: {}", e);
                }
                // 無論是哪種 IO 錯誤，都跳出內部迴圈，觸發外部的重連邏輯。
                break;
            }
        }
    }

    // --- 步驟 5.5: 清理 ---
    // 當迴圈結束時 (無論是因 Ctrl+C 還是裝置斷開)，我們來到這裡。
    // Rust 的 Drop trait 會自動處理 virtual_device 的關閉。
    // 對於 physical_device，雖然 drop 也會關閉檔案，但明確呼叫 ungrab 是個好習慣。
    info!("停止監聽，準備清理資源...");
    if let Err(e) = physical_device.ungrab() {
        warn!("釋放 (ungrab) 實體滑鼠時發生錯誤: {}", e);
    } else {
        info!("成功釋放實體滑鼠。");
    }

    Ok(())
}

// --- 6. 新增：將按鍵狀態初始化邏輯提取為獨立函數 ---
// 這樣可以讓 connect_and_run_loop 函數更整潔。
fn initialize_button_states() -> HashMap<KeyCode, ButtonState> {
    info!("正在初始化按鍵去抖動狀態...");
    let mut button_states = HashMap::new();
    let debounced_buttons = [
        KeyCode::BTN_LEFT, KeyCode::BTN_RIGHT, KeyCode::BTN_MIDDLE,
        KeyCode::BTN_SIDE, KeyCode::BTN_EXTRA,
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
    button_states
}

// --- 7. 事件處理函數 (handle_event) ---
// 這個函數保持不變。
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
                1 => {
                    if now.duration_since(state.last_press_time) > state.debounce_duration {
                        debug!(">>> 有效按下 {:?} (傳遞)", key_code);
                        state.last_press_time = now;
                        state.is_pressed = true;
                        virtual_device.emit(&[event.clone()])?;
                    } else {
                        debug!("--- 抖動按下 {:?} (忽略)", key_code);
                    }
                }
                0 => {
                    if state.is_pressed {
                        debug!("<<< 有效釋放 {:?} (傳遞)", key_code);
                        state.is_pressed = false;
                        virtual_device.emit(&[event.clone()])?;
                    } else {
                        debug!("--- 抖動釋放 {:?} (忽略)", key_code);
                    }
                }
                _ => {
                    virtual_device.emit(&[event.clone()])?;
                }
            }
            return Ok(());
        }
    }


    // 將滑鼠移動和其他事件的日誌級別降為 TRACE
    match event.event_type() {
        EventType::RELATIVE => {
            // 使用 trace! 而不是 debug!
            trace!("轉發相對移動: Code {} | 值: {}", event.code(), event.value());
        },
        _ => {}
    }


    virtual_device.emit(&[event.clone()])?;
    Ok(())
}
