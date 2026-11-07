//! The web palette, rendered directly with Win32/GDI (no embedded browser).
use super::*;
use std::sync::OnceLock;
const BG: u32 = 0x212121;
const SIDEBAR: u32 = 0x171717;
const CARD: u32 = 0x2f2f2f;
const TEXT: u32 = 0xececec;
const MUTED: u32 = 0xa09a8e;
const ACCENT: u32 = 0x7fa310;
#[repr(C)]
struct Paint {
    dc: H,
    erase: i32,
    rect: Rect,
    restore: i32,
    update: i32,
    reserved: [u8; 32],
}
#[repr(C)]
struct DrawItem {
    kind: u32,
    id: u32,
    item: u32,
    action: u32,
    state: u32,
    hwnd: H,
    dc: H,
    rect: Rect,
    data: usize,
}
#[repr(C)]
struct MinMax {
    reserved: Point,
    max: Point,
    pos: Point,
    min_track: Point,
    max_track: Point,
}
#[link(name = "gdi32")]
extern "system" {
    fn CreateSolidBrush(color: u32) -> H;
    fn SelectObject(dc: H, object: H) -> H;
    fn SetTextColor(dc: H, color: u32) -> u32;
    fn SetBkColor(dc: H, color: u32) -> u32;
    fn SetBkMode(dc: H, mode: i32) -> i32;
    fn RoundRect(dc: H, l: i32, t: i32, r: i32, b: i32, x: i32, y: i32) -> i32;
    fn CreateFontW(
        height: i32,
        width: i32,
        esc: i32,
        orientation: i32,
        weight: i32,
        italic: u32,
        underline: u32,
        strike: u32,
        charset: u32,
        out: u32,
        clip: u32,
        quality: u32,
        pitch: u32,
        face: *const u16,
    ) -> H;
}
#[link(name = "user32")]
extern "system" {
    fn FillRect(dc: H, rect: *const Rect, brush: H) -> i32;
    fn DrawTextW(dc: H, text: *const u16, len: i32, rect: *mut Rect, flags: u32) -> i32;
    fn BeginPaint(hwnd: H, paint: *mut Paint) -> H;
    fn EndPaint(hwnd: H, paint: *const Paint) -> i32;
    fn InvalidateRect(hwnd: H, rect: *const Rect, erase: i32) -> i32;
    fn GetParent(hwnd: H) -> H;
    fn GetDlgCtrlID(hwnd: H) -> i32;
    fn OpenClipboard(hwnd: H) -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(format: u32, data: H) -> H;
    fn CloseClipboard() -> i32;
}
#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(flags: u32, size: usize) -> H;
    fn GlobalLock(h: H) -> *mut u16;
    fn GlobalUnlock(h: H) -> i32;
    fn GlobalFree(h: H) -> H;
}
#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(name: *const u16) -> H;
    fn GetProcAddress(module: H, name: *const u8) -> *const u8;
}
pub fn brush(color: u32) -> H {
    static BRUSHES: OnceLock<HashMap<u32, H>> = OnceLock::new();
    *BRUSHES
        .get_or_init(|| {
            [BG, SIDEBAR, CARD, TEXT, MUTED, ACCENT, 0x3b3b3b]
                .into_iter()
                .map(|c| (c, unsafe { CreateSolidBrush(c) }))
                .collect()
        })
        .get(&color)
        .unwrap()
}
pub fn font(size: i32) -> H {
    static FONTS: OnceLock<HashMap<i32, H>> = OnceLock::new();
    *FONTS
        .get_or_init(|| {
            [13, 16, 22, 30]
                .into_iter()
                .map(|s| {
                    (s, unsafe {
                        CreateFontW(
                            -s,
                            0,
                            0,
                            0,
                            if s >= 22 { 600 } else { 400 },
                            0,
                            0,
                            0,
                            1,
                            0,
                            0,
                            5,
                            0,
                            wide("Segoe UI").as_ptr(),
                        )
                    })
                })
                .collect()
        })
        .get(&size)
        .unwrap()
}
pub fn background() -> H {
    brush(BG)
}
pub fn invalidate(hwnd: H) {
    unsafe {
        InvalidateRect(hwnd, null(), 0);
    }
}
pub fn dark_control(hwnd: H) {
    unsafe {
        let library = LoadLibraryW(wide("uxtheme.dll").as_ptr());
        let address = GetProcAddress(library, b"SetWindowTheme\0".as_ptr());
        if !address.is_null() {
            let call: unsafe extern "system" fn(H, *const u16, *const u16) -> i32 =
                std::mem::transmute(address);
            call(hwnd, wide("DarkMode_Explorer").as_ptr(), null());
        }
    }
}
pub fn dark_title(hwnd: H) {
    unsafe {
        let enabled = 1;
        let library = LoadLibraryW(wide("dwmapi.dll").as_ptr());
        let address = GetProcAddress(library, b"DwmSetWindowAttribute\0".as_ptr());
        if !address.is_null() {
            let call: unsafe extern "system" fn(H, u32, *const i32, u32) -> i32 =
                std::mem::transmute(address);
            call(hwnd, 20, &enabled, 4);
        }
    }
}
unsafe fn text(dc: H, s: &str, rect: &mut Rect, size: i32, color: u32, flags: u32) {
    SelectObject(dc, font(size));
    SetTextColor(dc, color);
    SetBkMode(dc, 1);
    DrawTextW(dc, wide(s).as_ptr(), -1, rect, flags | 0x800);
}
unsafe fn round(dc: H, r: &Rect, color: u32, radius: i32) {
    SelectObject(dc, brush(color));
    SelectObject(dc, GetStockObject(8));
    RoundRect(dc, r.left, r.top, r.right, r.bottom, radius, radius);
}
pub unsafe fn handle(app: &App, msg: u32, w: usize, l: isize) -> Option<isize> {
    match msg {
        0x24 => {
            let size = &mut *(l as *mut MinMax);
            size.min_track = Point { x: 940, y: 690 };
            Some(0)
        }
        0x14 => Some(1),
        0x0f => {
            let mut paint: Paint = std::mem::zeroed();
            let dc = BeginPaint(app.hwnd, &mut paint);
            let mut r: Rect = std::mem::zeroed();
            GetClientRect(app.hwnd, &mut r);
            FillRect(dc, &r, brush(BG));
            FillRect(
                dc,
                &Rect {
                    left: 0,
                    top: 0,
                    right: 254,
                    bottom: r.bottom,
                },
                brush(SIDEBAR),
            );
            if app.page == 0 {
                let width = (r.right - 322).min(790);
                let x = 266 + (r.right - 266 - width) / 2;
                round(
                    dc,
                    &Rect {
                        left: x,
                        top: r.bottom - 190,
                        right: x + width,
                        bottom: r.bottom - 50,
                    },
                    CARD,
                    28,
                );
            }
            EndPaint(app.hwnd, &paint);
            Some(0)
        }
        0x133 | 0x134 | 0x135 | 0x138 => {
            let id = GetDlgCtrlID(l);
            let color = if [101, 901, 902].contains(&id) {
                SIDEBAR
            } else if [103, 301, 501, 502, 503].contains(&id) {
                CARD
            } else {
                BG
            };
            SetTextColor(
                w as H,
                if [900, 902].contains(&id) {
                    MUTED
                } else {
                    TEXT
                },
            );
            SetBkColor(w as H, color);
            Some(brush(color))
        }
        0x2b => {
            let item = &*(l as *const DrawItem);
            let id = item.id as i32;
            let mut r = Rect {
                left: item.rect.left + 10,
                top: item.rect.top,
                right: item.rect.right - 10,
                bottom: item.rect.bottom,
            };
            if item.kind == 2 {
                FillRect(item.dc, &item.rect, brush(SIDEBAR));
                if item.item != u32::MAX {
                    if item.state & 1 != 0 {
                        round(item.dc, &item.rect, CARD, 12);
                    }
                    let title = app.state["chats"][item.item as usize]["title"]
                        .as_str()
                        .unwrap_or("Conversation");
                    text(item.dc, title, &mut r, 16, TEXT, 0x8024);
                }
            } else {
                let selected = (10..15).contains(&id) && id - 10 == app.page;
                let accent = [105, 106, 302, 401, 504].contains(&id);
                let base = if (10..15).contains(&id) { SIDEBAR } else { BG };
                FillRect(item.dc, &item.rect, brush(base));
                let color = if item.state & 1 != 0 {
                    0x3b3b3b
                } else if accent {
                    ACCENT
                } else if selected {
                    CARD
                } else {
                    base
                };
                round(item.dc, &item.rect, color, 14);
                text(
                    item.dc,
                    &app.text(id),
                    &mut r,
                    16,
                    if item.state & 4 != 0 { MUTED } else { TEXT },
                    0x8025,
                );
            }
            Some(1)
        }
        _ => None,
    }
}
pub fn copy(hwnd: H, value: &str) -> Result<()> {
    unsafe {
        if OpenClipboard(hwnd) == 0 {
            return Err("Klembord is in gebruik".into());
        }
        let data = wide(value);
        let memory = GlobalAlloc(2, data.len() * 2);
        if memory == 0 {
            CloseClipboard();
            return Err("Clipboard memory is missing".into());
        }
        let target = GlobalLock(memory);
        if target.is_null() {
            GlobalFree(memory);
            CloseClipboard();
            return Err("Failed to open clipboard".into());
        }
        std::ptr::copy_nonoverlapping(data.as_ptr(), target, data.len());
        GlobalUnlock(memory);
        EmptyClipboard();
        if SetClipboardData(13, memory) == 0 {
            GlobalFree(memory);
            CloseClipboard();
            return Err("Copy failed".into());
        }
        CloseClipboard();
        Ok(())
    }
}
pub fn register(instance: H) {
    unsafe {
        let name = wide("AIUSBChatView");
        let cls = WndClass {
            style: 3,
            proc: Some(chat_proc),
            cls: 0,
            win: 0,
            instance,
            icon: 0,
            cursor: LoadCursorW(0, 32512 as *const u16),
            background: brush(BG),
            menu: null(),
            name: name.as_ptr(),
        };
        RegisterClassW(&cls);
    }
}
unsafe extern "system" fn chat_proc(hwnd: H, msg: u32, w: usize, l: isize) -> isize {
    let parent = GetParent(hwnd);
    let p = GetWindowLongPtrW(parent, -21) as *mut App;
    if p.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let app = &*p;
    if msg == 0x20a {
        let delta = ((w >> 16) as u16 as i16) as i32;
        app.scroll.set((app.scroll.get() - delta).max(0));
        invalidate(hwnd);
        return 0;
    }
    if msg == 0x100 {
        let delta = match w {
            0x26 => -60,
            0x28 => 60,
            0x21 => -400,
            0x22 => 400,
            0x24 => -i32::MAX,
            0x23 => i32::MAX,
            _ => 0,
        };
        app.scroll
            .set(app.scroll.get().saturating_add(delta).max(0));
        invalidate(hwnd);
        return 0;
    }
    if msg == 0x14 {
        return 1;
    }
    if msg == 0x0f {
        let mut paint: Paint = std::mem::zeroed();
        let dc = BeginPaint(hwnd, &mut paint);
        let mut r: Rect = std::mem::zeroed();
        GetClientRect(hwnd, &mut r);
        FillRect(dc, &r, brush(BG));
        let messages = app.state["chats"]
            .as_array()
            .and_then(|a| a.iter().find(|c| c["id"].as_str() == app.active.as_deref()))
            .and_then(|c| c["messages"].as_array());
        if messages.is_none_or(|a| a.is_empty()) {
            let y = (r.bottom / 2 - 90).max(30);
            text(
                dc,
                "Waar kan ik je mee helpen?",
                &mut Rect {
                    left: 20,
                    top: y,
                    right: r.right - 20,
                    bottom: y + 55,
                },
                30,
                TEXT,
                0x25,
            );
            text(
                dc,
                "Je eigen AI, lokaal op je computer.",
                &mut Rect {
                    left: 20,
                    top: y + 65,
                    right: r.right - 20,
                    bottom: y + 100,
                },
                16,
                MUTED,
                0x25,
            );
            text(
                dc,
                "Choose a model and enter your question below.",
                &mut Rect {
                    left: 20,
                    top: y + 110,
                    right: r.right - 20,
                    bottom: y + 150,
                },
                16,
                MUTED,
                0x25,
            );
        } else {
            let messages = messages.unwrap();
            let width = (r.right - 52).min(720).max(100);
            let left = (r.right - width) / 2;
            let mut heights = Vec::new();
            let mut total = 24;
            for m in messages {
                let user = m["role"] == "user";
                let tw = if user { width * 4 / 5 - 36 } else { width - 52 };
                let mut measure = Rect {
                    left: 0,
                    top: 0,
                    right: tw,
                    bottom: 0,
                };
                text(
                    dc,
                    m["content"].as_str().unwrap_or(""),
                    &mut measure,
                    16,
                    TEXT,
                    0x410,
                );
                let height = measure.bottom.max(24) + if user { 34 } else { 42 };
                heights.push(height);
                total += height + 24;
            }
            app.total_height.set(total);
            let scroll = app.scroll.get().clamp(0, (total - r.bottom).max(0));
            app.scroll.set(scroll);
            let mut y = 24 - scroll;
            for (m, height) in messages.iter().zip(heights) {
                let user = m["role"] == "user";
                if y + height >= 0 && y < r.bottom {
                    let x = if user { left + width / 5 } else { left + 44 };
                    if user {
                        round(
                            dc,
                            &Rect {
                                left: x - 18,
                                top: y,
                                right: left + width,
                                bottom: y + height,
                            },
                            CARD,
                            24,
                        );
                    } else {
                        round(
                            dc,
                            &Rect {
                                left,
                                top: y + 2,
                                right: left + 30,
                                bottom: y + 32,
                            },
                            ACCENT,
                            30,
                        );
                        text(
                            dc,
                            "AI",
                            &mut Rect {
                                left,
                                top: y + 2,
                                right: left + 30,
                                bottom: y + 32,
                            },
                            13,
                            TEXT,
                            0x25,
                        );
                        text(
                            dc,
                            "AI USB",
                            &mut Rect {
                                left: x,
                                top: y,
                                right: left + width,
                                bottom: y + 22,
                            },
                            13,
                            MUTED,
                            0x20,
                        );
                    }
                    text(
                        dc,
                        m["content"].as_str().unwrap_or(""),
                        &mut Rect {
                            left: x,
                            top: y + if user { 16 } else { 28 },
                            right: left + width - 18,
                            bottom: y + height,
                        },
                        16,
                        TEXT,
                        0x10,
                    );
                }
                y += height + 24;
            }
            if total > r.bottom {
                let thumb = (r.bottom * r.bottom / total).max(24);
                let top = scroll * (r.bottom - thumb) / (total - r.bottom);
                round(
                    dc,
                    &Rect {
                        left: r.right - 5,
                        top,
                        right: r.right - 1,
                        bottom: top + thumb,
                    },
                    CARD,
                    4,
                );
            }
        }
        EndPaint(hwnd, &paint);
        return 0;
    }
    DefWindowProcW(hwnd, msg, w, l)
}
