#!/usr/bin/env python
# -*- coding: utf-8 -*-

from evdev import InputDevice, categorize, ecodes, UInput
import time
import os
import traceback

# --- 設定 ---
# 1. 指定你的實體滑鼠裝置路徑
PHYSICAL_MOUSE_PATH = "/dev/input/by-id/usb-Logitech_G102_LIGHTSYNC_Gaming_Mouse_206E36594858-event-mouse" # 替換成你的路徑
# 2. 設定 **通用** 的去抖動超時時間 (秒)
#    如果需要為不同按鍵設定不同超時，邏輯會更複雜
DEBOUNCE_TIMEOUT = 0.135  # 50 毫秒=0.05

# --- Helper function to get button name ---
def get_button_name(code):
    common_buttons = {
        ecodes.BTN_LEFT: "BTN_LEFT", ecodes.BTN_RIGHT: "BTN_RIGHT",
        ecodes.BTN_MIDDLE: "BTN_MIDDLE", ecodes.BTN_SIDE: "BTN_SIDE",
        ecodes.BTN_EXTRA: "BTN_EXTRA", ecodes.BTN_FORWARD: "BTN_FORWARD",
        ecodes.BTN_BACK: "BTN_BACK", ecodes.BTN_TASK: "BTN_TASK",
    }
    # 嘗試從我們的映射中獲取名稱，否則返回原始代碼
    return common_buttons.get(code, f"Code {code}")

# --- 全局變數移除 (不再需要單一目標的狀態) ---
# last_press_time = 0
# button_is_pressed_virtually = False

def main():
    # *** 修改：使用字典來追蹤每個按鍵的狀態 ***
    # Key: button_code (e.g., ecodes.BTN_LEFT)
    # Value: {'last_press': timestamp, 'is_pressed': boolean}
    button_states = {}
    physical_mouse = None

    try:
        # 權限檢查... (不變)
        if os.geteuid() != 0:
            print("警告: 需要 root 權限...")
            # return

        # 開啟實體滑鼠裝置... (不變)
        print(f"嘗試開啟實體滑鼠: {PHYSICAL_MOUSE_PATH}")
        physical_mouse = InputDevice(PHYSICAL_MOUSE_PATH)
        print(f"成功開啟: {physical_mouse.name} ({physical_mouse.phys})")

        # 定義虛擬裝置的能力 (保持完整)
        capabilities = {
            ecodes.EV_KEY: [
                ecodes.BTN_LEFT, ecodes.BTN_RIGHT, ecodes.BTN_MIDDLE,
                ecodes.BTN_SIDE, ecodes.BTN_EXTRA,
                ecodes.BTN_FORWARD, ecodes.BTN_BACK, ecodes.BTN_TASK,
            ],
            ecodes.EV_REL: [
                ecodes.REL_X, ecodes.REL_Y,
                ecodes.REL_WHEEL, ecodes.REL_HWHEEL,
                # 不包含 11 (根據之前的決定)
            ],
            # ecodes.EV_MSC: [ecodes.MSC_SCAN], # 可以選擇是否加入
        }
        print(f"準備建立虛擬滑鼠，能力: {capabilities}")

        # *** 初始化 button_states 字典 ***
        # 為 capabilities 中定義的所有按鍵初始化狀態
        if ecodes.EV_KEY in capabilities:
            for btn_code in capabilities[ecodes.EV_KEY]:
                button_states[btn_code] = {'last_press': 0, 'is_pressed': False}
        print(f"將為以下按鍵應用去抖動邏輯: {[get_button_name(c) for c in button_states.keys()]}")

        # 使用 with UInput... (不變)
        with UInput(capabilities, name='VirtualDebouncedMouse', version=0x1) as ui:
            print("虛擬滑鼠 'VirtualDebouncedMouse' 已建立.")
            time.sleep(0.5)

            # 獨佔實體滑鼠... (不變)
            try:
                 print("嘗試獨佔實體滑鼠...")
                 physical_mouse.grab()
                 print("已獨佔實體滑鼠.")
            except IOError as e:
                 print(f"!!! 無法獨佔滑鼠: {e}")
                 return

            # --- 移除針對單一目標按鍵的打印語句 ---
            # target_button_name = get_button_name(TARGET_BUTTON)
            # print(f"開始監聽事件，去抖動按鍵 {target_button_name}...")
            print(f"開始監聽事件，應用通用去抖動 (超時: {DEBOUNCE_TIMEOUT*1000:.0f} ms)")
            print("按 Ctrl+C 停止.")

            # --- 事件迴圈 (修改去抖動邏輯) ---
            for event in physical_mouse.read_loop():
                # 過濾掉不關心的事件 (MSC_SCAN, REL code 11)
                if (event.type == ecodes.EV_MSC and event.code == ecodes.MSC_SCAN) or \
                   (event.type == ecodes.EV_REL and event.code == 11):
                    continue

                # --- 通用按鍵去抖動邏輯 ---
                # 檢查事件是否為我們關心的按鍵類型，並且該按鍵在我們的狀態字典中
                if event.type == ecodes.EV_KEY and event.code in button_states:
                    button_code = event.code
                    state = button_states[button_code] # 獲取該按鍵的當前狀態
                    button_name = get_button_name(button_code) # 獲取按鍵名稱用於打印
                    current_time = time.monotonic()

                    # print(f"(--- 處理按鍵 {button_name} ---)") # 可選調試

                    if event.value == 1:  # 按鍵按下
                        if (current_time - state['last_press']) > DEBOUNCE_TIMEOUT:
                            # 有效按下
                            print(f">>> 有效按下 {button_name} (傳遞)")
                            try:
                                ui.write(ecodes.EV_KEY, button_code, 1)
                                ui.syn()
                                # 更新 **這個按鍵** 的狀態
                                state['last_press'] = current_time
                                state['is_pressed'] = True
                            except Exception as write_error:
                                print(f"!!! 寫入按下 {button_name} 時錯誤: {write_error}")
                        else:
                            # 抖動按下，忽略
                            print(f"--- 抖動按下 {button_name} (忽略)")
                    elif event.value == 0:  # 按鍵釋放
                        # 只有當這個按鍵在虛擬裝置中確實被按下時，才傳遞釋放事件
                        if state['is_pressed']:
                            print(f"<<< 有效釋放 {button_name} (傳遞)")
                            try:
                                ui.write(ecodes.EV_KEY, button_code, 0)
                                ui.syn()
                                # 更新 **這個按鍵** 的狀態
                                state['is_pressed'] = False
                            except Exception as write_error:
                                print(f"!!! 寫入釋放 {button_name} 時錯誤: {write_error}")
                        # else: 如果虛擬按鈕沒按下，收到釋放事件通常可以忽略
                           # print(f"--- 原始釋放 {button_name} (虛擬未按下 - 忽略)")

                    # 處理完按鍵事件後，跳到下一個事件
                    continue

                # --- 其他事件處理 (SYN, REL, 不在 button_states 中的 KEY 等) ---
                elif event.type == ecodes.EV_SYN:
                    continue # SYN 事件由 ui.syn() 處理，直接跳過

                else: # 主要處理 REL 事件和其他未預期的事件
                    # print(f"--- 收到其他事件: ...") # 可移除調試信息
                    type_supported = event.type in capabilities
                    code_supported = False
                    if type_supported:
                        if event.type in [ecodes.EV_KEY, ecodes.EV_REL]:
                            code_supported = event.code in capabilities.get(event.type, [])
                            # print(f"      檢查 KEY/REL: ...") # 可移除調試信息
                        else: # 其他類型 (如果加入了 MSC 等)
                            code_supported = True
                            # print(f"      檢查其他類型: ...") # 可移除調試信息

                    if type_supported and code_supported:
                        # print(f"      >>> 嘗試轉發: ...") # 可移除調試信息
                        try:
                            ui.write(event.type, event.code, event.value)
                            ui.syn()
                        except Exception as write_error:
                            print(f"!!! 轉發事件時發生錯誤: {write_error}")
                    # else: # 忽略不在 capabilities 中的事件
                        # print(f"--- 忽略不支持的事件: ...") # 可移除調試信息

    # except 和 finally 區塊... (保持不變)
    except KeyboardInterrupt:
        print("\n程式被 Ctrl+C 中斷.")
    except FileNotFoundError:
        print(f"!!! 錯誤: 找不到裝置 {PHYSICAL_MOUSE_PATH}")
    except PermissionError:
        print(f"!!! 錯誤: 沒有權限訪問 {PHYSICAL_MOUSE_PATH} 或 /dev/uinput。")
    except Exception as e:
        print("\n!!! 發生未預期的錯誤:")
        traceback.print_exc()
    finally:
        if physical_mouse:
            try:
                print("嘗試釋放實體滑鼠...")
                physical_mouse.ungrab()
                physical_mouse.close()
                print("實體滑鼠已釋放並關閉.")
            except Exception as e_close:
                print(f"關閉或釋放滑鼠時出錯: {e_close}")

if __name__ == "__main__":
    main()
