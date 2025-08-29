#!/usr/bin/env python
# -*- coding: utf-8 -*-

from evdev import InputDevice, categorize, ecodes, UInput
import time
import os
import traceback

# --- 設定 ---
PHYSICAL_MOUSE_PATH = "/dev/input/by-id/usb-Logitech_G102_LIGHTSYNC_Gaming_Mouse_206E36594858-event-mouse" # 替換成你的路徑

# --- 新增：為不同按鍵設定不同的去抖動延遲 ---
# 1. 全局預設的去抖動超時時間 (秒)
DEFAULT_DEBOUNCE_TIMEOUT = 0.05  # 50 毫秒

# 2. 為特定按鍵指定不同的超時時間 (秒)
#    使用字典，鍵是 ecodes.BTN_*，值是超時時間
#    如果某個按鍵沒有在這裡指定，則使用 DEFAULT_DEBOUNCE_TIMEOUT
CUSTOM_DEBOUNCE_TIMEOUTS = {
    ecodes.BTN_SIDE: 0.35,    # 例如，為 BTN_SIDE 設定 100 毫秒的延遲
    ecodes.BTN_EXTRA: 0.3,  # 例如，為 BTN_EXTRA 設定 80 毫秒的延遲
    # ecodes.BTN_LEFT: 0.03, # 如果需要，也可以為左鍵設定更短的延遲
}

# --- Helper function ---
def get_button_name(code):
    # ... (get_button_name 函數不變)
    common_buttons = {
        ecodes.BTN_LEFT: "BTN_LEFT", ecodes.BTN_RIGHT: "BTN_RIGHT",
        ecodes.BTN_MIDDLE: "BTN_MIDDLE", ecodes.BTN_SIDE: "BTN_SIDE",
        ecodes.BTN_EXTRA: "BTN_EXTRA", ecodes.BTN_FORWARD: "BTN_FORWARD",
        ecodes.BTN_BACK: "BTN_BACK", ecodes.BTN_TASK: "BTN_TASK",
    }
    return common_buttons.get(code, f"Code {code}")

def main():
    button_states = {}
    physical_mouse = None
    ui = None

    while True:
        try:
            physical_mouse = None
            ui = None

            if os.geteuid() != 0:
                print("警告: 需要 root 權限...")

            print(f"嘗試開啟實體滑鼠: {PHYSICAL_MOUSE_PATH}")
            physical_mouse = InputDevice(PHYSICAL_MOUSE_PATH) # <-- FileNotFoundError 可能發生在這裡
            print(f"成功開啟: {physical_mouse.name} ({physical_mouse.phys})")

            capabilities = {
                ecodes.EV_KEY: [
                    ecodes.BTN_LEFT, ecodes.BTN_RIGHT, ecodes.BTN_MIDDLE,
                    ecodes.BTN_SIDE, ecodes.BTN_EXTRA,
                    ecodes.BTN_FORWARD, ecodes.BTN_BACK, ecodes.BTN_TASK,
                ],
                ecodes.EV_REL: [
                    ecodes.REL_X, ecodes.REL_Y,
                    ecodes.REL_WHEEL, ecodes.REL_HWHEEL,
                ],
            }
            print(f"準備建立虛擬滑鼠，能力: {capabilities}")

            if not button_states and ecodes.EV_KEY in capabilities:
                for btn_code in capabilities[ecodes.EV_KEY]:
                    timeout = CUSTOM_DEBOUNCE_TIMEOUTS.get(btn_code, DEFAULT_DEBOUNCE_TIMEOUT)
                    button_states[btn_code] = {'last_press': 0, 'is_pressed': False, 'timeout': timeout}
                    print(f"  - 按鍵 {get_button_name(btn_code)}: 去抖動延遲 {timeout*1000:.0f} ms")
                print(f"將為以上按鍵應用去抖動邏輯。")

            ui = UInput(capabilities, name='VirtualDebouncedMouse', version=0x1)
            print("虛擬滑鼠 'VirtualDebouncedMouse' 已建立.")
            time.sleep(0.5)

            try:
                print("嘗試獨佔實體滑鼠...")
                physical_mouse.grab() # <-- IOError (OSError) 可能發生在這裡
                print("已獨佔實體滑鼠.")
            except IOError as e:
                print(f"!!! 無法獨佔滑鼠: {e}")
                raise # 重新拋出異常，讓外層的 except 捕獲並處理

            print(f"開始監聽事件，應用獨立按鍵去抖動延遲。")
            print("按 Ctrl+C 停止.")

            for event in physical_mouse.read_loop(): # <-- OSError 19 可能發生在這裡
                if (event.type == ecodes.EV_MSC and event.code == ecodes.MSC_SCAN) or \
                   (event.type == ecodes.EV_REL and event.code == 11):
                    continue

                if event.type == ecodes.EV_KEY and event.code in button_states:
                    button_code = event.code
                    state = button_states[button_code]
                    button_name = get_button_name(button_code)
                    current_time = time.monotonic()

                    current_button_timeout = state['timeout']

                    if event.value == 1:
                        if (current_time - state['last_press']) > current_button_timeout:
                            print(f">>> 有效按下 {button_name} (延遲: {current_button_timeout*1000:.0f}ms, 傳遞)")
                            try:
                                ui.write(ecodes.EV_KEY, button_code, 1)
                                ui.syn()
                                state['last_press'] = current_time
                                state['is_pressed'] = True
                            except Exception as write_error:
                                print(f"!!! 寫入按下 {button_name} 時錯誤: {write_error}")
                        else:
                            print(f"--- 抖動按下 {button_name} (延遲: {current_button_timeout*1000:.0f}ms, 忽略)")
                    elif event.value == 0:
                        if state['is_pressed']:
                            print(f"<<< 有效釋放 {button_name} (傳遞)")
                            try:
                                ui.write(ecodes.EV_KEY, button_code, 0)
                                ui.syn()
                                state['is_pressed'] = False
                            except Exception as write_error:
                                print(f"!!! 寫入釋放 {button_name} 時錯誤: {write_error}")
                    continue

                elif event.type == ecodes.EV_SYN:
                    continue
                else:
                    type_supported = event.type in capabilities
                    code_supported = False
                    if type_supported:
                        if event.type in [ecodes.EV_KEY, ecodes.EV_REL]:
                            code_supported = event.code in capabilities.get(event.type, [])
                        else:
                            code_supported = True
                    if type_supported and code_supported:
                        try:
                            ui.write(event.type, event.code, event.value)
                            ui.syn()
                        except Exception as write_error:
                            print(f"!!! 轉發事件時發生錯誤: {write_error}")

        # --- 錯誤處理區塊 (重要修改在這裡) ---
        except (OSError, FileNotFoundError) as e: # 同時捕獲 OSError 和 FileNotFoundError
            # 判斷是否為裝置暫時不可用 (斷線或路徑不存在)
            if isinstance(e, FileNotFoundError) or (isinstance(e, OSError) and e.errno == 19):
                print(f"\n!!! 偵測到裝置暫時不可用 ({e}).")
                print("   程式將嘗試重新連接。請確保滑鼠已連接。")
                time.sleep(3) # 等待 3 秒後重試連接
                continue # 跳到 while True 迴圈的下一次迭代
            else:
                # 其他非預期的 OSError，例如權限問題 (但 PermissionError 會單獨捕獲)
                print(f"\n!!! 發生非預期的系統錯誤: {e}")
                traceback.print_exc()
                break # 其他 OSError 導致程式退出
        except PermissionError:
            print(f"!!! 錯誤: 沒有權限訪問 {PHYSICAL_MOUSE_PATH} 或 /dev/uinput。")
            print("請嘗試使用 sudo 運行此腳本，或檢查 udev 規則。")
            break # 權限錯誤，程式退出
        except KeyboardInterrupt:
            print("\n程式被 Ctrl+C 中斷.")
            break # Ctrl+C，程式退出
        except Exception as e:
            print("\n!!! 發生未預期的錯誤:")
            traceback.print_exc()
            break # 其他未預期錯誤，程式退出
        finally:
            print("--- 進入 finally 清理區塊 ---")
            if physical_mouse:
                try:
                    print("嘗試釋放實體滑鼠 (ungrab)...")
                    physical_mouse.ungrab()
                    print("實體滑鼠已 ungrab.")
                except OSError as e_ungrab:
                    if e_ungrab.errno == 19: print("實體滑鼠已斷開，無法 ungrab。")
                    else: print(f"Ungrab 實體滑鼠時出錯: {e_ungrab}")
                except Exception as e_ungrab_other: print(f"Ungrab 實體滑鼠時發生其他錯誤: {e_ungrab_other}")
                try:
                    print("嘗試關閉實體滑鼠 (close)...")
                    physical_mouse.close()
                    print("實體滑鼠已關閉.")
                except OSError as e_close_phys:
                    if e_close_phys.errno == 19: print("實體滑鼠已斷開，無法 close。")
                    else: print(f"關閉實體滑鼠時出錯: {e_close_phys}")
                except Exception as e_close_phys_other: print(f"關閉實體滑鼠時發生其他錯誤: {e_close_phys_other}")
            if ui:
                try:
                    print("嘗試關閉虛擬滑鼠...")
                    ui.close()
                    print("虛擬滑鼠已關閉.")
                except Exception as e_close_ui: print(f"關閉虛擬滑鼠時出錯: {e_close_ui}")
            print("--- 清理完成 ---")
    print("程式結束.")

if __name__ == "__main__":
    main()
