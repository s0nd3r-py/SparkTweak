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
    Installing(Receiver<Result<(), String>>),
    Done,
    Failed(String),
}

struct Setup {
    dir: String,
    desktop: bool,
    launch: bool,
    state: State,
    installed: bool,
}

fn default_dir() -> String {
    let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into());
    format!(r"{pf}\SparkTweak")
}

fn install(dir: PathBuf, desktop: bool) -> Result<(), String> {
    // запущенную копию надо закрыть, иначе exe заблокирован
    sys::run_ps("Stop-Process -Name sparktweak -Force -EA SilentlyContinue; Start-Sleep -Milliseconds 400", |_| {});
    std::fs::create_dir_all(&dir).map_err(|e| format!("Не удалось создать папку: {e}"))?;
    let exe = dir.join("SparkTweak.exe");
    std::fs::write(&exe, APP_EXE).map_err(|e| format!("Не удалось записать файл: {e}"))?;
    let exe_s = exe.to_string_lossy();
    let q = |s: &str| sys::ps_quote(s);
    let script = format!(
        r#"$exe = {exe}
$ws = New-Object -ComObject WScript.Shell
$links = @("$([Environment]::GetFolderPath('CommonPrograms'))\SparkTweak.lnk")
if ({desktop}) {{ $links += "$([Environment]::GetFolderPath('CommonDesktopDirectory'))\SparkTweak.lnk" }}
foreach ($l in $links) {{ $s = $ws.CreateShortcut($l); $s.TargetPath = $exe; $s.WorkingDirectory = {dir}; $s.Description = 'SparkTweak — оптимизация Windows'; $s.Save() }}
$k = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\SparkTweak'
New-Item $k -Force | Out-Null
$v = @{{ DisplayName = 'SparkTweak'; DisplayVersion = {ver}; Publisher = 'SparkTweak'; DisplayIcon = $exe; InstallLocation = {dir}; UninstallString = "`"$exe`" --uninstall"; URLInfoAbout = 'https://github.com/s0nd3r-py/SparkTweak' }}
foreach ($n in $v.Keys) {{ Set-ItemProperty $k -Name $n -Value $v[$n] }}
Set-ItemProperty $k -Name NoModify -Value 1 -Type DWord
Set-ItemProperty $k -Name NoRepair -Value 1 -Type DWord
Set-ItemProperty $k -Name EstimatedSize -Value {kb} -Type DWord
'OK'"#,
        exe = q(&exe_s),
        dir = q(&dir.to_string_lossy()),
        desktop = if desktop { "$true" } else { "$false" },
        ver = q(VERSION),
        kb = APP_EXE.len() / 1024,
    );
    let mut out = vec![];
    sys::run_ps(&script, |l| out.push(l));
    if out.iter().any(|l| l == "OK") { Ok(()) } else { Err(out.join("\n")) }
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
        let dir = default_dir();
        let installed = PathBuf::from(&dir).join("SparkTweak.exe").exists();
        Self { dir, desktop: true, launch: true, state: State::Ready, installed }
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
                Ok(()) => State::Done,
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
                        ui.label(RichText::new("Папка установки").color(DIM));
                        ui.add(egui::TextEdit::singleline(&mut self.dir).desired_width(f32::INFINITY).margin(vec2(10.0, 7.0)));
                        ui.add_space(4.0);
                        ui.checkbox(&mut self.desktop, "Ярлык на рабочем столе");
                        ui.checkbox(&mut self.launch, "Запустить SparkTweak после установки");
                    });
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
                            let (dir, desktop, ctx) = (PathBuf::from(self.dir.trim()), self.desktop, ui.ctx().clone());
                            std::thread::spawn(move || {
                                let _ = tx.send(install(dir, desktop));
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
                State::Done => {
                    glass_accent(MINT).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new("✓ SparkTweak установлен").font(egui::FontId::new(18.0, bold())).color(MINT));
                        ui.label(RichText::new("Ярлык — в меню «Пуск». Удалить можно в Параметрах → Приложения.").color(DIM));
                    });
                    ui.add_space(10.0);
                    ui.vertical_centered(|ui| {
                        if style::primary_button(ui, if self.launch { "Запустить и закрыть" } else { "Готово" }).clicked() {
                            if self.launch {
                                let _ = std::process::Command::new(PathBuf::from(self.dir.trim()).join("SparkTweak.exe")).spawn();
                            }
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                }
            }
        });
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
