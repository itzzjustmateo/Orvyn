//! Native Win32 controls. No Python, browser engine or JavaScript in this window.
mod theme;
use crate::core::{self, err, wide, Result};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    ptr::{null, null_mut},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};
type H = isize;
#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
struct Msg {
    hwnd: H,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    point: Point,
    private: u32,
}
#[repr(C)]
struct WndClass {
    style: u32,
    proc: Option<unsafe extern "system" fn(H, u32, usize, isize) -> isize>,
    cls: i32,
    win: i32,
    instance: H,
    icon: H,
    cursor: H,
    background: H,
    menu: *const u16,
    name: *const u16,
}
#[repr(C)]
struct OpenFile {
    size: u32,
    owner: H,
    instance: H,
    filter: *const u16,
    custom: *mut u16,
    max_custom: u32,
    filter_index: u32,
    file: *mut u16,
    max_file: u32,
    file_title: *mut u16,
    max_title: u32,
    dir: *const u16,
    title: *const u16,
    flags: u32,
    offset: u16,
    extension: u16,
    default_extension: *const u16,
    data: isize,
    hook: usize,
    template: *const u16,
    reserved: *mut u8,
    reserved2: u32,
    flags_ex: u32,
}
#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(class: *const WndClass) -> u16;
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        text: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: H,
        menu: H,
        instance: H,
        param: *mut u8,
    ) -> H;
    fn DefWindowProcW(hwnd: H, msg: u32, w: usize, l: isize) -> isize;
    fn ShowWindow(hwnd: H, show: i32) -> i32;
    fn GetMessageW(msg: *mut Msg, hwnd: H, min: u32, max: u32) -> i32;
    fn TranslateMessage(msg: *const Msg) -> i32;
    fn DispatchMessageW(msg: *const Msg) -> isize;
    fn IsDialogMessageW(hwnd: H, msg: *mut Msg) -> i32;
    fn PostQuitMessage(code: i32);
    fn DestroyWindow(hwnd: H) -> i32;
    fn SendMessageW(hwnd: H, msg: u32, w: usize, l: isize) -> isize;
    fn SetWindowTextW(hwnd: H, text: *const u16) -> i32;
    fn GetWindowTextLengthW(hwnd: H) -> i32;
    fn GetWindowTextW(hwnd: H, text: *mut u16, max: i32) -> i32;
    fn SetWindowLongPtrW(hwnd: H, index: i32, value: isize) -> isize;
    fn GetWindowLongPtrW(hwnd: H, index: i32) -> isize;
    fn MoveWindow(hwnd: H, x: i32, y: i32, w: i32, h: i32, repaint: i32) -> i32;
    fn GetClientRect(hwnd: H, rect: *mut Rect) -> i32;
    fn SetTimer(hwnd: H, id: usize, interval: u32, callback: usize) -> usize;
    fn KillTimer(hwnd: H, id: usize) -> i32;
    fn EnableWindow(hwnd: H, enable: i32) -> i32;
    fn MessageBoxW(hwnd: H, text: *const u16, title: *const u16, kind: u32) -> i32;
    fn LoadCursorW(instance: H, name: *const u16) -> H;
    fn SetFocus(hwnd: H) -> H;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> H;
}
#[link(name = "gdi32")]
extern "system" {
    fn GetStockObject(index: i32) -> H;
}
#[link(name = "comdlg32")]
extern "system" {
    fn GetOpenFileNameW(file: *mut OpenFile) -> i32;
}
#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: H,
        op: *const u16,
        file: *const u16,
        args: *const u16,
        dir: *const u16,
        show: i32,
    ) -> H;
}

pub fn error(text: &str) {
    unsafe {
        MessageBoxW(0, wide(text).as_ptr(), wide("AI USB").as_ptr(), 0x10);
    }
}
fn confirm(hwnd: H, text: &str) -> bool {
    unsafe { MessageBoxW(hwnd, wide(text).as_ptr(), wide("AI USB").as_ptr(), 0x24) == 6 }
}
fn open(path: &str) {
    unsafe {
        ShellExecuteW(
            0,
            wide("open").as_ptr(),
            wide(path).as_ptr(),
            null(),
            null(),
            1,
        );
    }
}
fn file_dialog(hwnd: H, filter: &str) -> Option<PathBuf> {
    unsafe {
        let mut buffer = vec![0u16; 32768];
        let filter = wide(filter);
        let mut dialog: OpenFile = std::mem::zeroed();
        dialog.size = std::mem::size_of::<OpenFile>() as u32;
        dialog.owner = hwnd;
        dialog.filter = filter.as_ptr();
        dialog.file = buffer.as_mut_ptr();
        dialog.max_file = buffer.len() as u32;
        dialog.flags = 0x1000 | 0x800 | 8;
        if GetOpenFileNameW(&mut dialog) == 0 {
            None
        } else {
            Some(PathBuf::from(String::from_utf16_lossy(
                &buffer[..buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len())],
            )))
        }
    }
}
enum Event {
    Answer(String, Result<String>),
    Done(Result<String>),
    Progress(String),
}
struct App {
    hwnd: H,
    root: PathBuf,
    cfg: Value,
    state: Value,
    active: Option<String>,
    controls: HashMap<i32, H>,
    groups: HashMap<i32, i32>,
    page: i32,
    busy: bool,
    rx: Receiver<Event>,
    tx: Sender<Event>,
    cancel: Arc<AtomicBool>,
    attachment: Option<PathBuf>,
    config_text: String,
    scroll: std::cell::Cell<i32>,
    total_height: std::cell::Cell<i32>,
}
impl App {
    fn control(&mut self, id: i32, page: i32, class: &str, text: &str, extra: u32) {
        unsafe {
            let extra = if class == "BUTTON" {
                extra | 0xB
            } else if class == "LISTBOX" {
                (extra & !0x00800000) | 0x10 | 0x40 | 0x100
            } else {
                extra & !0x00800000
            };
            let h = CreateWindowExW(
                0,
                wide(class).as_ptr(),
                wide(text).as_ptr(),
                0x50010000 | extra,
                0,
                0,
                100,
                30,
                self.hwnd,
                id as H,
                GetModuleHandleW(null()),
                null_mut(),
            );
            SendMessageW(h, 0x30, theme::font(16) as usize, 1);
            theme::dark_control(h);
            if class == "LISTBOX" {
                SendMessageW(h, 0x1A0, 0, 40);
            }
            if class == "EDIT" {
                SendMessageW(h, 0xC5, 40_000_000, 0);
            }
            self.controls.insert(id, h);
            self.groups.insert(id, page);
        }
    }
    fn text(&self, id: i32) -> String {
        unsafe {
            let h = self.controls[&id];
            let n = GetWindowTextLengthW(h);
            let mut out = vec![0; n as usize + 1];
            GetWindowTextW(h, out.as_mut_ptr(), out.len() as i32);
            String::from_utf16_lossy(&out[..n as usize])
        }
    }
    fn set(&self, id: i32, text: &str) {
        unsafe {
            SetWindowTextW(
                self.controls[&id],
                wide(&text.replace('\n', "\r\n").replace("\r\r\n", "\r\n")).as_ptr(),
            );
        }
    }
    fn status(&self, text: &str) {
        self.set(900, text);
    }
    fn combo(&self, id: i32, values: &[&str]) {
        unsafe {
            for value in values {
                SendMessageW(self.controls[&id], 0x143, 0, wide(value).as_ptr() as isize);
            }
            SendMessageW(self.controls[&id], 0x14E, 0, 0);
        }
    }
    fn index(&self, id: i32) -> usize {
        unsafe { SendMessageW(self.controls[&id], 0x147, 0, 0).max(0) as usize }
    }
    fn model(&self) -> &str {
        if self.index(104) == 1 {
            "glm-ocr"
        } else {
            "qwen3"
        }
    }
    fn chosen_service(&self) -> String {
        self.cfg["services"]
            .as_object()
            .unwrap()
            .keys()
            .nth(self.index(202))
            .cloned()
            .unwrap_or_default()
    }
    fn place(&self, id: i32, x: i32, y: i32, w: i32, h: i32) {
        unsafe {
            MoveWindow(self.controls[&id], x, y, w.max(20), h.max(20), 1);
        }
    }
    fn layout(&self) {
        unsafe {
            let mut r: Rect = std::mem::zeroed();
            GetClientRect(self.hwnd, &mut r);
            let w = r.right.max(900);
            let h = r.bottom.max(620);
            for i in 0..5 {
                self.place(10 + i, 14, h - 232 + i * 40, 226, 36);
            }
            self.place(901, 24, 24, 210, 36);
            self.place(902, 24, 126, 210, 25);
            self.place(903, 282, 22, w - 312, 38);
            self.place(900, 282, h - 36, w - 306, 28);
            for (id, page) in &self.groups {
                ShowWindow(
                    self.controls[id],
                    if *page < 0 || *page == self.page {
                        5
                    } else {
                        0
                    },
                );
            }
            match self.page {
                0 => {
                    self.place(101, 14, 157, 226, h - 404);
                    self.place(106, 14, 76, 226, 40);
                    self.place(107, w - 145, 22, 122, 34);
                    self.place(104, 282, 67, 205, 250);
                    self.place(109, 499, 67, 132, 34);
                    self.place(110, w - 145, 67, 122, 34);
                    self.place(102, 266, 114, w - 278, h - 312);
                    let composer = (w - 322).min(790);
                    let x = 266 + (w - 266 - composer) / 2;
                    self.place(103, x + 18, h - 174, composer - 36, 68);
                    self.place(108, x + 10, h - 96, 136, 34);
                    self.place(111, x + 156, h - 96, 150, 34);
                    self.place(105, x + composer - 125, h - 96, 112, 34);
                }
                1 => {
                    self.place(202, 282, 80, 210, 250);
                    self.place(203, 508, 80, 105, 36);
                    self.place(204, 625, 80, 105, 36);
                    self.place(205, 282, 130, 170, 36);
                    self.place(206, 464, 130, 150, 36);
                    self.place(207, 626, 130, 155, 36);
                    self.place(201, 282, 182, w - 312, 120);
                    self.place(214, 282, 316, 190, 36);
                    self.place(215, 282, 365, w - 312, h - 419);
                }
                2 => {
                    self.place(301, 282, 80, w - 312, h - 202);
                    self.place(302, 282, h - 102, 205, 40);
                    self.place(303, 501, h - 102, 190, 40);
                }
                3 => {
                    self.place(400, 300, 90, w - 350, 150);
                    self.place(401, 300, 260, 260, 44);
                    self.place(402, 300, 320, 260, 44);
                    self.place(403, 300, 380, 260, 44);
                }
                _ => {
                    for (label, input, y) in [(510, 501, 90), (511, 502, 175), (512, 503, 260)] {
                        self.place(label, 300, y, w - 350, 25);
                        self.place(input, 300, y + 30, w - 350, 36);
                    }
                    self.place(504, 300, 350, 220, 40);
                    self.place(505, 532, 350, 110, 40);
                    self.place(506, 654, 350, 170, 40);
                    self.place(513, 300, 420, w - 350, 120);
                }
            }
            self.set(
                903,
                [
                    "Chat",
                    "Services and logs",
                    "Configuration",
                    "Backup and restore",
                    "Download models",
                ][self.page as usize],
            );
            theme::invalidate(self.hwnd);
        }
    }
    fn refresh_chats(&mut self) {
        unsafe {
            SendMessageW(self.controls[&101], 0x184, 0, 0);
            if let Some(chats) = self.state["chats"].as_array() {
                for (i, chat) in chats.iter().enumerate() {
                    SendMessageW(
                        self.controls[&101],
                        0x180,
                        0,
                        wide(chat["title"].as_str().unwrap_or("Conversation")).as_ptr() as isize,
                    );
                    if self.active.as_deref() == chat["id"].as_str() {
                        SendMessageW(self.controls[&101], 0x186, i, 0);
                    }
                }
            }
            self.render();
        }
    }
    fn render(&self) {
        let mut text = String::new();
        if let Some(chat) = self.state["chats"].as_array().and_then(|a| {
            a.iter()
                .find(|c| c["id"].as_str() == self.active.as_deref())
        }) {
            if let Some(messages) = chat["messages"].as_array() {
                for m in messages {
                    text.push_str(if m["role"] == "user" {
                        "Jij\r\n"
                    } else {
                        "AI\r\n"
                    });
                    text.push_str(m["content"].as_str().unwrap_or(""));
                    text.push_str("\r\n\r\n");
                }
            }
        } else {
            text="Welcome to AI USB\r\n\r\nThis is the native Rust chat. Start the selected model and ask your question.\r\nFor GLM-OCR: attach an image and request text recognition.".into();
        }
        self.set(102, &text);
        self.scroll.set(i32::MAX);
        theme::invalidate(self.controls[&102]);
    }
    fn begin(&mut self) -> bool {
        if self.busy {
            self.status("Wait for the current action to finish, or pause the download.");
            return false;
        }
        self.busy = true;
        self.cancel.store(false, Ordering::Relaxed);
        self.status("Working...");
        unsafe {
            EnableWindow(self.controls[&105], 0);
        }
        true
    }
    fn work(&mut self, f: impl FnOnce() -> Result<String> + Send + 'static) {
        if !self.begin() {
            return;
        }
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Event::Done(f()));
        });
    }
    fn reload(&mut self) -> Result<()> {
        self.cfg = core::config(&self.root)?;
        self.state = core::load_chats(&self.cfg)?;
        self.active = self.state["chats"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v["id"].as_str())
            .map(String::from);
        self.refresh_chats();
        Ok(())
    }
    fn send(&mut self) -> Result<()> {
        if self.busy {
            return Err("An action is already running".into());
        }
        self.cfg = core::config(&self.root)?;
        let key = self.model().to_string();
        let input = self.text(103).trim().to_string();
        if input.is_empty() {
            return Ok(());
        }
        if !core::online(&self.cfg, &key) {
            return Err("Model is offline or still loading. Click Start model and try again.".into());
        }
        if key == "glm-ocr" && self.attachment.is_none() {
            return Err("GLM-OCR requires an image. Click Image.".into());
        }
        let mut next = self.state.clone();
        let active = self.active.clone().unwrap_or_else(core::id);
        let chats = next["chats"].as_array_mut().ok_or("Invalid chat storage")?;
        if !chats.iter().any(|c| c["id"] == active) {
            chats.insert(0,json!({"id":active,"title":input.chars().take(50).collect::<String>(),"model":key,"createdAt":SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,"messages":[]}));
        }
        let chat = chats.iter_mut().find(|c| c["id"] == active).unwrap();
        chat["messages"]
            .as_array_mut()
            .ok_or("Invalid messages")?
            .push(json!({"id":core::id(),"role":"user","content":input}));
        let mut messages = vec![];
        if key == "qwen3" {
            messages.push(json!({"role":"system","content":"You are a helpful assistant. Respond in English by default, without emojis."}));
            for m in chat["messages"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|m| m["isError"] != true)
            {
                messages.push(json!({"role":m["role"],"content":m["content"]}));
            }
        } else {
            use base64::Engine;
            let file = self.attachment.as_ref().unwrap();
            if std::fs::metadata(file).map_err(err)?.len() > 20_000_000 {
                return Err("Images must be no larger than 20 MB".into());
            }
            let bytes = std::fs::read(file).map_err(err)?;
            let mime = if bytes.starts_with(b"\x89PNG") {
                "image/png"
            } else if bytes.starts_with(b"\xff\xd8") {
                "image/jpeg"
            } else {
                return Err("Choose a PNG or JPEG image".into());
            };
            messages.push(json!({"role":"user","content":[{"type":"text","text":"Text Recognition:"},{"type":"image_url","image_url":{"url":format!("data:{mime};base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes))}}]}));
        }
        core::save_chats(&self.cfg, &mut next)?;
        self.state = next;
        self.active = Some(active.clone());
        self.refresh_chats();
        self.set(103, "");
        self.begin();
        let cfg = self.cfg.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let answer = core::chat(&cfg, &key, Value::Array(messages));
            let _ = tx.send(Event::Answer(active, answer));
        });
        Ok(())
    }
    fn service_status(&self) {
        let mut text = String::new();
        for (key, svc) in self.cfg["services"].as_object().unwrap() {
            text.push_str(&format!(
                "{}   poort {}   {}\r\n",
                svc["name"].as_str().unwrap_or(key),
                svc["port"],
                if core::online(&self.cfg, key) {
                    "Bereikbaar"
                } else {
                    "Offline / loading"
                }
            ));
        }
        self.set(201, &text);
    }
    fn command(&mut self, id: i32, notification: usize) -> Result<()> {
        if (10..15).contains(&id) {
            self.page = id - 10;
            self.layout();
            if self.page == 1 {
                self.cfg = core::config(&self.root)?;
                self.service_status();
            }
            return Ok(());
        }
        if id == 101 && notification != 1 {
            return Ok(());
        }
        if id != 101
            && (!matches!(id,105..=111|203..=207|214|302|303|401..=403|504..=506)
                || notification != 0)
        {
            return Ok(());
        }
        if self.busy && ![505, 101].contains(&id) {
            return Err("Wait for the current action to finish".into());
        }
        match id {
            101 if notification == 1 => unsafe {
                let i = SendMessageW(self.controls[&101], 0x188, 0, 0);
                if i >= 0 {
                    self.active = self.state["chats"][i as usize]["id"]
                        .as_str()
                        .map(String::from);
                    self.render();
                }
            },
            105 => self.send()?,
            111 => {
                if let Some(answer) = self.state["chats"]
                    .as_array()
                    .and_then(|a| {
                        a.iter()
                            .find(|c| c["id"].as_str() == self.active.as_deref())
                    })
                    .and_then(|c| c["messages"].as_array())
                    .and_then(|a| a.iter().rev().find(|m| m["role"] == "assistant"))
                    .and_then(|m| m["content"].as_str())
                {
                    theme::copy(self.hwnd, answer)?;
                    self.status("Response copied");
                }
            }
            106 => {
                self.active = None;
                self.attachment = None;
                self.set(103, "");
                self.render();
                self.status("New conversation");
            }
            107 => {
                self.reload()?;
                self.status("Reload conversations");
            }
            108 => {
                self.attachment = file_dialog(self.hwnd, "Images\0*.png;*.jpg;*.jpeg\0\0");
                self.status(&format!(
                    "Image: {}",
                    self.attachment
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or("none".into())
                ));
            }
            109 => {
                let root = self.root.clone();
                let key = self.model().to_string();
                self.work(move || core::service(&root, &key, true));
            }
            110 => {
                if let Some(active) = &self.active {
                    if confirm(self.hwnd, "Delete this conversation?") {
                        let mut next = self.state.clone();
                        next["chats"]
                            .as_array_mut()
                            .unwrap()
                            .retain(|c| c["id"] != *active);
                        core::save_chats(&self.cfg, &mut next)?;
                        self.state = next;
                        self.active = None;
                        self.refresh_chats();
                    }
                }
            }
            203 | 204 => {
                let root = self.root.clone();
                let key = self.chosen_service();
                self.work(move || core::service(&root, &key, id == 203));
            }
            205 | 206 => {
                let root = self.root.clone();
                self.work(move || {
                    let mut out = vec![];
                    for key in ["qwen3", "glm-ocr"] {
                        out.push(core::service(&root, key, id == 205)?);
                    }
                    Ok(out.join("; "))
                });
            }
            207 => {
                self.cfg = core::config(&self.root)?;
                self.service_status();
            }
            214 => {
                let file =
                    core::path(&self.cfg, "logs").join(format!("{}.log", self.chosen_service()));
                use std::io::{Read, Seek, SeekFrom};
                let mut f = std::fs::File::open(file).map_err(err)?;
                let len = f.metadata().map_err(err)?.len();
                f.seek(SeekFrom::Start(len.saturating_sub(100_000)))
                    .map_err(err)?;
                let mut bytes = Vec::new();
                f.read_to_end(&mut bytes).map_err(err)?;
                self.set(215, &String::from_utf8_lossy(&bytes));
            }
            302 => {
                let current =
                    std::fs::read_to_string(self.root.join("config/settings.json")).map_err(err)?;
                if current != self.config_text {
                    return Err("Configuration changed elsewhere. Click Reload.".into());
                }
                let text = self.text(301);
                let cfg: Value = serde_json::from_str(&text).map_err(err)?;
                core::validate(&cfg)?;
                core::backup(&self.cfg)?;
                core::atomic(&self.root.join("config/settings.json"), &cfg)?;
                self.cfg = core::config(&self.root)?;
                self.config_text =
                    std::fs::read_to_string(self.root.join("config/settings.json")).map_err(err)?;
                self.set(301, &self.config_text);
                self.status("Saved. Restart the affected services.");
            }
            303 => {
                self.config_text =
                    std::fs::read_to_string(self.root.join("config/settings.json")).map_err(err)?;
                self.set(301, &self.config_text);
            }
            401 => {
                let cfg = core::config(&self.root)?;
                self.work(move || core::backup(&cfg).map(|p| format!("Back-up: {}", p.display())));
            }
            402 => {
                if let Some(file) = file_dialog(self.hwnd, "AI USB back-up\0*.zip\0\0") {
                    if confirm(self.hwnd,"Restore conversations and settings? Close browser chats first. Current data will be backed up."){let cfg=core::config(&self.root)?;let root=self.root.clone();self.work(move||core::restore(&cfg,&root,&file));}
                }
            }
            403 => {
                let dir = core::path(&self.cfg, "backups");
                std::fs::create_dir_all(&dir).map_err(err)?;
                open(&dir.to_string_lossy());
            }
            504 => {
                let cfg = core::config(&self.root)?;
                let url = self.text(501).trim().to_string();
                let name = self.text(502).trim().to_string();
                let hash = self.text(503).trim().to_string();
                let cancel = self.cancel.clone();
                let tx = self.tx.clone();
                self.work(move || {
                    core::download(&cfg, &url, &name, &hash, cancel, |s| {
                        let _ = tx.send(Event::Progress(s));
                    })
                });
            }
            505 => {
                self.cancel.store(true, Ordering::Relaxed);
            }
            506 => open(&core::path(&self.cfg, "models").to_string_lossy()),
            _ => {}
        }
        Ok(())
    }
    fn poll(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Progress(text) => self.status(&text),
                Event::Done(result) => {
                    self.finish();
                    match result {
                        Ok(text) => self.status(&text),
                        Err(e) => {
                            self.status(&e);
                            error(&e);
                        }
                    }
                }
                Event::Answer(chat_id, result) => {
                    self.finish();
                    let failed = result.is_err();
                    let content = match result {
                        Ok(s) => s,
                        Err(e) => format!("Error: {e}"),
                    };
                    let mut next = self.state.clone();
                    if let Some(chat) = next["chats"]
                        .as_array_mut()
                        .and_then(|a| a.iter_mut().find(|c| c["id"] == chat_id))
                    {
                        chat["messages"].as_array_mut().unwrap().push(json!({"id":core::id(),"role":"assistant","content":content,"isError":failed}));
                    }
                    match core::save_chats(&self.cfg, &mut next) {
                        Ok(()) => {
                            self.state = next;
                            self.render();
                            self.status(if failed {
                                "Model request failed; see conversation"
                            } else {
                                "Response saved"
                            });
                        }
                        Err(e) => {
                            // Preserve an answer if another client changed the shared history.
                            let recovery = core::path(&self.cfg, "data")
                                .join(format!("recovery-{}.json", core::id()));
                            let _ = core::atomic(&recovery, &next);
                            self.state = next;
                            self.render();
                            self.status(&format!(
                                "{e} Antwoord ook bewaard in {}",
                                recovery.display()
                            ));
                        }
                    }
                }
            }
        }
    }
    fn finish(&mut self) {
        self.busy = false;
        unsafe {
            EnableWindow(self.controls[&105], 1);
        }
    }
}
unsafe extern "system" fn proc(hwnd: H, msg: u32, w: usize, l: isize) -> isize {
    let ptr = GetWindowLongPtrW(hwnd, -21) as *mut App;
    if !ptr.is_null() {
        let app = &mut *ptr;
        if let Some(result) = theme::handle(app, msg, w, l) {
            return result;
        }
        match msg {
            0x111 => {
                if let Err(e) = app.command((w & 0xffff) as i32, w >> 16) {
                    app.status(&e);
                    error(&e);
                }
                return 0;
            }
            5 => {
                app.layout();
                return 0;
            }
            0x113 => {
                if w == 2 {
                    if app.page < 4 {
                        app.page += 1;
                        app.layout();
                    } else {
                        DestroyWindow(hwnd);
                    }
                } else {
                    app.poll();
                }
                return 0;
            }
            0x10 => {
                if app.busy {
                    error("Wait for the response or pause the download before closing.");
                    return 0;
                }
            }
            _ => {}
        }
    }
    if msg == 2 {
        KillTimer(hwnd, 1);
        PostQuitMessage(0);
        return 0;
    }
    DefWindowProcW(hwnd, msg, w, l)
}
fn create(root: PathBuf, smoke: bool) -> Result<()> {
    unsafe {
        let cfg = core::config(&root)?;
        let state = core::load_chats(&cfg)?;
        let name = wide("AIUSBNativeRust");
        let instance = GetModuleHandleW(null());
        let class = WndClass {
            style: 3,
            proc: Some(proc),
            cls: 0,
            win: 0,
            instance,
            icon: 0,
            cursor: LoadCursorW(0, 32512 as *const u16),
            background: theme::background(),
            menu: null(),
            name: name.as_ptr(),
        };
        RegisterClassW(&class);
        theme::register(instance);
        let hwnd = CreateWindowExW(
            0,
            name.as_ptr(),
            wide("AI USB - Rust Desktop").as_ptr(),
            0x02CF0000,
            100,
            70,
            1100,
            780,
            0,
            0,
            instance,
            null_mut(),
        );
        if hwnd == 0 {
            return Err(err(std::io::Error::last_os_error()));
        }
        theme::dark_title(hwnd);
        let (tx, rx) = mpsc::channel();
        let config_text =
            std::fs::read_to_string(root.join("config/settings.json")).map_err(err)?;
        let mut app = Box::new(App {
            hwnd,
            root,
            cfg,
            state,
            active: None,
            controls: HashMap::new(),
            groups: HashMap::new(),
            page: 0,
            busy: false,
            rx,
            tx,
            cancel: Arc::new(AtomicBool::new(false)),
            attachment: None,
            config_text,
            scroll: std::cell::Cell::new(0),
            total_height: std::cell::Cell::new(0),
        });
        for (i, label) in [
            "Chat",
            "Services and logs",
            "Configuration",
            "Backup / restore",
            "Model downloads",
        ]
        .iter()
        .enumerate()
        {
            app.control(10 + i as i32, -1, "BUTTON", label, 0);
        }
        app.control(
            900,
            -1,
            "STATIC",
            "Ready - native Rust; chat uses no Python proxy",
            0,
        );
        app.control(901, -1, "STATIC", "AI USB", 0);
        app.control(902, 0, "STATIC", "CONVERSATIONS", 0);
        app.control(903, -1, "STATIC", "Chat", 0);
        SendMessageW(app.controls[&901], 0x30, theme::font(22) as usize, 1);
        SendMessageW(app.controls[&903], 0x30, theme::font(22) as usize, 1);
        SendMessageW(app.controls[&900], 0x30, theme::font(13) as usize, 1);
        let edit = 0x00800000 | 0x00200000 | 4 | 0x40 | 0x1000;
        app.control(101, 0, "LISTBOX", "", 0x00800000 | 0x00200000 | 1);
        app.control(102, 0, "AIUSBChatView", "", 0);
        app.control(103, 0, "EDIT", "", edit);
        app.control(104, 0, "COMBOBOX", "", 3 | 0x200 | 0x00200000);
        app.combo(104, &["Qwen3 - Chat", "GLM-OCR - Image"]);
        for (id, text) in [
            (105, "Send"),
            (106, "+  New conversation"),
            (107, "Reload"),
            (108, "Image..."),
            (109, "Start model"),
            (110, "Delete"),
            (111, "Copy answer"),
        ] {
            app.control(id, 0, "BUTTON", text, 0);
        }
        app.control(201, 1, "EDIT", "", edit | 0x800);
        app.control(202, 1, "COMBOBOX", "", 3 | 0x200 | 0x00200000);
        let keys: Vec<&str> = app.cfg["services"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        app.combo(202, &keys);
        for (id, text) in [
            (203, "Start"),
            (204, "Stop"),
            (205, "Start AI models"),
            (206, "Stop models"),
            (207, "Refresh status"),
            (214, "View log"),
        ] {
            app.control(id, 1, "BUTTON", text, 0);
        }
        app.control(215, 1, "EDIT", "", edit | 0x800);
        app.control(301, 2, "EDIT", "", edit | 0x00100000);
        app.set(301, &app.config_text);
        app.control(302, 2, "BUTTON", "Validate and save", 0);
        app.control(303, 2, "BUTTON", "Reload", 0);
        app.control(400,3,"STATIC","Backups contain conversations and settings, not model weights.\r\nConversations are stored in data/chats.json and shared with the web interface.\r\nClose browser chats before restoring. After restoring, click Chat > Reload.\r\nCurrent data is automatically backed up before restoring.",0);
        for (id, text) in [
            (401, "Create backup"),
            (402, "Restore backup..."),
            (403, "Open backup folder"),
        ] {
            app.control(id, 3, "BUTTON", text, 0);
        }
        for (id, label) in [
            (510, "Direct HTTPS download URL"),
            (511, "Filename (.gguf, without folders)"),
            (512, "Expected publisher SHA-256"),
        ] {
            app.control(id, 4, "STATIC", label, 0);
            app.control(id - 9, 4, "EDIT", "", 0x00800000 | 0x80);
        }
        for (id, text) in [
            (504, "Download / resume"),
            (505, "Pause"),
            (506, "Open model folder"),
        ] {
            app.control(id, 4, "BUTTON", text, 0);
        }
        app.control(513,4,"STATIC","Progress appears below. A download becomes a .gguf file only after SHA-256 verification.\r\nUse the same URL, filename, and hash to resume.\r\nThen select the new file under --model / --mmproj in Configuration.",0);
        app.active = app.state["chats"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|c| c["id"].as_str())
            .map(String::from);
        app.refresh_chats();
        app.layout();
        SetWindowLongPtrW(hwnd, -21, (&mut *app as *mut App) as isize);
        ShowWindow(hwnd, 5);
        SetFocus(app.controls[&103]);
        SetTimer(hwnd, 1, 150, 0);
        if smoke {
            SetTimer(hwnd, 2, 1500, 0);
        }
        let mut msg: Msg = std::mem::zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            if IsDialogMessageW(hwnd, &mut msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        Ok(())
    }
}
pub fn run(root: PathBuf) -> Result<()> {
    create(root, false)
}
pub fn pick_model_file()->Option<PathBuf>{file_dialog(0,"GGUF model of projector\0*.gguf\0\0")}
pub fn smoke_test(root: PathBuf) -> Result<()> {
    create(root, true)
}
