#![windows_subsystem = "windows"]

mod style;
mod sys;
mod tweaks;
mod ui;

fn uninstall() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{IDYES, MB_ICONQUESTION, MB_YESNO, MessageBoxW};
    let (t, c) = (sys::wide("Удалить SparkTweak с компьютера?\n\nПрименённые твики останутся. Откатить их можно заранее кнопкой «Откатить все твики»."), sys::wide("SparkTweak"));
    if unsafe { MessageBoxW(std::ptr::null_mut(), t.as_ptr(), c.as_ptr(), MB_YESNO | MB_ICONQUESTION) } != IDYES {
        return;
    }
    let exe = std::env::current_exe().unwrap_or_default();
    let dir = exe.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let script = format!(
        r#"Remove-Item "$([Environment]::GetFolderPath('CommonPrograms'))\SparkTweak.lnk","$([Environment]::GetFolderPath('CommonDesktopDirectory'))\SparkTweak.lnk" -Force -EA SilentlyContinue
Remove-Item 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\SparkTweak' -Recurse -Force -EA SilentlyContinue
Start-Process cmd.exe -ArgumentList '/c timeout /t 2 /nobreak >nul & rmdir /s /q "{dir}"' -WindowStyle Hidden"#
    );
    sys::run_ps(&script, |_| {});
    sys::message_box("SparkTweak удалён.", "SparkTweak");
}

fn main() -> eframe::Result {
    if !sys::is_admin() && std::env::var_os("SPARKTWEAK_NOELEVATE").is_none() && sys::relaunch_as_admin() {
        return Ok(());
    }
    if std::env::args().any(|a| a == "--uninstall") {
        uninstall();
        return Ok(());
    }
    let opts = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("SparkTweak")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([1040.0, 660.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_icon(style::icon()),
        ..Default::default()
    };
    eframe::run_native("SparkTweak", opts, Box::new(|cc| Ok(Box::new(ui::App::new(cc)))))
}
