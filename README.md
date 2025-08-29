# Debounce-Software
Debounce-Software is a user-friendly debouncing tool dedicated to eliminating the frustration of hardware chatter issues for good. Say goodbye to accidental double-clicks and unintended key presses forever.

*  **At least for now, this is how the software works**: This tool works by grabbing the physical mouse's evdev events via python-evdev, filtering them based on the debouncing logic, and then forwarding the clean events to a new virtual mouse device created with the UInput module.




## Important Notice

⚠️ **This program requires administrator/superuser privileges to run.**

Root access is necessary to directly intercept and manage device events. On Linux, you will likely need to run it with `sudo`.


*  **About the Code** : The entire codebase is well-commented, so feel free to explore the source code or adapt it to your specific needs.

## Current project progress and status
### **Automatic Reconnection**

Event interception based on a specified mouse device path is now implemented.


*   **Example (from a Logitech G102 LIGHTSYNC):**
*   Note: Device IDs under `/dev/input/by-id/` can vary even between identical models. The path below serves as an example, and you must identify the correct one for your system.
    ```
    PHYSICAL_MOUSE_PATH = "/dev/input/by-id/usb-Logitech_G102_LIGHTSYNC_Gaming_Mouse_206E36594858-event-mouse"
    ```


### **Configurable Debounce Timeout (in milliseconds)**
The software now supports a flexible debounce configuration:

*   **Global Default:** A default timeout is applied globally to all buttons.
*   **Per-Button Override:** You can override the global setting by assigning a specific and different debounce time to each individual button.


## Operating System Compatibility
This tool is currently developed and tested exclusively for **Linux**.

**Note:** Due to current hardware and resource limitations, I am unable to test or develop for other platforms like Windows or macOS. Therefore, it is only guaranteed to work on Linux for the time being. Community contributions to support other operating systems are welcome.


## How to use
Basically, there are no other installation methods for this project. 



Just git clone it:
```git
git clone https://github.com/frostmnh/Debounce_Software/
```

Next, navigate into the project directory you just downloaded using the `cd` command:
```bash
cd Debounce_Software
```

**Linux**
- Next, run the script with administrator privileges using the following command: `sudo py debounce_software.py`
- P.S. By using sudo, you are telling the computer: "I, the user, authorize this program to run with the highest level of permissions."


## 文档
The documentation and project Wiki are currently a work in progress (WIP).

## Future Features/Roadmap/Planned Features
1.Add Default Configuration Profiles for Multiple Mice

2.Automatic Device Detection and User-Guided Selection

3.Enable Simultaneous Multi-Device Support

4.Add Support for Keyboard Debouncing

5.Implement an Interactive TUI for Configuration and Real-Time Monitoring(Users have the option to disable monitoring to save system resources.)

6.Add Internationalization (i18n) Support

7.Improve Configuration and Documentation for Ease of Use

8.Performance Optimization

9.Implement Auto-Start on Boot


## **FAQ (Frequently Asked Questions)**

**Q: Is it possible to run this without administrator privileges? I'm concerned about security.**

**A:** At the moment, this is not possible. The core functionality of intercepting and managing hardware events at a low level inherently requires administrator (root) permissions.

We understand that this permission model is similar to that of a keylogger, which can raise security concerns. However, we want to be very clear about how this program operates:

*   **This tool works by grabbing the physical mouse's `evdev` events via `python-evdev`, filtering them based on the debouncing logic, and then forwarding the clean events to a new virtual mouse device created with the `UInput` module.**

*   **Regarding the logging feature:** Strictly speaking, if you enable the diagnostic logging, anyone with access to the log files could potentially infer what was typed. **However, you can simply disable the logging feature entirely to eliminate this risk.**

We are actively exploring alternative implementation methods that would require fewer privileges or operate without administrator rights. Until such a solution is found, using administrator permissions is the only viable approach.


## License
This project is licensed under the terms of the MIT License.


## Maintainer
This project is currently maintained by:
*   [frostmnh](https://github.com/frostmnh)

Contributions are welcome!
