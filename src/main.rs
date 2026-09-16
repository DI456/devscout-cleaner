#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::ffi::{c_char, c_void, OsStr};
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::ptr::{null, null_mut};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc, Arc,
};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, env};

type Hwnd = isize;
type Hkey = isize;

const ERROR_SUCCESS: u32 = 0;
const ERROR_MORE_DATA: u32 = 234;
const ERROR_NO_MORE_ITEMS: u32 = 259;
const ERROR_FILE_NOT_FOUND: u32 = 2;
const KEY_READ: u32 = 0x20019;
const KEY_WRITE: u32 = 0x20006;
const KEY_WOW64_64KEY: u32 = 0x0100;
const KEY_WOW64_32KEY: u32 = 0x0200;
const REG_SZ: u32 = 1;
const REG_EXPAND_SZ: u32 = 2;
const REG_MULTI_SZ: u32 = 7;
const HKCU: Hkey = 0x80000001u32 as isize;
const HKLM: Hkey = 0x80000002u32 as isize;

const WM_CREATE: u32 = 0x0001;
const WM_DESTROY: u32 = 0x0002;
const WM_COMMAND: u32 = 0x0111;
const WM_DROPFILES: u32 = 0x0233;
const WM_SETFONT: u32 = 0x0030;
const WM_APP: u32 = 0x8000;
const WM_SCAN_PROGRESS: u32 = WM_APP + 1;
const WM_SCAN_COMPLETE: u32 = WM_APP + 2;
const BN_CLICKED: u16 = 0;
const ES_MULTILINE: u32 = 0x0004;
const ES_AUTOVSCROLL: u32 = 0x0040;
const ES_READONLY: u32 = 0x0800;
const ES_AUTOHSCROLL: u32 = 0x0080;
const WS_CHILD: u32 = 0x40000000;
const WS_VISIBLE: u32 = 0x10000000;
const WS_BORDER: u32 = 0x00800000;
const WS_TABSTOP: u32 = 0x00010000;
const WS_VSCROLL: u32 = 0x00200000;
const WS_OVERLAPPEDWINDOW: u32 = 0x00CF0000;
const WS_EX_CLIENTEDGE: u32 = 0x00000200;
const LVS_REPORT: u32 = 0x0001;
const LVS_SHOWSELALWAYS: u32 = 0x0008;
const LVS_EX_FULLROWSELECT: usize = 0x00000020;
const LVS_EX_CHECKBOXES: usize = 0x00000004;
const LVM_SETEXTENDEDLISTVIEWSTYLE: u32 = 0x1036;
const LVM_INSERTCOLUMNW: u32 = 0x1061;
const LVM_INSERTITEMW: u32 = 0x104D;
const LVM_SETITEMTEXTW: u32 = 0x1074;
const LVM_SETITEMSTATE: u32 = 0x102B;
const LVM_GETITEMSTATE: u32 = 0x102C;
const LVM_DELETEALLITEMS: u32 = 0x1009;
const LVCF_WIDTH: u32 = 0x0002;
const LVCF_TEXT: u32 = 0x0004;
const LVIF_TEXT: u32 = 0x0001;
const LVIS_STATEIMAGEMASK: u32 = 0xF000;
const INDEXTOSTATEIMAGEMASK_CHECKED: u32 = 2 << 12;
const ICC_LISTVIEW_CLASSES: u32 = 0x00000001;
const SW_SHOW: i32 = 5;
const COLOR_WINDOW: isize = 5;
const DEFAULT_GUI_FONT: i32 = 17;
const MB_OK: u32 = 0x00000000;
const MB_ICONERROR: u32 = 0x00000010;
const MB_ICONWARNING: u32 = 0x00000030;
const MB_YESNO: u32 = 0x00000004;
const IDYES: i32 = 6;
const IDC_ARROW: usize = 32512;

const INPUT_ID: usize = 1001;
const SCAN_ID: usize = 1002;
const DELETE_ID: usize = 1003;
const CANCEL_SCAN_ID: usize = 1004;
const OUTPUT_ID: usize = 1005;
const REGISTRY_LIST_ID: usize = 1006;
const BACKUP_ID: usize = 1007;
const OPEN_BACKUP_ID: usize = 1008;
const EN_CHANGE: u16 = 0x0300;
const BS_AUTOCHECKBOX: u32 = 0x0003;
const BM_GETCHECK: u32 = 0x00F0;
const BM_SETCHECK: u32 = 0x00F1;
const BST_CHECKED: usize = 1;

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
struct Msg {
    hwnd: Hwnd,
    message: u32,
    w_param: usize,
    l_param: isize,
    time: u32,
    point: Point,
}

#[repr(C)]
struct WndClassW {
    style: u32,
    wnd_proc: Option<unsafe extern "system" fn(Hwnd, u32, usize, isize) -> isize>,
    cb_cls_extra: i32,
    cb_wnd_extra: i32,
    h_instance: isize,
    h_icon: isize,
    h_cursor: isize,
    h_brush: isize,
    menu_name: *const u16,
    class_name: *const u16,
}

#[repr(C)]
struct CreateStructW {
    create_params: *mut c_void,
    instance: isize,
    menu: isize,
    parent: Hwnd,
    cy: i32,
    cx: i32,
    y: i32,
    x: i32,
    style: isize,
    name: *const u16,
    class_name: *const u16,
    ex_style: u32,
}

#[repr(C)]
struct InitCommonControlsEx {
    size: u32,
    icc: u32,
}

#[repr(C)]
struct LvColumnW {
    mask: u32,
    fmt: i32,
    width: i32,
    text: *mut u16,
    sub_item: i32,
    image: i32,
    order: i32,
}

#[repr(C)]
struct LvItemW {
    mask: u32,
    item: i32,
    sub_item: i32,
    state: u32,
    state_mask: u32,
    text: *mut u16,
    text_max: i32,
    image: i32,
    param: isize,
    indent: i32,
    group_id: i32,
    columns: u32,
    column_numbers: *mut u32,
    column_formats: *mut i32,
    group: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FileTime {
    low: u32,
    high: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RmUniqueProcess {
    process_id: u32,
    start_time: FileTime,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RmProcessInfo {
    process: RmUniqueProcess,
    app_name: [u16; 256],
    service_name: [u16; 64],
    app_type: u32,
    app_status: u32,
    ts_session_id: u32,
    restartable: i32,
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(class: *const WndClassW) -> u16;
    fn CreateWindowExW(
        ex_style: u32,
        class_name: *const u16,
        window_name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Hwnd,
        menu: isize,
        instance: isize,
        param: *mut c_void,
    ) -> Hwnd;
    fn DefWindowProcW(hwnd: Hwnd, message: u32, w_param: usize, l_param: isize) -> isize;
    fn DispatchMessageW(msg: *const Msg) -> isize;
    fn GetMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32) -> i32;
    fn GetWindowTextLengthW(hwnd: Hwnd) -> i32;
    fn GetWindowTextW(hwnd: Hwnd, text: *mut u16, max_count: i32) -> i32;
    fn LoadCursorW(instance: isize, cursor: usize) -> isize;
    fn MessageBoxW(hwnd: Hwnd, text: *const u16, caption: *const u16, flags: u32) -> i32;
    fn PostMessageW(hwnd: Hwnd, message: u32, w_param: usize, l_param: isize) -> i32;
    fn PostQuitMessage(exit_code: i32);
    fn SendMessageW(hwnd: Hwnd, message: u32, w_param: usize, l_param: isize) -> isize;
    fn SetWindowTextW(hwnd: Hwnd, text: *const u16) -> i32;
    fn ShowWindow(hwnd: Hwnd, command: i32) -> i32;
    fn TranslateMessage(msg: *const Msg) -> i32;
    fn UpdateWindow(hwnd: Hwnd) -> i32;
}

#[link(name = "comctl32")]
extern "system" {
    fn InitCommonControlsEx(init: *const InitCommonControlsEx) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> isize;
    fn GetCurrentProcessId() -> u32;
    fn LoadLibraryW(name: *const u16) -> isize;
    fn GetProcAddress(module: isize, name: *const c_char) -> *const c_void;
}

#[link(name = "gdi32")]
extern "system" {
    fn GetStockObject(index: i32) -> isize;
}

#[link(name = "shell32")]
extern "system" {
    fn DragAcceptFiles(hwnd: Hwnd, accept: i32);
    fn DragQueryFileW(drop: Hwnd, index: u32, file_name: *mut u16, max_count: u32) -> u32;
    fn DragFinish(drop: Hwnd);
    fn IsUserAnAdmin() -> i32;
}

#[link(name = "advapi32")]
extern "system" {
    fn RegOpenKeyExW(
        key: Hkey,
        sub_key: *const u16,
        options: u32,
        access: u32,
        result: *mut Hkey,
    ) -> u32;
    fn RegCloseKey(key: Hkey) -> u32;
    fn RegEnumKeyExW(
        key: Hkey,
        index: u32,
        name: *mut u16,
        name_len: *mut u32,
        reserved: *mut u32,
        class: *mut u16,
        class_len: *mut u32,
        last_write: *mut FileTime,
    ) -> u32;
    fn RegEnumValueW(
        key: Hkey,
        index: u32,
        name: *mut u16,
        name_len: *mut u32,
        reserved: *mut u32,
        value_type: *mut u32,
        data: *mut u8,
        data_len: *mut u32,
    ) -> u32;
    fn RegDeleteTreeW(key: Hkey, sub_key: *const u16) -> u32;
}

type RmStartSessionFn = unsafe extern "system" fn(*mut u32, u32, *mut u16) -> u32;
type RmRegisterResourcesFn = unsafe extern "system" fn(
    u32,
    u32,
    *const *const u16,
    u32,
    *const c_void,
    u32,
    *const *const u16,
) -> u32;
type RmGetListFn =
    unsafe extern "system" fn(u32, *mut u32, *mut u32, *mut RmProcessInfo, *mut u32) -> u32;
type RmEndSessionFn = unsafe extern "system" fn(u32) -> u32;

struct RestartManagerApi {
    module: isize,
    start_session: RmStartSessionFn,
    register_resources: RmRegisterResourcesFn,
    get_list: RmGetListFn,
    end_session: RmEndSessionFn,
}

#[derive(Clone)]
struct RegistryCandidate {
    root: Hkey,
    view: u32,
    sub_key: String,
    display_name: String,
    reason: String,
    certain: bool,
    selected: bool,
}

#[derive(Clone)]
struct ScanResult {
    target: PathBuf,
    size: u64,
    files: u64,
    scanned_registry_keys: u64,
    candidates: Vec<RegistryCandidate>,
}

struct ScanProgress {
    generation: u64,
    message: String,
}

struct ScanWorkerResult {
    generation: u64,
    result: Result<ScanResult, String>,
}

static mut INPUT_HWND: Hwnd = 0;
static mut OUTPUT_HWND: Hwnd = 0;
static mut REGISTRY_LIST_HWND: Hwnd = 0;
static mut LAST_SCAN: Option<ScanResult> = None;
static mut MAIN_HWND: Hwnd = 0;
static mut BACKUP_HWND: Hwnd = 0;
static mut ACTIVE_SCAN_CANCEL: Option<Arc<AtomicBool>> = None;
static SCAN_GENERATION: AtomicU64 = AtomicU64::new(0);

fn wide(text: &str) -> Vec<u16> {
    OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn from_wide(text: &[u16]) -> String {
    let end = text
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(text.len());
    String::from_utf16_lossy(&text[..end])
}

fn set_text(hwnd: Hwnd, text: &str) {
    let value = wide(text);
    unsafe {
        SetWindowTextW(hwnd, value.as_ptr());
    }
}

fn get_text(hwnd: Hwnd) -> String {
    unsafe {
        let length = GetWindowTextLengthW(hwnd);
        if length <= 0 {
            return String::new();
        }
        let mut buffer = vec![0u16; length as usize + 1];
        GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        from_wide(&buffer)
    }
}

fn show_error(message: &str) {
    let text = wide(message);
    let title = wide("DevScout Cleaner");
    unsafe {
        MessageBoxW(
            MAIN_HWND,
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

fn is_protected_directory(path: &Path) -> bool {
    let normalized = normalize_path(path);
    let candidates = [
        std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string()),
        std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string()),
        std::env::var("ProgramFiles(x86)")
            .unwrap_or_else(|_| "C:\\Program Files (x86)".to_string()),
        std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users".to_string()),
        "C:\\ProgramData".to_string(),
    ];
    candidates
        .iter()
        .any(|candidate| normalized == normalize_path(Path::new(candidate)))
}

fn validate_target(input: &str) -> Result<PathBuf, String> {
    let raw = PathBuf::from(input.trim().trim_matches('"'));
    if !raw.is_absolute() {
        return Err("请输入绝对路径。".to_string());
    }
    let target = fs::canonicalize(&raw).map_err(|error| format!("无法访问目录：{error}"))?;
    if !target.is_dir() {
        return Err("目标必须是目录。".to_string());
    }
    if target.parent().is_none() || is_protected_directory(&target) {
        return Err("为避免误删，拒绝清理磁盘根目录或系统关键目录。".to_string());
    }
    Ok(target)
}

fn directory_stats(path: &Path, cancelled: &AtomicBool) -> Option<(u64, u64)> {
    let mut size = 0;
    let mut files = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(current) = stack.pop() {
        if cancelled.load(Ordering::Relaxed) {
            return None;
        }
        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let child = entry.path();
            let metadata = match fs::symlink_metadata(&child) {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            if metadata.is_dir() {
                stack.push(child);
            } else if metadata.is_file() {
                files += 1;
                size += metadata.len();
            }
        }
    }
    Some((size, files))
}

fn restart_manager_api() -> Option<RestartManagerApi> {
    let module_name = wide("rstrtmgr.dll");
    let module = unsafe { LoadLibraryW(module_name.as_ptr()) };
    if module == 0 {
        return None;
    }
    unsafe {
        let start = GetProcAddress(module, c"RmStartSession".as_ptr());
        let register = GetProcAddress(module, c"RmRegisterResources".as_ptr());
        let get_list = GetProcAddress(module, c"RmGetList".as_ptr());
        let end = GetProcAddress(module, c"RmEndSession".as_ptr());
        if start.is_null() || register.is_null() || get_list.is_null() || end.is_null() {
            return None;
        }
        Some(RestartManagerApi {
            module,
            start_session: std::mem::transmute::<*const c_void, RmStartSessionFn>(start),
            register_resources: std::mem::transmute::<*const c_void, RmRegisterResourcesFn>(
                register,
            ),
            get_list: std::mem::transmute::<*const c_void, RmGetListFn>(get_list),
            end_session: std::mem::transmute::<*const c_void, RmEndSessionFn>(end),
        })
    }
}

fn registry_root_label(root: Hkey) -> &'static str {
    if root == HKCU {
        "HKCU"
    } else {
        "HKLM"
    }
}

fn open_registry(root: Hkey, sub_key: &str, view: u32, access: u32) -> Result<Hkey, u32> {
    let key = wide(sub_key);
    let mut handle = 0;
    let result = unsafe { RegOpenKeyExW(root, key.as_ptr(), 0, access | view, &mut handle) };
    if result == ERROR_SUCCESS {
        Ok(handle)
    } else {
        Err(result)
    }
}

fn enum_subkeys(root: Hkey, base: &str, view: u32) -> Vec<String> {
    let handle = match open_registry(root, base, view, KEY_READ) {
        Ok(handle) => handle,
        Err(_) => return Vec::new(),
    };
    let mut result = Vec::new();
    let mut index = 0;
    loop {
        let mut name = vec![0u16; 512];
        let mut length = name.len() as u32 - 1;
        let code = unsafe {
            RegEnumKeyExW(
                handle,
                index,
                name.as_mut_ptr(),
                &mut length,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };
        if code == ERROR_NO_MORE_ITEMS {
            break;
        }
        if code == ERROR_SUCCESS {
            result.push(String::from_utf16_lossy(&name[..length as usize]));
        }
        index += 1;
    }
    unsafe {
        RegCloseKey(handle);
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn walk_registry_key_paths(
    root: Hkey,
    base: &str,
    view: u32,
    depth: u32,
    cancelled: &AtomicBool,
    scanned: &mut u64,
    progress: &mut dyn FnMut(u64),
    visit: &mut dyn FnMut(String),
) {
    if depth == 0 || cancelled.load(Ordering::Relaxed) {
        return;
    }
    for child in enum_subkeys(root, base, view) {
        if cancelled.load(Ordering::Relaxed) {
            return;
        }
        let path = format!("{base}\\{child}");
        *scanned += 1;
        progress(*scanned);
        visit(path.clone());
        if depth > 1 {
            walk_registry_key_paths(
                root,
                &path,
                view,
                depth - 1,
                cancelled,
                scanned,
                progress,
                visit,
            );
        }
    }
}

fn registry_data_to_string(value_type: u32, data: &[u8]) -> Option<String> {
    if value_type != REG_SZ && value_type != REG_EXPAND_SZ && value_type != REG_MULTI_SZ {
        return None;
    }
    if data.len() < 2 {
        return Some(String::new());
    }
    let words = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u16, data.len() / 2) };
    let value = if value_type == REG_MULTI_SZ {
        words
            .split(|word| *word == 0)
            .filter(|part| !part.is_empty())
            .map(from_wide)
            .collect::<Vec<_>>()
            .join(";")
    } else {
        from_wide(words)
    };
    if value_type == REG_EXPAND_SZ {
        Some(expand_environment(value.trim()))
    } else {
        Some(value.trim().to_string())
    }
}

fn enum_registry_values(key: Hkey) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut index = 0;
    let mut name = vec![0u16; 256];
    let mut data = vec![0u8; 4096];
    loop {
        let mut name_length = name.len() as u32 - 1;
        let mut value_type = 0;
        let mut data_length = data.len() as u32;
        let code = unsafe {
            RegEnumValueW(
                key,
                index,
                name.as_mut_ptr(),
                &mut name_length,
                null_mut(),
                &mut value_type,
                data.as_mut_ptr(),
                &mut data_length,
            )
        };
        if code == ERROR_NO_MORE_ITEMS {
            break;
        }
        if code == ERROR_MORE_DATA {
            if name_length as usize >= name.len() - 1 {
                name.resize(name_length as usize + 1, 0);
            }
            if data_length as usize > data.len() {
                data.resize(data_length as usize, 0);
            }
            continue;
        }
        if code == ERROR_SUCCESS {
            let value_name = String::from_utf16_lossy(&name[..name_length as usize]);
            if let Some(value) = registry_data_to_string(value_type, &data[..data_length as usize])
            {
                result.push((value_name, value));
            }
        }
        index += 1;
    }
    result
}

fn expand_environment(value: &str) -> String {
    let mut result = value.to_string();
    for (key, replacement) in std::env::vars() {
        result = result.replace(&format!("%{key}%"), &replacement);
    }
    result
}

fn path_reference_matches(value: &str, target: &str) -> bool {
    let expanded = expand_environment(value).replace('"', "");
    let normalized = normalize_path(Path::new(&expanded));
    let target = target.trim_end_matches('\\');
    if normalized == target {
        return true;
    }
    if !normalized.starts_with(target) {
        return false;
    }
    if normalized.as_bytes().get(target.len()) == Some(&b'\\') {
        return true;
    }
    let text = expanded.replace('/', "\\").to_ascii_lowercase();
    let Some(position) = text.find(target) else {
        return false;
    };
    let before_ok = position == 0
        || text[..position]
            .chars()
            .next_back()
            .is_some_and(|character| !character.is_ascii_alphanumeric());
    let end = position + target.len();
    let after_ok = end == text.len()
        || text[end..]
            .chars()
            .next()
            .is_some_and(|character| !character.is_ascii_alphanumeric());
    before_ok && after_ok
}

fn classify_registry_reference(
    category: &str,
    name: &str,
    value: &str,
    target: &str,
) -> (bool, bool) {
    if !path_reference_matches(value, target) {
        return (false, false);
    }
    let value_name = name.to_ascii_lowercase();
    let certain = value_name == "installlocation" && normalize_path(Path::new(value)) == target
        || value_name == "uninstallstring"
        || value_name == "quietuninstallstring"
        || (category.starts_with("App Paths") && (value_name.is_empty() || value_name == "path"));
    (true, certain)
}

fn target_signals(target: &Path) -> Vec<String> {
    let mut signals = Vec::new();
    if let Some(name) = target.file_name().and_then(|name| name.to_str()) {
        signals.push(name.to_ascii_lowercase());
    }
    if let Ok(entries) = fs::read_dir(target) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
            {
                if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                    signals.push(stem.to_ascii_lowercase());
                }
            }
        }
    }
    signals
        .into_iter()
        .map(|signal| signal.replace(['_', '-', ' '], ""))
        .filter(|signal| signal.len() >= 5 && signal != "unins000")
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn contains_target_signal(text: &str, signals: &[String]) -> Option<String> {
    let normalized = text.to_ascii_lowercase().replace(['_', '-', ' ', '"'], "");
    signals
        .iter()
        .find(|signal| normalized.contains(signal.as_str()))
        .cloned()
}

#[cfg(test)]
fn scan_registry_for_target(target: &Path) -> Vec<RegistryCandidate> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let mut progress = |_| {};
    scan_registry_for_target_with_progress(target, cancelled, &mut progress)
        .map(|(candidates, _)| candidates)
        .unwrap_or_default()
}

enum RegistryScanMessage {
    Progress(u64),
    Complete(Vec<RegistryCandidate>, u64),
}

#[allow(clippy::too_many_arguments)]
fn scan_registry_location(
    root: Hkey,
    base: &str,
    view: u32,
    category: &str,
    target_norm: &str,
    signals: &[String],
    cancelled: &AtomicBool,
    scanned: &mut u64,
    progress: &mut dyn FnMut(u64),
) -> Vec<RegistryCandidate> {
    let depth = if category == "软件键路径引用" {
        4
    } else {
        1
    };
    let mut candidates = Vec::new();
    let mut visit = |sub_key: String| {
        let child = sub_key
            .rsplit_once('\\')
            .map(|(_, value)| value)
            .unwrap_or(&sub_key);
        let handle = match open_registry(root, &sub_key, view, KEY_READ) {
            Ok(handle) => handle,
            Err(_) => return,
        };
        let values = enum_registry_values(handle);
        unsafe {
            RegCloseKey(handle);
        }
        let mut display_name = child.to_string();
        let mut reasons = Vec::new();
        let mut certain = false;
        if let Some(signal) = contains_target_signal(&sub_key, signals) {
            reasons.push(format!("注册表键名包含程序标识 {signal}"));
        }
        for (name, value) in &values {
            if name.eq_ignore_ascii_case("DisplayName") && !value.is_empty() {
                display_name = value.clone();
            }
            let (matched, is_certain) =
                classify_registry_reference(category, name, value, target_norm);
            if matched {
                certain |= is_certain;
                reasons.push(if is_certain {
                    format!("{name} 强匹配目标目录")
                } else {
                    format!("{name} 路径引用目标目录")
                });
            }
            if let Some(signal) = contains_target_signal(value, signals) {
                reasons.push(format!("{name} 包含程序标识 {signal}"));
            }
        }
        if !reasons.is_empty() {
            candidates.push(RegistryCandidate {
                root,
                view,
                sub_key,
                display_name,
                reason: reasons.join("；"),
                certain,
                selected: true,
            });
        }
    };
    walk_registry_key_paths(
        root, base, view, depth, cancelled, scanned, progress, &mut visit,
    );
    candidates
}

fn merge_registry_candidate(
    candidates: &mut Vec<RegistryCandidate>,
    indexes: &mut HashMap<String, usize>,
    candidate: RegistryCandidate,
) {
    let identity = format!(
        "{}\0{}\0{}",
        registry_root_label(candidate.root),
        candidate.view,
        candidate.sub_key.to_ascii_lowercase()
    );
    if let Some(index) = indexes.get(&identity).copied() {
        let existing = &mut candidates[index];
        existing.certain |= candidate.certain;
        if !existing.reason.contains(&candidate.reason) {
            existing.reason.push('；');
            existing.reason.push_str(&candidate.reason);
        }
    } else {
        indexes.insert(identity, candidates.len());
        candidates.push(candidate);
    }
}

fn scan_registry_for_target_with_progress(
    target: &Path,
    cancelled: Arc<AtomicBool>,
    progress: &mut dyn FnMut(u64),
) -> Option<(Vec<RegistryCandidate>, u64)> {
    let target_norm = normalize_path(target);
    let signals = target_signals(target);
    let locations = [
        (
            HKCU,
            "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
            KEY_WOW64_64KEY,
            "卸载项 InstallLocation",
        ),
        (
            HKCU,
            "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
            KEY_WOW64_32KEY,
            "卸载项 InstallLocation",
        ),
        (
            HKLM,
            "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
            KEY_WOW64_64KEY,
            "卸载项 InstallLocation",
        ),
        (
            HKLM,
            "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
            KEY_WOW64_32KEY,
            "卸载项 InstallLocation",
        ),
        (
            HKCU,
            "Software\\Microsoft\\Windows\\CurrentVersion\\App Paths",
            KEY_WOW64_64KEY,
            "App Paths 程序路径",
        ),
        (
            HKCU,
            "Software\\Microsoft\\Windows\\CurrentVersion\\App Paths",
            KEY_WOW64_32KEY,
            "App Paths 程序路径",
        ),
        (
            HKLM,
            "Software\\Microsoft\\Windows\\CurrentVersion\\App Paths",
            KEY_WOW64_64KEY,
            "App Paths 程序路径",
        ),
        (
            HKLM,
            "Software\\Microsoft\\Windows\\CurrentVersion\\App Paths",
            KEY_WOW64_32KEY,
            "App Paths 程序路径",
        ),
        (HKCU, "Software", KEY_WOW64_64KEY, "软件键路径引用"),
        (HKCU, "Software", KEY_WOW64_32KEY, "软件键路径引用"),
        (HKLM, "Software", KEY_WOW64_64KEY, "软件键路径引用"),
        (HKLM, "Software", KEY_WOW64_32KEY, "软件键路径引用"),
    ];
    let location_count = locations.len();
    let (sender, receiver) = mpsc::channel();
    let total_scanned = Arc::new(AtomicU64::new(0));
    let mut handles = Vec::with_capacity(locations.len());
    for (root, base, view, category) in locations {
        let sender = sender.clone();
        let target_norm = target_norm.clone();
        let signals = signals.clone();
        let cancelled = cancelled.clone();
        let total_scanned = total_scanned.clone();
        handles.push(thread::spawn(move || {
            let mut scanned = 0;
            let mut last_reported = 0;
            let mut report = |_: u64| {
                let total = total_scanned.fetch_add(1, Ordering::Relaxed) + 1;
                if total == 1 || total.saturating_sub(last_reported) >= 64 {
                    last_reported = total;
                    let _ = sender.send(RegistryScanMessage::Progress(total));
                }
            };
            let candidates = scan_registry_location(
                root,
                base,
                view,
                category,
                &target_norm,
                &signals,
                &cancelled,
                &mut scanned,
                &mut report,
            );
            let _ = sender.send(RegistryScanMessage::Complete(candidates, scanned));
        }));
    }
    drop(sender);
    let mut candidates: Vec<RegistryCandidate> = Vec::new();
    let mut candidate_indexes: HashMap<String, usize> = HashMap::new();
    let mut scanned = 0;
    let mut completed_locations = 0;
    while completed_locations < location_count {
        match receiver.recv() {
            Ok(RegistryScanMessage::Progress(count)) => progress(count),
            Ok(RegistryScanMessage::Complete(location_candidates, location_scanned)) => {
                scanned += location_scanned;
                for candidate in location_candidates {
                    merge_registry_candidate(&mut candidates, &mut candidate_indexes, candidate);
                }
                completed_locations += 1;
            }
            Err(_) => return None,
        }
    }
    for handle in handles {
        let _ = handle.join();
    }
    if cancelled.load(Ordering::Relaxed) {
        return None;
    }
    progress(scanned);
    candidates.sort_by(|left, right| {
        left.certain
            .cmp(&right.certain)
            .reverse()
            .then_with(|| registry_full_name(left).cmp(&registry_full_name(right)))
            .then_with(|| left.view.cmp(&right.view))
    });
    Some((candidates, scanned))
}

fn registry_full_name(candidate: &RegistryCandidate) -> String {
    format!(
        "{}\\{}",
        registry_root_label(candidate.root),
        candidate.sub_key
    )
}

fn registry_view_label(view: u32) -> &'static str {
    if view == KEY_WOW64_32KEY {
        "32 位"
    } else {
        "64 位"
    }
}

fn registry_view_flag(view: u32) -> &'static str {
    if view == KEY_WOW64_32KEY {
        "/reg:32"
    } else {
        "/reg:64"
    }
}

fn is_user_admin() -> bool {
    unsafe { IsUserAnAdmin() != 0 }
}

fn permission_label() -> &'static str {
    if is_user_admin() {
        "管理员"
    } else {
        "普通用户"
    }
}

fn backup_root_directory() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn backup_session_directory(target: &Path) -> PathBuf {
    let _ = target;
    backup_root_directory()
}

fn backup_file_prefix(target: &Path) -> String {
    let target_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .map(sanitize_filename)
        .unwrap_or_else(|| "target-folder".to_string());
    format!(
        "DevScoutCleaner-{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        target_name
    )
}

fn backup_enabled() -> bool {
    unsafe { SendMessageW(BACKUP_HWND, BM_GETCHECK, 0, 0) as usize == BST_CHECKED }
}

fn open_backup_directory() {
    let directory = backup_root_directory();
    if let Err(error) = fs::create_dir_all(&directory) {
        show_error(&format!("无法创建备份目录：{error}"));
        return;
    }
    if let Err(error) = Command::new("explorer.exe").arg(&directory).spawn() {
        show_error(&format!("无法打开备份目录：{error}"));
    }
}

fn sanitize_filename(value: &str) -> String {
    let result: String = value
        .chars()
        .map(|character| {
            if "\\/:*?\"<>|".contains(character) {
                '_'
            } else {
                character
            }
        })
        .collect();
    if result.is_empty() {
        "registry-key".to_string()
    } else {
        result.chars().take(80).collect()
    }
}

fn backup_registry_candidate(
    candidate: &RegistryCandidate,
    directory: &Path,
    index: usize,
    prefix: &str,
) -> Result<PathBuf, String> {
    let file = directory.join(format!(
        "{prefix}-{:03}-{}-{}-{}.reg",
        index + 1,
        registry_root_label(candidate.root),
        registry_view_label(candidate.view),
        sanitize_filename(&candidate.display_name)
    ));
    let status = Command::new("reg.exe")
        .args([
            "export",
            &registry_full_name(candidate),
            file.to_string_lossy().as_ref(),
            "/y",
            registry_view_flag(candidate.view),
        ])
        .status()
        .map_err(|error| error.to_string())?;
    if status.success() {
        Ok(file)
    } else {
        Err(format!("reg.exe export 返回 {}", status))
    }
}

fn delete_registry_candidate(candidate: &RegistryCandidate) -> Result<(), String> {
    let (parent, leaf) = candidate
        .sub_key
        .rsplit_once('\\')
        .ok_or_else(|| "注册表路径格式无效".to_string())?;
    let handle = open_registry(candidate.root, parent, candidate.view, KEY_WRITE)
        .map_err(|code| format!("打开注册表键失败，错误码 {code}"))?;
    let leaf_wide = wide(leaf);
    let code = unsafe { RegDeleteTreeW(handle, leaf_wide.as_ptr()) };
    unsafe {
        RegCloseKey(handle);
    }
    if code == ERROR_SUCCESS || code == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        Err(format!("删除注册表键失败，错误码 {code}"))
    }
}

fn collect_files(path: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let child = entry.path();
            let metadata = match fs::symlink_metadata(&child) {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            if metadata.is_dir() {
                stack.push(child);
            } else if metadata.is_file() {
                result.push(child);
            }
        }
    }
    result
}

fn locking_processes(path: &Path) -> Vec<(u32, String)> {
    let files = collect_files(path);
    let mut found = Vec::new();
    let Some(api) = restart_manager_api() else {
        return found;
    };
    for chunk in files.chunks(64) {
        let mut session = 0;
        let mut key = vec![0u16; 64];
        if unsafe { (api.start_session)(&mut session, 0, key.as_mut_ptr()) } != ERROR_SUCCESS {
            continue;
        }
        let strings: Vec<Vec<u16>> = chunk
            .iter()
            .map(|file| wide(&file.to_string_lossy()))
            .collect();
        let pointers: Vec<*const u16> = strings.iter().map(|value| value.as_ptr()).collect();
        let registered = unsafe {
            (api.register_resources)(
                session,
                pointers.len() as u32,
                pointers.as_ptr(),
                0,
                null(),
                0,
                null(),
            )
        };
        if registered == ERROR_SUCCESS {
            let mut needed = 0;
            let mut count = 0;
            let mut reasons = 0;
            let first = unsafe {
                (api.get_list)(session, &mut needed, &mut count, null_mut(), &mut reasons)
            };
            if first == ERROR_MORE_DATA && needed > 0 {
                let mut info = vec![
                    RmProcessInfo {
                        process: RmUniqueProcess {
                            process_id: 0,
                            start_time: FileTime { low: 0, high: 0 }
                        },
                        app_name: [0; 256],
                        service_name: [0; 64],
                        app_type: 0,
                        app_status: 0,
                        ts_session_id: 0,
                        restartable: 0
                    };
                    needed as usize
                ];
                count = needed;
                if unsafe {
                    (api.get_list)(
                        session,
                        &mut needed,
                        &mut count,
                        info.as_mut_ptr(),
                        &mut reasons,
                    )
                } == ERROR_SUCCESS
                {
                    for item in info.into_iter().take(count as usize) {
                        let entry = (item.process.process_id, from_wide(&item.app_name));
                        if !found
                            .iter()
                            .any(|existing: &(u32, String)| existing.0 == entry.0)
                        {
                            found.push(entry);
                        }
                    }
                }
            }
        }
        unsafe {
            (api.end_session)(session);
        }
    }
    let _module = api.module;
    found
}

fn process_basename(name: &str) -> &str {
    name.rsplit(['\\', '/']).next().unwrap_or(name)
}

fn is_protected_process(pid: u32, name: &str) -> bool {
    let current_pid = unsafe { GetCurrentProcessId() };
    if pid == 0 || pid == 4 || pid == current_pid {
        return true;
    }
    matches!(
        process_basename(name).to_ascii_lowercase().as_str(),
        "system"
            | "system idle process"
            | "registry"
            | "smss.exe"
            | "csrss.exe"
            | "wininit.exe"
            | "services.exe"
            | "lsass.exe"
            | "svchost.exe"
            | "winlogon.exe"
            | "dwm.exe"
    )
}

fn force_terminate_processes(locks: &[(u32, String)]) -> (usize, Vec<String>, bool) {
    let mut terminated = 0;
    let mut failures = Vec::new();
    let mut killed_explorer = false;
    for (pid, name) in locks {
        if is_protected_process(*pid, name) {
            failures.push(format!("已跳过受保护进程 {name} (PID {pid})"));
            continue;
        }
        let status = Command::new("taskkill.exe")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        match status {
            Ok(result) if result.success() => {
                terminated += 1;
                if process_basename(name).eq_ignore_ascii_case("explorer.exe")
                    || process_basename(name).eq_ignore_ascii_case("explorer")
                {
                    killed_explorer = true;
                }
            }
            Ok(result) => failures.push(format!("终止 {name} (PID {pid}) 失败：{result}")),
            Err(error) => failures.push(format!("终止 {name} (PID {pid}) 失败：{error}")),
        }
    }
    (terminated, failures, killed_explorer)
}

fn restart_explorer() -> bool {
    Command::new("explorer.exe")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}

fn format_size(size: u64) -> String {
    if size >= 1_073_741_824 {
        format!("{:.2} GB", size as f64 / 1_073_741_824.0)
    } else if size >= 1_048_576 {
        format!("{:.1} MB", size as f64 / 1_048_576.0)
    } else if size >= 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else {
        format!("{size} B")
    }
}

fn cancel_active_scan() -> bool {
    unsafe {
        let previous = std::ptr::replace(std::ptr::addr_of_mut!(ACTIVE_SCAN_CANCEL), None);
        if let Some(cancel) = previous {
            cancel.store(true, Ordering::Relaxed);
            return true;
        }
    }
    false
}

fn invalidate_scan() -> u64 {
    let generation = SCAN_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    cancel_active_scan();
    generation
}

fn reset_scan_ui() {
    unsafe {
        LAST_SCAN = None;
        if REGISTRY_LIST_HWND != 0 {
            SendMessageW(REGISTRY_LIST_HWND, LVM_DELETEALLITEMS, 0, 0);
        }
    }
}

fn clear_scan_state() {
    invalidate_scan();
    reset_scan_ui();
    set_text(
        unsafe { OUTPUT_HWND },
        "尚未扫描。请拖入文件夹，或输入绝对路径后点击“扫描”。",
    );
}

fn cancel_scan_clicked() {
    let was_scanning = cancel_active_scan();
    SCAN_GENERATION.fetch_add(1, Ordering::SeqCst);
    reset_scan_ui();
    if was_scanning {
        set_text(
            unsafe { OUTPUT_HWND },
            "扫描已取消。请拖入文件夹，或输入路径后重新扫描。",
        );
    } else {
        set_text(unsafe { OUTPUT_HWND }, "当前没有正在进行的扫描。");
    }
}

fn post_scan_progress(hwnd: Hwnd, generation: u64, message: String) {
    let payload = Box::new(ScanProgress {
        generation,
        message,
    });
    let pointer = Box::into_raw(payload);
    let posted = unsafe { PostMessageW(hwnd, WM_SCAN_PROGRESS, 0, pointer as isize) };
    if posted == 0 {
        unsafe {
            drop(Box::from_raw(pointer));
        }
    }
}

fn post_scan_result(hwnd: Hwnd, result: ScanWorkerResult) {
    let pointer = Box::into_raw(Box::new(result));
    let posted = unsafe { PostMessageW(hwnd, WM_SCAN_COMPLETE, 0, pointer as isize) };
    if posted == 0 {
        unsafe {
            drop(Box::from_raw(pointer));
        }
    }
}

unsafe fn list_set_item_text(list: Hwnd, item: i32, sub_item: i32, text: &str) {
    let mut value = wide(text);
    let mut entry = LvItemW {
        mask: LVIF_TEXT,
        item,
        sub_item,
        state: 0,
        state_mask: 0,
        text: value.as_mut_ptr(),
        text_max: value.len() as i32,
        image: 0,
        param: 0,
        indent: 0,
        group_id: 0,
        columns: 0,
        column_numbers: null_mut(),
        column_formats: null_mut(),
        group: 0,
    };
    SendMessageW(
        list,
        LVM_SETITEMTEXTW,
        item as usize,
        &mut entry as *mut LvItemW as isize,
    );
}

unsafe fn populate_registry_list(candidates: &[RegistryCandidate]) {
    if REGISTRY_LIST_HWND == 0 {
        return;
    }
    SendMessageW(REGISTRY_LIST_HWND, LVM_DELETEALLITEMS, 0, 0);
    for (index, candidate) in candidates.iter().enumerate() {
        let category = if candidate.certain {
            "确定垃圾"
        } else {
            "可疑"
        };
        let mut first = wide(category);
        let mut item = LvItemW {
            mask: LVIF_TEXT,
            item: index as i32,
            sub_item: 0,
            state: 0,
            state_mask: 0,
            text: first.as_mut_ptr(),
            text_max: first.len() as i32,
            image: 0,
            param: index as isize,
            indent: 0,
            group_id: 0,
            columns: 0,
            column_numbers: null_mut(),
            column_formats: null_mut(),
            group: 0,
        };
        let inserted = SendMessageW(
            REGISTRY_LIST_HWND,
            LVM_INSERTITEMW,
            0,
            &mut item as *mut LvItemW as isize,
        ) as i32;
        if inserted < 0 {
            continue;
        }
        list_set_item_text(REGISTRY_LIST_HWND, inserted, 1, &candidate.display_name);
        list_set_item_text(
            REGISTRY_LIST_HWND,
            inserted,
            2,
            registry_view_label(candidate.view),
        );
        list_set_item_text(
            REGISTRY_LIST_HWND,
            inserted,
            3,
            &registry_full_name(candidate),
        );
        list_set_item_text(REGISTRY_LIST_HWND, inserted, 4, &candidate.reason);
        let state = if candidate.selected {
            INDEXTOSTATEIMAGEMASK_CHECKED
        } else {
            1 << 12
        };
        let mut state_item = LvItemW {
            mask: 0,
            item: inserted,
            sub_item: 0,
            state,
            state_mask: LVIS_STATEIMAGEMASK,
            text: null_mut(),
            text_max: 0,
            image: 0,
            param: 0,
            indent: 0,
            group_id: 0,
            columns: 0,
            column_numbers: null_mut(),
            column_formats: null_mut(),
            group: 0,
        };
        SendMessageW(
            REGISTRY_LIST_HWND,
            LVM_SETITEMSTATE,
            inserted as usize,
            &mut state_item as *mut LvItemW as isize,
        );
    }
}

fn selected_candidates(scan: &ScanResult) -> Vec<RegistryCandidate> {
    let mut selected = Vec::new();
    for (index, candidate) in scan.candidates.iter().enumerate() {
        let state = unsafe {
            SendMessageW(
                REGISTRY_LIST_HWND,
                LVM_GETITEMSTATE,
                index,
                LVIS_STATEIMAGEMASK as isize,
            ) as u32
        };
        if state & LVIS_STATEIMAGEMASK == INDEXTOSTATEIMAGEMASK_CHECKED {
            selected.push(candidate.clone());
        }
    }
    selected
}

fn scan_clicked() {
    let input = unsafe { get_text(INPUT_HWND) };
    match validate_target(&input) {
        Ok(target) => {
            let generation = invalidate_scan();
            reset_scan_ui();
            set_text(
                unsafe { OUTPUT_HWND },
                "正在统计目录……\n\n注册表扫描尚未开始。\n可以继续拖放或修改路径，当前扫描会自动取消。",
            );
            let cancel = Arc::new(AtomicBool::new(false));
            unsafe {
                ACTIVE_SCAN_CANCEL = Some(cancel.clone());
            }
            let hwnd = unsafe { MAIN_HWND };
            thread::spawn(move || {
                post_scan_progress(
                    hwnd,
                    generation,
                    "正在统计目录……\n\n注册表扫描尚未开始。".to_string(),
                );
                let Some((size, files)) = directory_stats(&target, &cancel) else {
                    return;
                };
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                post_scan_progress(
                    hwnd,
                    generation,
                    format!(
                        "目录统计完成：{} 个文件，{}。\n\n正在搜索注册表……\n已搜索到 0 个注册表项。",
                        files,
                        format_size(size)
                    ),
                );
                let mut last_reported = 0;
                let mut progress = |scanned: u64| {
                    if scanned == 1 || scanned.saturating_sub(last_reported) >= 64 {
                        last_reported = scanned;
                        post_scan_progress(
                            hwnd,
                            generation,
                            format!(
                                "目录统计完成：{} 个文件，{}。\n\n正在搜索注册表……\n已搜索到 {} 个注册表项。",
                                files,
                                format_size(size),
                                scanned
                            ),
                        );
                    }
                };
                let Some((candidates, scanned_registry_keys)) =
                    scan_registry_for_target_with_progress(&target, cancel.clone(), &mut progress)
                else {
                    return;
                };
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                post_scan_result(
                    hwnd,
                    ScanWorkerResult {
                        generation,
                        result: Ok(ScanResult {
                            target,
                            size,
                            files,
                            scanned_registry_keys,
                            candidates,
                        }),
                    },
                );
            });
        }
        Err(error) => {
            invalidate_scan();
            reset_scan_ui();
            show_error(&error);
        }
    }
}

fn delete_clicked() {
    let scan = unsafe { (&*std::ptr::addr_of!(LAST_SCAN)).clone() };
    let Some(scan) = scan else {
        show_error("请先扫描一个目录。");
        return;
    };
    let current = match validate_target(&get_text(unsafe { INPUT_HWND })) {
        Ok(path) => path,
        Err(error) => {
            show_error(&error);
            return;
        }
    };
    if normalize_path(&current) != normalize_path(&scan.target) {
        show_error("目录路径已改变，请重新扫描。");
        return;
    }
    let selected = selected_candidates(&scan);
    let should_backup = backup_enabled();
    let mut prompt = format!(
        "确定永久删除目录？\n\n{}\n\n大小：{}，文件：{}",
        scan.target.display(),
        format_size(scan.size),
        scan.files
    );
    if !selected.is_empty() {
        if should_backup {
            prompt.push_str(&format!(
                "\n\n同时清理 {} 个已勾选注册表项（删除前备份）。",
                selected.len()
            ));
        } else {
            prompt.push_str(&format!(
                "\n\n同时清理 {} 个已勾选注册表项。\n警告：当前未启用注册表备份。",
                selected.len()
            ));
        }
    }
    if !is_user_admin() {
        prompt.push_str("\n\n当前权限：普通用户。部分文件或 HKLM 注册表项可能需要管理员权限。");
    }
    let text = wide(&prompt);
    let title = wide("确认清理");
    if unsafe {
        MessageBoxW(
            MAIN_HWND,
            text.as_ptr(),
            title.as_ptr(),
            MB_YESNO | MB_ICONWARNING,
        )
    } != IDYES
    {
        return;
    }
    let mut output = String::new();
    let backup_prefix = if should_backup && !selected.is_empty() {
        Some(backup_file_prefix(&scan.target))
    } else {
        None
    };
    let backup_directory = if should_backup && !selected.is_empty() {
        let directory = backup_session_directory(&scan.target);
        match fs::create_dir_all(&directory) {
            Ok(_) => {
                output.push_str(&format!("注册表备份位置：{}\n", directory.display()));
                Some(directory)
            }
            Err(error) => {
                output.push_str(&format!(
                    "注册表备份目录创建失败：{error}\n文件删除已取消。"
                ));
                None
            }
        }
    } else {
        None
    };
    if should_backup && !selected.is_empty() && backup_directory.is_none() {
        set_text(unsafe { OUTPUT_HWND }, &output);
        return;
    }
    match fs::remove_dir_all(&scan.target) {
        Ok(_) => output.push_str(&format!("目录已删除：{}\n", scan.target.display())),
        Err(error) => {
            output.push_str(&format!("普通删除失败：{error}\n"));
            let locks = locking_processes(&scan.target);
            if locks.is_empty() {
                output.push_str(
                    "Restart Manager 未发现占用进程，可能是权限或只读属性问题；注册表尚未修改。\n",
                );
                set_text(unsafe { OUTPUT_HWND }, &output);
                return;
            }
            output.push_str("检测到占用进程，正在执行强力删除……\n");
            for (pid, name) in &locks {
                output.push_str(&format!("- {name} (PID {pid})\n"));
            }
            let (terminated, failures, killed_explorer) = force_terminate_processes(&locks);
            output.push_str(&format!("已终止 {terminated} 个占用进程，正在重试删除……\n"));
            if killed_explorer {
                if restart_explorer() {
                    output.push_str("Windows Explorer 已重新启动。\n");
                } else {
                    output.push_str("Windows Explorer 自动重启失败。\n");
                }
            }
            thread::sleep(Duration::from_millis(300));
            if let Err(retry_error) = fs::remove_dir_all(&scan.target) {
                output.push_str(&format!("强力删除仍失败：{retry_error}\n"));
                for failure in failures {
                    output.push_str(&format!("{failure}\n"));
                }
                output.push_str("注册表尚未修改。\n");
                set_text(unsafe { OUTPUT_HWND }, &output);
                return;
            }
            output.push_str("强力删除成功。\n");
        }
    }
    if !selected.is_empty() {
        if let Some(directory) = backup_directory {
            for (index, candidate) in selected.iter().enumerate() {
                match backup_registry_candidate(
                    candidate,
                    &directory,
                    index,
                    backup_prefix.as_deref().unwrap_or("DevScoutCleaner"),
                ) {
                    Ok(file) => match delete_registry_candidate(candidate) {
                        Ok(_) => output.push_str(&format!(
                            "已清理注册表：{}\n备份：{}\n",
                            registry_full_name(candidate),
                            file.display()
                        )),
                        Err(error) => output.push_str(&format!(
                            "注册表删除失败：{}：{}\n备份：{}\n",
                            registry_full_name(candidate),
                            error,
                            file.display()
                        )),
                    },
                    Err(error) => output.push_str(&format!(
                        "已跳过注册表（备份失败）：{}：{}\n",
                        registry_full_name(candidate),
                        error
                    )),
                }
            }
        }
    }
    output.push_str("\n清理完成。\n");
    unsafe {
        LAST_SCAN = None;
    }
    set_text(unsafe { OUTPUT_HWND }, &output);
}

#[allow(clippy::too_many_arguments)]
unsafe fn create_control(
    class: &str,
    text: &str,
    style: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    parent: Hwnd,
    instance: isize,
    id: usize,
) -> Hwnd {
    let class_wide = wide(class);
    let text_wide = wide(text);
    let hwnd = CreateWindowExW(
        0,
        class_wide.as_ptr(),
        text_wide.as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        x,
        y,
        width,
        height,
        parent,
        id as isize,
        instance,
        null_mut(),
    );
    let font = GetStockObject(DEFAULT_GUI_FONT);
    SendMessageW(hwnd, WM_SETFONT, font as usize, 1);
    hwnd
}

unsafe fn configure_registry_list(list: Hwnd) {
    SendMessageW(
        list,
        LVM_SETEXTENDEDLISTVIEWSTYLE,
        LVS_EX_FULLROWSELECT | LVS_EX_CHECKBOXES,
        (LVS_EX_FULLROWSELECT | LVS_EX_CHECKBOXES) as isize,
    );
    let columns = [
        ("分类", 90),
        ("名称", 220),
        ("视图", 70),
        ("注册表路径", 420),
        ("判定依据", 560),
    ];
    for (index, (title, width)) in columns.iter().enumerate() {
        let mut text = wide(title);
        let mut column = LvColumnW {
            mask: LVCF_TEXT | LVCF_WIDTH,
            fmt: 0,
            width: *width,
            text: text.as_mut_ptr(),
            sub_item: index as i32,
            image: 0,
            order: index as i32,
        };
        SendMessageW(
            list,
            LVM_INSERTCOLUMNW,
            index,
            &mut column as *mut LvColumnW as isize,
        );
    }
}

unsafe extern "system" fn window_proc(
    hwnd: Hwnd,
    message: u32,
    w_param: usize,
    l_param: isize,
) -> isize {
    match message {
        WM_CREATE => {
            MAIN_HWND = hwnd;
            let create = &*(l_param as *const CreateStructW);
            let instance = create.instance;
            let label = create_control(
                "STATIC",
                "目标文件夹：",
                0,
                16,
                18,
                100,
                24,
                hwnd,
                instance,
                0,
            );
            let _ = label;
            INPUT_HWND = create_control(
                "EDIT",
                "",
                WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL,
                112,
                14,
                530,
                26,
                hwnd,
                instance,
                INPUT_ID,
            );
            let _ = create_control(
                "BUTTON", "扫描", WS_TABSTOP, 652, 14, 80, 26, hwnd, instance, SCAN_ID,
            );
            let _ = create_control(
                "BUTTON",
                "取消扫描",
                WS_TABSTOP,
                738,
                14,
                80,
                26,
                hwnd,
                instance,
                CANCEL_SCAN_ID,
            );
            let _ = create_control(
                "BUTTON",
                "删除并清理",
                WS_TABSTOP,
                824,
                14,
                110,
                26,
                hwnd,
                instance,
                DELETE_ID,
            );
            let _ = create_control(
                "BUTTON",
                "打开备份目录",
                WS_TABSTOP,
                940,
                14,
                140,
                26,
                hwnd,
                instance,
                OPEN_BACKUP_ID,
            );
            let instruction = format!(
                "拖入文件夹后点击扫描；列表默认全选，取消勾选不想清理的注册表项。当前权限：{}。",
                permission_label()
            );
            let _ = create_control(
                "STATIC",
                &instruction,
                0,
                16,
                50,
                650,
                26,
                hwnd,
                instance,
                0,
            );
            BACKUP_HWND = create_control(
                "BUTTON",
                "删除前备份注册表",
                WS_TABSTOP | BS_AUTOCHECKBOX,
                680,
                50,
                180,
                26,
                hwnd,
                instance,
                BACKUP_ID,
            );
            SendMessageW(BACKUP_HWND, BM_SETCHECK, BST_CHECKED, 0);
            OUTPUT_HWND = create_control(
                "EDIT",
                "尚未扫描。请拖入文件夹，或输入绝对路径后点击“扫描”。",
                WS_BORDER | ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY | WS_VSCROLL,
                16,
                82,
                1060,
                82,
                hwnd,
                instance,
                OUTPUT_ID,
            );
            REGISTRY_LIST_HWND = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                wide("SysListView32").as_ptr(),
                wide("").as_ptr(),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_BORDER
                    | WS_TABSTOP
                    | LVS_REPORT
                    | LVS_SHOWSELALWAYS
                    | WS_VSCROLL,
                16,
                178,
                1060,
                365,
                hwnd,
                REGISTRY_LIST_ID as isize,
                instance,
                null_mut(),
            );
            let font = GetStockObject(DEFAULT_GUI_FONT);
            SendMessageW(REGISTRY_LIST_HWND, WM_SETFONT, font as usize, 1);
            configure_registry_list(REGISTRY_LIST_HWND);
            DragAcceptFiles(hwnd, 1);
            set_text(INPUT_HWND, "");
            0
        }
        WM_SCAN_PROGRESS => {
            if l_param != 0 {
                let progress = Box::from_raw(l_param as *mut ScanProgress);
                if progress.generation == SCAN_GENERATION.load(Ordering::SeqCst) {
                    set_text(OUTPUT_HWND, &progress.message);
                }
            }
            0
        }
        WM_SCAN_COMPLETE => {
            if l_param != 0 {
                let completed = Box::from_raw(l_param as *mut ScanWorkerResult);
                if completed.generation != SCAN_GENERATION.load(Ordering::SeqCst) {
                    return 0;
                }
                ACTIVE_SCAN_CANCEL = None;
                match completed.result {
                    Ok(scan) => {
                        let mut output = format!(
                            "扫描完成\n\n目录：{}\n文件：{}\n大小：{}\n已搜索注册表项：{}\n当前权限：{}\n\n",
                            scan.target.display(),
                            scan.files,
                            format_size(scan.size),
                            scan.scanned_registry_keys,
                            permission_label(),
                        );
                        if scan.candidates.is_empty() {
                            output.push_str("注册表：未发现路径引用。\n");
                        } else {
                            output.push_str(&format!(
                                "注册表候选：{} 项；上方为确定垃圾，下方为可疑项。\n\n",
                                scan.candidates.len(),
                            ));
                        }
                        populate_registry_list(&scan.candidates);
                        LAST_SCAN = Some(scan);
                        set_text(OUTPUT_HWND, &output);
                    }
                    Err(error) => {
                        reset_scan_ui();
                        set_text(OUTPUT_HWND, &format!("扫描失败：{error}"));
                    }
                }
            }
            0
        }
        WM_COMMAND => {
            let id = w_param & 0xffff;
            let notify = (w_param >> 16) as u16;
            if id == INPUT_ID && notify == EN_CHANGE {
                clear_scan_state();
            } else if notify == BN_CLICKED && id == SCAN_ID {
                scan_clicked();
            } else if notify == BN_CLICKED && id == CANCEL_SCAN_ID {
                cancel_scan_clicked();
            } else if notify == BN_CLICKED && id == DELETE_ID {
                delete_clicked();
            } else if notify == BN_CLICKED && id == OPEN_BACKUP_ID {
                open_backup_directory();
            }
            0
        }
        WM_DROPFILES => {
            let drop = w_param as Hwnd;
            let length = DragQueryFileW(drop, 0, null_mut(), 0);
            if length > 0 {
                let mut buffer = vec![0u16; length as usize + 1];
                DragQueryFileW(drop, 0, buffer.as_mut_ptr(), buffer.len() as u32);
                set_text(INPUT_HWND, &from_wide(&buffer));
                clear_scan_state();
            }
            DragFinish(drop);
            0
        }
        WM_DESTROY => {
            invalidate_scan();
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, message, w_param, l_param),
    }
}

fn run_gui() -> Result<(), String> {
    let class_name = wide("DevScoutCleanerWindow");
    let title = wide("DevScout Cleaner - 文件与注册表清理");
    unsafe {
        let common_controls = InitCommonControlsEx {
            size: std::mem::size_of::<InitCommonControlsEx>() as u32,
            icc: ICC_LISTVIEW_CLASSES,
        };
        if InitCommonControlsEx(&common_controls) == 0 {
            return Err("初始化列表控件失败。".to_string());
        }
        let instance = GetModuleHandleW(null());
        let class = WndClassW {
            style: 0,
            wnd_proc: Some(window_proc),
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_instance: instance,
            h_icon: 0,
            h_cursor: LoadCursorW(0, IDC_ARROW),
            h_brush: COLOR_WINDOW + 1,
            menu_name: null(),
            class_name: class_name.as_ptr(),
        };
        if RegisterClassW(&class) == 0 {
            return Err("注册窗口类失败。".to_string());
        }
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            100,
            100,
            1100,
            600,
            0,
            0,
            instance,
            null_mut(),
        );
        if hwnd == 0 {
            return Err("创建窗口失败。".to_string());
        }
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        let mut msg = Msg {
            hwnd: 0,
            message: 0,
            w_param: 0,
            l_param: 0,
            time: 0,
            point: Point { x: 0, y: 0 },
        };
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run_gui() {
        show_error(&error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_quoted_uninstall_command() {
        assert!(path_reference_matches(
            "\"D:\\toss pc\\QuarkCloudDrive\\unins000.exe\" --brand-clouddrive /SILENT",
            "d:\\toss pc\\quarkclouddrive"
        ));
    }

    #[test]
    fn rejects_similar_prefix() {
        assert!(!path_reference_matches(
            "D:\\toss pc\\QuarkCloudDriveBackup\\unins000.exe",
            "d:\\toss pc\\quarkclouddrive"
        ));
    }

    #[test]
    fn decodes_registry_string_values() {
        let bytes: Vec<u8> = "D:\\apps\\demo\\demo.exe\0"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(
            registry_data_to_string(REG_SZ, &bytes),
            Some("D:\\apps\\demo\\demo.exe".to_string())
        );
    }

    #[test]
    #[ignore = "需要本机安装夸克网盘，仅用于本地回归验证"]
    fn discovers_quark_install_record_when_present() {
        let target = Path::new("D:\\toss pc\\QuarkCloudDrive");
        if target.is_dir() {
            let candidates = scan_registry_for_target(target);
            assert!(candidates.iter().any(|candidate| {
                candidate.reason.contains("InstallLocation")
                    && candidate.display_name.contains("夸克网盘")
            }));
        }
    }

    #[test]
    fn protects_system_processes_from_force_delete() {
        assert!(is_protected_process(4, "System"));
        assert!(is_protected_process(12345, "svchost.exe"));
    }
}
