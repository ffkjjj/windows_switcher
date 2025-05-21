use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows::Win32::System::Diagnostics::ToolHelp::*;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, EnumWindows, GetClassNameW, GetForegroundWindow, GetMessageW, GetWindowRect,
    IsIconic, IsWindowVisible, SetForegroundWindow, TranslateMessage, MSG, WM_HOTKEY,
};

const HOTKEY_LEFT: i32 = 1;
const HOTKEY_RIGHT: i32 = 2;

fn main() {
    unsafe {
        let hwnd = HWND(0);
        if !RegisterHotKey(hwnd, HOTKEY_LEFT, MOD_ALT | MOD_SHIFT, 'H' as u32).is_ok() {
            eprintln!("Failed to register HOTKEY_LEFT");
        }
        if !RegisterHotKey(hwnd, HOTKEY_RIGHT, MOD_ALT | MOD_SHIFT, 'L' as u32).is_ok() {
            eprintln!("Failed to register HOTKEY_RIGHT");
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND(0), 0, 0).into() {
            if msg.message == WM_HOTKEY {
                match msg.wParam.0 as i32 {
                    HOTKEY_LEFT => switch_window(Direction::Left),
                    HOTKEY_RIGHT => switch_window(Direction::Right),
                    _ => {}
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // 程序退出时注销热键
        UnregisterHotKey(HWND(0), HOTKEY_LEFT);
        UnregisterHotKey(HWND(0), HOTKEY_RIGHT);
    }
    println!("退出程序");
}

#[derive(Copy, Clone)]
enum Direction {
    Left,
    Right,
}

unsafe fn switch_window(direction: Direction) {
    let current = GetForegroundWindow();
    if current.0 == 0 || IsIconic(current).as_bool() {
        return;
    }

    let mut current_rect = RECT::default();
    if !GetWindowRect(current, &mut current_rect).is_ok() {
        return;
    }

    let mut windows: Vec<(HWND, RECT)> = vec![];
    EnumWindows(
        Some(enum_windows_proc),
        LPARAM(&mut windows as *mut _ as isize),
    );

    windows.retain(|(hwnd, _)| unsafe { is_real_user_window(*hwnd) });

    windows.sort_by_key(|(_, rect)| rect.left);

    for i in 0..windows.len() {
        if windows[i].0 == current {
            match direction {
                Direction::Left if i > 0 => {
                    // println!("active window, {:?}", windows[i - 1]);
                    SetForegroundWindow(windows[i - 1].0);
                }
                Direction::Right if i + 1 < windows.len() => {
                    SetForegroundWindow(windows[i + 1].0);
                    // println!("active window, {:?}", windows[i + 1]);
                }
                _ => {}
            }
            break;
        }
    }
}

extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let windows = &mut *(lparam.0 as *mut Vec<(HWND, RECT)>);

        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return true.into();
        }

        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_ok()
            && (rect.right - rect.left) > 30
            && (rect.bottom - rect.top) > 30
        {
            windows.push((hwnd, rect));
        }
        true.into()
    }
}

pub unsafe fn is_real_user_window(hwnd: HWND) -> bool {
    // 必须是可见窗口
    if !IsWindowVisible(hwnd).as_bool() {
        return false;
    }

    // 不应最小化
    if IsIconic(hwnd).as_bool() {
        return false;
    }

    // 不应有所有者窗口（排除工具栏/浮动面板等）
    if GetWindow(hwnd, GW_OWNER).0 != 0 {
        return false;
    }

    // 不应是工具窗口（WS_EX_TOOLWINDOW） — 不会出现在 Alt+Tab
    let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
    if (ex_style & WS_EX_TOOLWINDOW.0 as i32) != 0 {
        return false;
    }

    // 应该不是系统窗口类（如 Progman, WorkerW, CoreWindow 等）
    let mut class_name = [0u16; 256];
    let len = GetClassNameW(hwnd, &mut class_name);
    let class = String::from_utf16_lossy(&class_name[..len as usize]);
    if matches!(
        class.as_str(),
        "Progman" | "WorkerW" | "Windows.UI.Core.CoreWindow"
    ) {
        return false;
    }

    // 需要有标题
    let mut title = [0u16; 256];
    let len = GetWindowTextW(hwnd, &mut title);
    let title = String::from_utf16_lossy(&title[..len as usize]);
    if title.trim().is_empty() {
        return false;
    }

    // 可选：排除特定系统进程（如 explorer 桌面、TextInputHost.exe）
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32));
    if let Some(process_name) = get_process_name(pid) {
        if matches!(
            process_name.to_lowercase().as_str(),
            "textinputhost.exe" | "applicationframehost.exe"
        ) {
            return false;
        }
    }

    true
}

// 获取进程名称辅助函数
unsafe fn get_process_name(pid: u32) -> Option<String> {
    let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
    let mut pe = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };

    if Process32FirstW(snapshot, &mut pe).is_ok() {
        loop {
            if pe.th32ProcessID == pid {
                let name = String::from_utf16_lossy(
                    &pe.szExeFile
                        .iter()
                        .take_while(|&&c| c != 0)
                        .cloned()
                        .collect::<Vec<_>>(),
                );
                return Some(name);
            }
            if !Process32NextW(snapshot, &mut pe).is_ok() {
                break;
            }
        }
    }
    None
}
