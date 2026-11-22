//! Всё, что общается с Windows: права администратора, PowerShell, память, окно.

use std::io::{BufRead, BufReader};
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LUID};
use windows_sys::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES,
    TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

pub fn is_admin() -> bool {
    unsafe { IsUserAnAdmin() != 0 }
}

/// Перезапускает текущий exe с UAC. true — новый процесс запущен.
pub fn relaunch_as_admin() -> bool {
    let exe = std::env::current_exe().unwrap_or_default();
    let args: Vec<String> = std::env::args().skip(1).map(|a| format!("\"{a}\"")).collect();
    let (verb, file, params) = (wide("runas"), wide(&exe.to_string_lossy()), wide(&args.join(" ")));
    let r = unsafe {
        ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), file.as_ptr(), params.as_ptr(), std::ptr::null(), SW_SHOWNORMAL)
    };
    r as isize > 32
}

/// Общие функции, доступные каждому скрипту твика.
pub const PRELUDE: &str = r#"
$ProgressPreference = 'SilentlyContinue'
$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
function Reg($p, $n, $v, $t = 'DWord') { if (!(Test-Path $p)) { New-Item $p -Force | Out-Null }; Set-ItemProperty -Path $p -Name $n -Value $v -Type $t -Force }
function Unreg($p, $n) { Remove-ItemProperty -Path $p -Name $n -Force -EA SilentlyContinue }
function IsReg($p, $n, $v) { try { (Get-ItemProperty -Path $p -Name $n -EA Stop).$n -eq $v } catch { $false } }
function SvcOff([string[]]$n) { foreach ($s in $n) { if (Get-Service $s -EA SilentlyContinue) { Stop-Service $s -Force -EA SilentlyContinue; Set-Service $s -StartupType Disabled -EA SilentlyContinue; "  − $s" } } }
function SvcOn([string[]]$n, [string]$t = 'Manual') { foreach ($s in $n) { Set-Service $s -StartupType $t -EA SilentlyContinue } }
function SvcIsOff([string[]]$n) { @($n | ? { $x = Get-Service $_ -EA SilentlyContinue; $x -and $x.StartType -ne 'Disabled' }).Count -eq 0 }
function PcAc($sub, $set) { $m = [regex]::Matches("$(powercfg -q SCHEME_CURRENT $sub $set)", '0x[0-9a-fA-F]{8}'); [Convert]::ToInt32($m[$m.Count - 2].Value, 16) }
function RestorePoint($d) {
  Reg 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\SystemRestore' 'SystemRestorePointCreationFrequency' 0
  Enable-ComputerRestore -Drive "$env:SystemDrive\" -EA SilentlyContinue
  try { Checkpoint-Computer -Description $d -RestorePointType MODIFY_SETTINGS -EA Stop; '✓ Точка восстановления создана' } catch { "⚠ Точка восстановления не создана: $($_.Exception.Message)" }
}
"#;

/// Запускает скрипт в Windows PowerShell 5.1 и отдаёт каждую строку вывода (stdout+stderr) в `on_line`.
pub fn run_ps(body: &str, mut on_line: impl FnMut(String)) -> bool {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let dir = std::env::temp_dir().join("SparkTweak");
    let _ = std::fs::create_dir_all(&dir);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = dir.join(format!("job_{}_{n}.ps1", std::process::id()));
    // BOM обязателен: без него PowerShell 5.1 читает кириллицу как ANSI.
    let script = format!("\u{feff}{PRELUDE}\n& {{\n{body}\n}} 2>&1 | ForEach-Object {{ \"$_\" }}\n");
    if let Err(e) = std::fs::write(&path, script) {
        on_line(format!("✗ Не удалось записать скрипт: {e}"));
        return false;
    }
    let child = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    let ok = match child {
        Ok(mut c) => {
            if let Some(out) = c.stdout.take() {
                for line in BufReader::new(out).split(b'\n').map_while(Result::ok) {
                    let l = String::from_utf8_lossy(&line).trim_end().to_string();
                    if !l.is_empty() {
                        on_line(l);
                    }
                }
            }
            // ошибки разбора скрипта приходят в stderr мимо `2>&1`
            if let Some(mut err) = c.stderr.take() {
                let mut e = Vec::new();
                let _ = std::io::Read::read_to_end(&mut err, &mut e);
                String::from_utf8_lossy(&e).lines().filter(|l| !l.trim().is_empty()).for_each(|l| on_line(l.to_string()));
            }
            c.wait().map(|s| s.success()).unwrap_or(false)
        }
        Err(e) => {
            on_line(format!("✗ Не удалось запустить PowerShell: {e}"));
            false
        }
    };
    let _ = std::fs::remove_file(&path);
    ok
}

pub fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

// ─────────────── Память ───────────────

#[derive(Clone, Copy)]
pub struct Mem {
    pub total: u64,
    pub avail: u64,
    pub load: u32,
    pub commit_total: u64,
    pub commit_limit: u64,
}

pub fn mem_status() -> Mem {
    let mut m: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    m.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
    unsafe { GlobalMemoryStatusEx(&mut m) };
    Mem {
        total: m.ullTotalPhys,
        avail: m.ullAvailPhys,
        load: m.dwMemoryLoad,
        commit_total: m.ullTotalPageFile - m.ullAvailPageFile,
        commit_limit: m.ullTotalPageFile,
    }
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtSetSystemInformation(class: i32, info: *const std::ffi::c_void, len: u32) -> i32;
}

fn enable_privilege(name: &str) -> bool {
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut luid = LUID { LowPart: 0, HighPart: 0 };
        let n = wide(name);
        let ok = LookupPrivilegeValueW(std::ptr::null(), n.as_ptr(), &mut luid) != 0 && {
            let tp = TOKEN_PRIVILEGES {
                PrivilegeCount: 1,
                Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }],
            };
            AdjustTokenPrivileges(token, 0, &tp, 0, std::ptr::null_mut(), std::ptr::null_mut()) != 0
        };
        CloseHandle(token);
        ok
    }
}

/// Команды SystemMemoryListInformation (так же работают RAMMap и ISLC).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MemCmd {
    WorkingSets = 2,
    ModifiedList = 3,
    StandbyList = 4,
    LowPriorityStandby = 5,
}

impl MemCmd {
    pub fn label(self) -> &'static str {
        match self {
            MemCmd::WorkingSets => "Рабочие наборы процессов",
            MemCmd::ModifiedList => "Список изменённых страниц",
            MemCmd::StandbyList => "Standby-список (кэш)",
            MemCmd::LowPriorityStandby => "Standby низкого приоритета",
        }
    }
}

/// Выполняет команду очистки. Ok(()) или NTSTATUS ошибки.
pub fn mem_purge(cmd: MemCmd) -> Result<(), i32> {
    enable_privilege("SeProfileSingleProcessPrivilege");
    enable_privilege("SeIncreaseQuotaPrivilege");
    let c = cmd as i32;
    let st = unsafe { NtSetSystemInformation(80, &c as *const i32 as _, 4) };
    if st >= 0 { Ok(()) } else { Err(st) }
}

pub fn uptime_secs() -> u64 {
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() / 1000 }
}

/// Скруглённые углы Windows 11 для окна без рамки.
pub fn round_corners(hwnd: isize) {
    use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND};
    let pref = DWMWCP_ROUND;
    unsafe {
        DwmSetWindowAttribute(hwnd as _, DWMWA_WINDOW_CORNER_PREFERENCE as u32, &pref as *const _ as _, 4);
    }
}

pub fn message_box(text: &str, title: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION};
    let (t, c) = (wide(text), wide(title));
    unsafe { MessageBoxW(std::ptr::null_mut(), t.as_ptr(), c.as_ptr(), MB_ICONINFORMATION) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_escapes_single_quotes() {
        assert_eq!(ps_quote("it's"), "'it''s'");
    }

    #[test]
    fn ps_roundtrip_utf8() {
        let mut out = vec![];
        assert!(run_ps("'привет ⚡'; Write-Error 'oops'", |l| out.push(l)));
        assert_eq!(out[0], "привет ⚡");
        assert!(out[1].contains("oops"), "stderr должен попадать в вывод: {out:?}");
    }

    #[test]
    fn mem_status_is_sane() {
        let m = mem_status();
        assert!(m.total > m.avail && m.avail > 0);
    }
}
