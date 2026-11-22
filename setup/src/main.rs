#![windows_subsystem = "windows"]
#![allow(dead_code)] // sys.rs и style.rs общие с приложением, здесь нужна только часть

#[path = "../../app/src/style.rs"]
mod style;
#[path = "../../app/src/sys.rs"]
mod sys;

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use eframe::egui::{self, RichText, vec2};
use style::{DIM, MINT, RED, bold, glass, glass_accent};

static APP_EXE: &[u8] = include_bytes!(env!("SPARK_APP_EXE"));
const VERSION: &str = env!("CARGO_PKG_VERSION");

enum State {
    Ready,
    Installing(Receiver<Result<PathBuf, String>>),
    Done(PathBuf),
    Failed(String),
}

struct Setup {
    dir: String,
    desktop: bool,
    launch: bool,
    state: State,
    installed: bool,
    admin: bool,
}

/// Без прав администратора ставим только для текущего пользователя (как VS Code): туда писать можно всегда.
fn default_dir(admin: bool) -> String {
    let base = if admin {
        std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into())
    } else {
        format!(r"{}\Programs", std::env::var("LOCALAPPDATA").unwrap_or_default())
    };
    format!(r"{base}\SparkTweak")
}

/// Папка, куда реально ляжет программа: если пользователь указал «D:\Games», создаём «D:\Games\SparkTweak».
fn target_dir(input: &str) -> PathBuf {
    let p = PathBuf::from(input.trim().trim_matches('"'));
    if p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("SparkTweak")) { p } else { p.join("SparkTweak") }
}

fn install(dir: PathBuf, desktop: bool, admin: bool) -> Result<PathBuf, String> {
    // запущенную копию надо закрыть, иначе exe заблокирован
    sys::run_ps("Stop-Process -Name sparktweak -Force -EA SilentlyContinue; Start-Sleep -Milliseconds 400", |_| {});
    std::fs::create_dir_all(&dir).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied && !admin {
            format!("Нет доступа к папке {}. Выберите другую папку или нажмите «Установить для всех» (нужны права администратора).", dir.display())
        } else {
            format!("Не удалось создать папку {}: {e}", dir.display())
        }
    })?;
    let exe = dir.join("SparkTweak.exe");
    std::fs::write(&exe, APP_EXE).map_err(|e| format!("Не удалось записать {}: {e}", exe.display()))?;
    let exe_s = exe.to_string_lossy();
    let q = |s: &str| sys::ps_quote(s);
    let (programs, desk, hive) = if admin { ("CommonPrograms", "CommonDesktopDirectory", "HKLM") } else { ("Programs", "DesktopDirectory", "HKCU") };
    let script = format!(
        r#"$exe = {exe}
$ws = New-Object -ComObject WScript.Shell
$links = @("$([Environment]::GetFolderPath('{programs}'))\SparkTweak.lnk")
if ({desktop}) {{ $links += "$([Environment]::GetFolderPath('{desk}'))\SparkTweak.lnk" }}
foreach ($l in $links) {{ $s = $ws.CreateShortcut($l); $s.TargetPath = $exe; $s.WorkingDirectory = {dir}; $s.Description = 'SparkTweak — оптимизация Windows'; $s.Save() }}
$k = '{hive}:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\SparkTweak'
New-Item $k -Force | Out-Null
$v = @{{ DisplayName = 'SparkTweak'; DisplayVersion = {ver}; Publisher = 'SparkTweak'; DisplayIcon = $exe; InstallLocation = {dir}; UninstallString = "`"$exe`" --uninstall"; URLInfoAbout = 'https://github.com/s0nd3r-py/SparkTweak' }}
foreach ($n in $v.Keys) {{ Set-ItemProperty $k -Name $n -Value $v[$n] }}
Set-ItemProperty $k -Name NoModify -Value 1 -Type DWord
Set-ItemProperty $k -Name NoRepair -Value 1 -Type DWord
Set-ItemProperty $k -Name EstimatedSize -Value {kb} -Type DWord
if (@($links | ? {{ !(Test-Path $_) }}).Count -eq 0 -and (Test-Path $k)) {{ 'OK' }} else {{ 'Не удалось создать ярлыки или запись в «Установленных приложениях»' }}"#,
        exe = q(&exe_s),
        dir = q(&dir.to_string_lossy()),
        desktop = if desktop { "$true" } else { "$false" },
        ver = q(VERSION),
        kb = APP_EXE.len() / 1024,
    );
    let mut out = vec![];
    sys::run_ps(&script, |l| out.push(l));
    if !exe.exists() {
        return Err(format!("Файл не появился в {}", dir.display()));
    }
    if out.iter().any(|l| l == "OK") { Ok(dir) } else { Err(out.join("\n")) }
}

impl Setup {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        style::setup(&cc.egui_ctx);
        if std::env::var_os("SPARKTWEAK_SOLID").is_none() {
            let _ = window_vibrancy::apply_acrylic(cc, Some((28, 30, 33, 110)));
        }
        if let Ok(h) = raw_window_handle::HasWindowHandle::window_handle(cc)
            && let raw_window_handle::RawWindowHandle::Win32(w) = h.as_raw()
        {
            sys::round_corners(w.hwnd.get());
        }
        let admin = sys::is_admin();
        let dir = default_dir(admin);
        let installed = PathBuf::from(&dir).join("SparkTweak.exe").exists();
        Self { dir, desktop: true, launch: true, state: State::Ready, installed, admin }
    }
}

impl eframe::App for Setup {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let full = ui.max_rect();
        style::paint_background(ui, full, std::env::var_os("SPARKTWEAK_SOLID").is_some());
        let title = egui::Rect::from_min_size(full.min, vec2(full.width(), 34.0));
        style::title_bar(ui, title, "SparkTweak", &format!("Установка · {VERSION}"));

        if let State::Installing(rx) = &self.state
            && let Ok(r) = rx.try_recv()
        {
            self.state = match r {
                Ok(dir) => State::Done(dir),
                Err(e) => State::Failed(e),
            };
        }

        let body = egui::Rect::from_min_max(title.left_bottom(), full.max).shrink(22.0);
        ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
            ui.vertical_centered(|ui| {
                style::logo(ui, 72.0);
                ui.label(RichText::new("SparkTweak").font(egui::FontId::new(30.0, bold())));
                ui.label(RichText::new(format!("Версия {VERSION} · оптимизация Windows в один клик")).color(DIM));
            });
            ui.add_space(14.0);
            match &self.state {
                State::Ready | State::Failed(_) => {
                    glass().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Папка установки").color(DIM));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button(format!("{}  Обзор", style::ic::DRIVES)).clicked()
                                    && let Some(d) = pick_folder()
                                {
                                    self.dir = target_dir(&d).to_string_lossy().into();
                                }
                            });
                        });
                        ui.add(egui::TextEdit::singleline(&mut self.dir).desired_width(f32::INFINITY).margin(vec2(10.0, 7.0)));
                        let target = target_dir(&self.dir);
                        ui.label(RichText::new(format!("Будет создано: {}", target.join("SparkTweak.exe").display())).size(12.0).color(DIM));
                        ui.add_space(4.0);
                        ui.checkbox(&mut self.desktop, "Ярлык на рабочем столе");
                        ui.checkbox(&mut self.launch, "Запустить SparkTweak после установки");
                    });
                    if !self.admin {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Установка только для вас (без прав администратора)").size(12.5).color(DIM));
                            if ui.small_button(format!("{}  Установить для всех", style::ic::SHIELD)).clicked() && sys::relaunch_as_admin() {
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        });
                    }
                    if let State::Failed(e) = &self.state {
                        glass_accent(RED).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(RichText::new(format!("✗ {e}")).color(RED));
                        });
                    }
                    ui.add_space(10.0);
                    ui.vertical_centered(|ui| {
                        let label = if self.installed { "Переустановить" } else { "Установить" };
                        if style::primary_button(ui, label).clicked() {
                            let (tx, rx) = channel();
                            let (dir, desktop, admin, ctx) = (target_dir(&self.dir), self.desktop, self.admin, ui.ctx().clone());
                            std::thread::spawn(move || {
                                let _ = tx.send(install(dir, desktop, admin));
                                ctx.request_repaint();
                            });
                            self.state = State::Installing(rx);
                        }
                    });
                }
                State::Installing(_) => {
                    glass().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(RichText::new("Устанавливаю файлы, ярлыки и запись в «Установленных приложениях»…").color(MINT));
                        });
                    });
                    ui.ctx().request_repaint();
                }
                State::Done(dir) => {
                    let dir = dir.clone();
                    glass_accent(MINT).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new("✓ SparkTweak установлен").font(egui::FontId::new(18.0, bold())).color(MINT));
                        ui.label(RichText::new(format!("Папка: {}", dir.display())).color(DIM));
                        ui.label(RichText::new("Ярлык — в меню «Пуск». Удалить можно в Параметрах → Приложения.").color(DIM));
                    });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.add_space((ui.available_width() - 330.0).max(0.0) / 2.0);
                        if style::button(ui, style::ic::DRIVES, "Открыть папку").clicked() {
                            let _ = std::process::Command::new("explorer.exe").arg(&dir).spawn();
                        }
                        if style::primary_button(ui, if self.launch { "Запустить и закрыть" } else { "Готово" }).clicked() {
                            if self.launch {
                                let _ = std::process::Command::new(dir.join("SparkTweak.exe")).spawn();
                            }
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                }
            }
        });
    }
}

/// Стандартный диалог выбора папки Windows (через PowerShell, без лишних зависимостей).
fn pick_folder() -> Option<String> {
    let mut out = None;
    sys::run_ps(
        "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.FolderBrowserDialog; $d.Description = 'Куда установить SparkTweak'; if ($d.ShowDialog() -eq 'OK') { '@@DIR ' + $d.SelectedPath }",
        |l| if let Some(d) = l.strip_prefix("@@DIR ") { out = Some(d.to_string()) },
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_dir_always_ends_with_app_folder() {
        assert_eq!(target_dir(r"D:\Games"), PathBuf::from(r"D:\Games\SparkTweak"));
        assert_eq!(target_dir(r" D:\Games\SparkTweak "), PathBuf::from(r"D:\Games\SparkTweak"));
        assert_eq!(target_dir(r#""C:\Program Files\sparktweak""#), PathBuf::from(r"C:\Program Files\sparktweak"));
    }

    #[test]
    fn per_user_install_creates_folder_without_admin() {
        let base = std::env::temp_dir().join("sparktweak_setup_test");
        let _ = std::fs::remove_dir_all(&base);
        let dir = target_dir(&base.to_string_lossy());
        // ярлыки и HKCU-ключ тоже создаются, поэтому подчищаем после теста
        // запущенный SparkTweak установщик закрывает; в тесте это не нужно, но безвредно без прав админа
        let r = install(dir.clone(), false, false);
        assert!(dir.join("SparkTweak.exe").exists(), "{r:?}");
        assert!(r.is_ok(), "{r:?}");
        sys::run_ps(r#"Remove-Item 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\SparkTweak' -Recurse -Force -EA SilentlyContinue; Remove-Item "$([Environment]::GetFolderPath('Programs'))\SparkTweak.lnk" -Force -EA SilentlyContinue"#, |_| {});
        let _ = std::fs::remove_dir_all(&base);
    }
}

fn main() -> eframe::Result {
    if !sys::is_admin() && std::env::var_os("SPARKTWEAK_NOELEVATE").is_none() && sys::relaunch_as_admin() {
        return Ok(());
    }
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Установка SparkTweak")
            .with_inner_size([560.0, 520.0])
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(true)
            .with_icon(style::icon()),
        ..Default::default()
    };
    eframe::run_native("SparkTweak Setup", opts, Box::new(|cc| Ok(Box::new(Setup::new(cc)))))
}
