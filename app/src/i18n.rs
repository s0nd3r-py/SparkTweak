//! Русский / английский. Выбор хранится в %APPDATA%\SparkTweak\lang, по умолчанию — язык Windows.

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

static EN: AtomicBool = AtomicBool::new(false);

pub fn en() -> bool {
    EN.load(Relaxed)
}

/// Строка на текущем языке. Пустой английский вариант — значит перевода нет, показываем русский.
pub fn l<'a>(ru: &'a str, en: &'a str) -> &'a str {
    if self::en() && !en.is_empty() { en } else { ru }
}

/// `lf!("Включено {n}", "Enabled {n}")` — format! на текущем языке.
#[macro_export]
macro_rules! lf {
    ($ru:literal, $en:literal) => {
        if $crate::i18n::en() { format!($en) } else { format!($ru) }
    };
}

fn file() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("APPDATA").unwrap_or_default()).join("SparkTweak").join("lang")
}

fn windows_is_cyrillic() -> bool {
    // русский, украинский, белорусский, казахский
    let lang = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() } & 0x3ff;
    matches!(lang, 0x19 | 0x22 | 0x23 | 0x3f)
}

pub fn init() {
    let en = match std::fs::read_to_string(file()) {
        Ok(s) => s.trim() == "en",
        Err(_) => !windows_is_cyrillic(),
    };
    EN.store(en, Relaxed);
}

pub fn set(en: bool) {
    EN.store(en, Relaxed);
    let f = file();
    if let Some(d) = f.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(f, if en { "en" } else { "ru" });
}
