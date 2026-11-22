//! Каталог всех твиков. Каждый твик — это данные: записи реестра и/или PowerShell.
//! Твик без `revert` считается разовым действием (кнопка «Запустить»).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cat {
    Perf,
    Memory,
    Games,
    Network,
    Privacy,
    PowerShell,
    Interface,
    Services,
    Cleanup,
    Drivers,
    System,
}

use crate::style::ic;

pub const CATS: &[(Cat, &str, &str, &str)] = &[
    (Cat::Perf, ic::ROCKET, "Производительность", "Отклик системы, питание, планировщик и CPU"),
    (Cat::Memory, ic::MEMORY, "Память", "Очистка RAM в реальном времени и снижение потребления"),
    (Cat::Games, ic::GAME, "Игры", "Задержки ввода, FPS, приоритеты и таймеры"),
    (Cat::Network, ic::GLOBE, "Сеть", "Пинг, пропускная способность, DNS"),
    (Cat::Privacy, ic::LOCK, "Приватность", "Телеметрия, реклама, Copilot и Recall"),
    (Cat::PowerShell, ic::TERMINAL, "PowerShell", "Быстрый запуск, телеметрия и безопасность"),
    (Cat::Interface, ic::PALETTE, "Интерфейс", "Explorer, панель задач, меню и анимации"),
    (Cat::Services, ic::GEAR, "Службы", "Отключение лишних служб Windows"),
    (Cat::Cleanup, ic::BROOM, "Очистка", "Временные файлы, кэши, корзина и мусор"),
    (Cat::Drivers, ic::WRENCH, "Драйверы", "Поиск, установка и резервная копия драйверов"),
    (Cat::System, ic::TOOLBOX, "Система", "Восстановление, проверка и обслуживание"),
];

#[derive(Clone, Copy)]
pub enum V {
    D(u32),
    S(&'static str),
}

/// (путь, имя, значение «вкл», значение «выкл» — None = удалить значение)
pub struct Reg(pub &'static str, pub &'static str, pub V, pub Option<V>);

pub struct Tweak {
    pub name: &'static str,
    pub desc: &'static str,
    pub cat: Cat,
    pub regs: &'static [Reg],
    pub apply: &'static str,
    pub revert: &'static str,
    pub check: &'static str,
    /// Входит в «Рекомендуемый» профиль.
    pub rec: bool,
    /// Входит в «Игровой» профиль.
    pub game: bool,
    /// Может что-то сломать у части пользователей.
    pub risk: bool,
    pub reboot: bool,
}

impl Tweak {
    pub fn is_action(&self) -> bool {
        self.regs.is_empty() && self.revert.is_empty()
    }
}

const T: Tweak = Tweak {
    name: "",
    desc: "",
    cat: Cat::Perf,
    regs: &[],
    apply: "",
    revert: "",
    check: "",
    rec: false,
    game: false,
    risk: false,
    reboot: false,
};

use Cat::*;
use V::*;

const MM: &str = r"HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile";
const GAMES: &str = r"HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games";
const ADV: &str = r"HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const CDM: &str = r"HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager";
const DESK: &str = r"HKCU:\Control Panel\Desktop";
const MEMMGMT: &str = r"HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management";
const FS: &str = r"HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem";
const GCS: &str = r"HKCU:\System\GameConfigStore";
const ENV: &str = r"HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
const PERS: &str = r"HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
const SEARCH: &str = r"HKCU:\Software\Microsoft\Windows\CurrentVersion\Search";

pub static TWEAKS: &[Tweak] = &[
    // ───────────── Производительность ─────────────
    Tweak {
        name: "План питания «Максимальная производительность»",
        desc: "Создаёт и включает скрытую схему Ultimate Performance: CPU не сбрасывает частоты, нет задержек выхода из простоя.",
        cat: Perf,
        apply: r#"$o = powercfg -duplicatescheme e9a42b02-d5df-448d-aa00-03f14749eb61; $g = [regex]::Match("$o", '[0-9a-fA-F]{8}-[0-9a-fA-F-]{27}').Value; powercfg -changename $g "SparkTweak Ultimate" | Out-Null; powercfg -setactive $g; "Активна схема $g""#,
        revert: "powercfg -setactive 381b4222-f694-41f0-9685-ff5bb260df2e",
        check: "\"$(powercfg /getactivescheme)\" -match 'SparkTweak'",
        rec: true,
        game: true,
        ..T
    },
    Tweak {
        name: "Отключить Power Throttling",
        desc: "Windows перестаёт искусственно замедлять фоновые процессы ради экономии энергии.",
        cat: Perf,
        regs: &[Reg(r"HKLM:\SYSTEM\CurrentControlSet\Control\Power\PowerThrottling", "PowerThrottlingOff", D(1), None)],
        rec: true,
        game: true,
        reboot: true,
        ..T
    },
    Tweak {
        name: "Приоритет активного окна",
        desc: "Win32PrioritySeparation = 0x26: короткие кванты с усилением для программы на переднем плане.",
        cat: Perf,
        regs: &[Reg(r"HKLM:\SYSTEM\CurrentControlSet\Control\PriorityControl", "Win32PrioritySeparation", D(0x26), Some(D(2)))],
        rec: true,
        game: true,
        ..T
    },
    Tweak {
        name: "Отзывчивость мультимедиа",
        desc: "SystemResponsiveness = 10: фоновым задачам остаётся меньше CPU, переднему плану больше.",
        cat: Perf,
        regs: &[Reg(MM, "SystemResponsiveness", D(10), Some(D(20)))],
        rec: true,
        game: true,
        ..T
    },
    Tweak {
        name: "Аппаратное планирование GPU (HAGS)",
        desc: "Видеокарта сама управляет своей памятью и очередями. Меньше задержка и нагрузка на CPU.",
        cat: Perf,
        regs: &[Reg(r"HKLM:\SYSTEM\CurrentControlSet\Control\GraphicsDrivers", "HwSchMode", D(2), Some(D(1)))],
        game: true,
        reboot: true,
        ..T
    },
    Tweak {
        name: "Мгновенные меню",
        desc: "Убирает задержку 400 мс перед открытием подменю.",
        cat: Perf,
        regs: &[Reg(DESK, "MenuShowDelay", S("0"), Some(S("400")))],
        rec: true,
        ..T
    },
    Tweak {
        name: "Быстрое завершение работы",
        desc: "Зависшие программы и службы закрываются за 2 секунды, а не 5–20.",
        cat: Perf,
        regs: &[
            Reg(r"HKLM:\SYSTEM\CurrentControlSet\Control", "WaitToKillServiceTimeout", S("2000"), Some(S("5000"))),
            Reg(DESK, "WaitToKillAppTimeout", S("2000"), None),
            Reg(DESK, "HungAppTimeout", S("1000"), None),
            Reg(DESK, "AutoEndTasks", S("1"), None),
        ],
        rec: true,
        ..T
    },
    Tweak {
        name: "Без задержки автозагрузки",
        desc: "Программы из автозагрузки стартуют сразу, без искусственной паузы после входа.",
        cat: Perf,
        regs: &[Reg(r"HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Serialize", "StartupDelayInMSec", D(0), None)],
        rec: true,
        ..T
    },
    Tweak {
        name: "Визуальные эффекты на скорость",
        desc: "Отключает анимации окон, панели задач и сворачивания.",
        cat: Perf,
        regs: &[
            Reg(r"HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\VisualEffects", "VisualFXSetting", D(3), Some(D(0))),
            Reg(r"HKCU:\Control Panel\Desktop\WindowMetrics", "MinAnimate", S("0"), Some(S("1"))),
            Reg(ADV, "TaskbarAnimations", D(0), Some(D(1))),
            Reg(r"HKCU:\Software\Microsoft\Windows\DWM", "EnableAeroPeek", D(0), Some(D(1))),
        ],
        ..T
    },
    Tweak {
        name: "Отключить гибернацию",
        desc: "Освобождает на диске C: место размером с объём RAM (hiberfil.sys) и отключает быстрый запуск.",
        cat: Perf,
        apply: "powercfg -h off",
        revert: "powercfg -h on",
        check: r"(Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Power').HibernateEnabled -eq 0",
        ..T
    },
    Tweak {
        name: "Не отключать USB и PCIe для экономии",
        desc: "Отключает Selective Suspend и Link State Power Management. Убирает микрозависания мыши и звука.",
        cat: Perf,
        apply: "powercfg -setacvalueindex SCHEME_CURRENT 2a737441-1930-4402-8d77-b2bebba308a3 48e6b7a6-50f5-4782-a5d4-53bb8f07e226 0; powercfg -setacvalueindex SCHEME_CURRENT 501a4d13-42af-4429-9fd1-a8218c268e20 ee12f906-d277-404b-b6da-e5fa1a576df5 0; powercfg -setactive SCHEME_CURRENT",
        revert: "powercfg -setacvalueindex SCHEME_CURRENT 2a737441-1930-4402-8d77-b2bebba308a3 48e6b7a6-50f5-4782-a5d4-53bb8f07e226 1; powercfg -setacvalueindex SCHEME_CURRENT 501a4d13-42af-4429-9fd1-a8218c268e20 ee12f906-d277-404b-b6da-e5fa1a576df5 1; powercfg -setactive SCHEME_CURRENT",
        check: "(PcAc 2a737441-1930-4402-8d77-b2bebba308a3 48e6b7a6-50f5-4782-a5d4-53bb8f07e226) -eq 0 -and (PcAc 501a4d13-42af-4429-9fd1-a8218c268e20 ee12f906-d277-404b-b6da-e5fa1a576df5) -eq 0",
        game: true,
        ..T
    },
    Tweak {
        name: "Минимальное состояние CPU 100%",
        desc: "Процессор всегда на максимальной частоте от сети. Больше нагрев, зато нет просадок при разгоне частоты.",
        cat: Perf,
        apply: "powercfg -setacvalueindex SCHEME_CURRENT SUB_PROCESSOR PROCTHROTTLEMIN 100; powercfg -setactive SCHEME_CURRENT",
        revert: "powercfg -setacvalueindex SCHEME_CURRENT SUB_PROCESSOR PROCTHROTTLEMIN 5; powercfg -setactive SCHEME_CURRENT",
        check: "(PcAc SUB_PROCESSOR PROCTHROTTLEMIN) -eq 100",
        risk: true,
        ..T
    },
    Tweak {
        name: "Отключить Core Parking",
        desc: "Все ядра CPU остаются активными, без «парковки» и задержек на пробуждение.",
        cat: Perf,
        apply: "powercfg -setacvalueindex SCHEME_CURRENT SUB_PROCESSOR CPMINCORES 100; powercfg -setactive SCHEME_CURRENT",
        revert: "powercfg -setacvalueindex SCHEME_CURRENT SUB_PROCESSOR CPMINCORES 10; powercfg -setactive SCHEME_CURRENT",
        check: "(PcAc SUB_PROCESSOR CPMINCORES) -eq 100",
        game: true,
        ..T
    },
    Tweak {
        name: "NTFS: не обновлять время доступа",
        desc: "Меньше лишних записей на диск при каждом чтении файла.",
        cat: Perf,
        regs: &[Reg(FS, "NtfsDisableLastAccessUpdate", D(0x80000001), Some(D(0x80000002)))],
        rec: true,
        ..T
    },
    Tweak {
        name: "NTFS: без коротких имён 8.3",
        desc: "Ускоряет работу с папками, где много файлов.",
        cat: Perf,
        regs: &[Reg(FS, "NtfsDisable8dot3NameCreation", D(1), Some(D(2)))],
        rec: true,
        ..T
    },
    Tweak {
        name: "Больший кэш файловой системы NTFS",
        desc: "NtfsMemoryUsage = 2: больше памяти под пул метаданных NTFS.",
        cat: Perf,
        regs: &[Reg(FS, "NtfsMemoryUsage", D(2), None)],
        reboot: true,
        ..T
    },
    Tweak {
        name: "Отключить индексацию поиска",
        desc: "Останавливает службу Windows Search. Меньше нагрузка на диск и CPU, но поиск в «Пуске» станет медленнее.",
        cat: Perf,
        apply: "SvcOff WSearch",
        revert: "SvcOn WSearch Automatic; Start-Service WSearch -EA SilentlyContinue",
        check: "SvcIsOff WSearch",
        risk: true,
        ..T
    },
    // ───────────── Память ─────────────
    Tweak {
        name: "Отключить фоновые приложения",
        desc: "UWP-приложения не работают в фоне. В Windows 11 может сломать переключение языка, уведомления и поиск.",
        cat: Memory,
        regs: &[
            Reg(r"HKCU:\Software\Microsoft\Windows\CurrentVersion\BackgroundAccessApplications", "GlobalUserDisabled", D(1), Some(D(0))),
            Reg(SEARCH, "BackgroundAppGlobalToggle", D(0), Some(D(1))),
        ],
        risk: true,
        ..T
    },
    Tweak {
        name: "Отключить SysMain (Superfetch)",
        desc: "На SSD предзагрузка программ в RAM почти не нужна. Освобождает память и снимает нагрузку на диск.",
        cat: Memory,
        apply: "SvcOff SysMain",
        revert: "SvcOn SysMain Automatic; Start-Service SysMain -EA SilentlyContinue",
        check: "SvcIsOff SysMain",
        game: true,
        ..T
    },
    Tweak {
        name: "Отключить сжатие памяти",
        desc: "Убирает нагрузку на CPU от сжатия RAM. Только для ПК с 16 ГБ памяти и больше.",
        cat: Memory,
        apply: "Disable-MMAgent -MemoryCompression",
        revert: "Enable-MMAgent -MemoryCompression",
        check: "(Get-MMAgent).MemoryCompression -eq $false",
        risk: true,
        reboot: true,
        ..T
    },
    Tweak {
        name: "Ядро всегда в RAM",
        desc: "DisablePagingExecutive: драйверы и ядро не уходят в файл подкачки.",
        cat: Memory,
        regs: &[Reg(MEMMGMT, "DisablePagingExecutive", D(1), Some(D(0)))],
        reboot: true,
        ..T
    },
    Tweak {
        name: "Отключить предзагрузку Edge",
        desc: "Edge перестаёт держать процессы в памяти после закрытия и при старте Windows.",
        cat: Memory,
        regs: &[
            Reg(r"HKLM:\SOFTWARE\Policies\Microsoft\Edge", "StartupBoostEnabled", D(0), None),
            Reg(r"HKLM:\SOFTWARE\Policies\Microsoft\Edge", "BackgroundModeEnabled", D(0), None),
        ],
        rec: true,
        ..T
    },
    Tweak {
        name: "Отключить фоновый режим Chrome",
        desc: "Chrome не остаётся в памяти после закрытия последнего окна.",
        cat: Memory,
        regs: &[Reg(r"HKLM:\SOFTWARE\Policies\Google\Chrome", "BackgroundModeEnabled", D(0), None)],
        rec: true,
        ..T
    },
    Tweak {
        name: "Отключить виджеты",
        desc: "Убирает панель «Новости и интересы» и её процесс WebView (~150–300 МБ RAM).",
        cat: Memory,
        regs: &[Reg(r"HKLM:\SOFTWARE\Policies\Microsoft\Dsh", "AllowNewsAndInterests", D(0), None)],
        rec: true,
        ..T
    },
    Tweak {
        name: "Отключить Copilot",
        desc: "Убирает Windows Copilot и его фоновый процесс.",
        cat: Memory,
        regs: &[
            Reg(r"HKCU:\Software\Policies\Microsoft\Windows\WindowsCopilot", "TurnOffWindowsCopilot", D(1), None),
            Reg(r"HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsCopilot", "TurnOffWindowsCopilot", D(1), None),
        ],
        rec: true,
        ..T
    },
    Tweak {
        name: "Отключить Teams в автозагрузке",
        desc: "Встроенный чат Teams не запускается вместе с Windows.",
        cat: Memory,
        regs: &[Reg(r"HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Chat", "ChatIcon", D(3), None)],
        rec: true,
        ..T
    },
    Tweak {
        name: "Разделять svchost реже",
        desc: "Объединяет службы в меньшее число процессов svchost, это экономит RAM. Порог задаётся по объёму памяти.",
        cat: Memory,
        apply: r#"$kb = [math]::Round((Get-CimInstance Win32_PhysicalMemory | Measure-Object Capacity -Sum).Sum / 1KB); Reg 'HKLM:\SYSTEM\CurrentControlSet\Control' 'SvcHostSplitThresholdInKB' $kb; "Порог: $kb КБ""#,
        revert: r"Reg 'HKLM:\SYSTEM\CurrentControlSet\Control' 'SvcHostSplitThresholdInKB' 0x380000",
        check: r"(Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control').SvcHostSplitThresholdInKB -gt 0x380000",
        reboot: true,
        ..T
    },
    // ───────────── Игры ─────────────
    Tweak {
        name: "Приоритет игр в планировщике MMCSS",
        desc: "Задачам «Games» выдаётся высокий приоритет CPU, GPU и ввода-вывода.",
        cat: Games,
        regs: &[
            Reg(GAMES, "GPU Priority", D(8), Some(D(8))),
            Reg(GAMES, "Priority", D(6), Some(D(2))),
            Reg(GAMES, "Scheduling Category", S("High"), Some(S("Medium"))),
            Reg(GAMES, "SFIO Priority", S("High"), Some(S("Normal"))),
        ],
        rec: true,
        game: true,
        ..T
    },
    Tweak {
        name: "Отключить Game DVR и запись в фоне",
        desc: "Xbox Game Bar перестаёт постоянно записывать экран. Это снимает нагрузку с GPU.",
        cat: Games,
        regs: &[
            Reg(GCS, "GameDVR_Enabled", D(0), Some(D(1))),
            Reg(r"HKLM:\SOFTWARE\Policies\Microsoft\Windows\GameDVR", "AllowGameDVR", D(0), None),
            Reg(r"HKCU:\Software\Microsoft\Windows\CurrentVersion\GameDVR", "AppCaptureEnabled", D(0), Some(D(1))),
        ],
        rec: true,
        game: true,
        ..T
    },
    Tweak {
        name: "Игровой режим Windows",
        desc: "Включает Game Mode: игра получает приоритет, обновления не ставятся во время игры.",
        cat: Games,
        regs: &[
            Reg(r"HKCU:\Software\Microsoft\GameBar", "AutoGameModeEnabled", D(1), Some(D(0))),
            Reg(r"HKCU:\Software\Microsoft\GameBar", "AllowAutoGameMode", D(1), Some(D(0))),
        ],
        rec: true,
        game: true,
        ..T
    },
    Tweak {
        name: "Отключить оптимизации полноэкранного режима",
        desc: "Глобально отключает FSO: настоящий эксклюзивный полноэкранный режим и меньше задержка ввода.",
        cat: Games,
        regs: &[
            Reg(GCS, "GameDVR_FSEBehaviorMode", D(2), None),
            Reg(GCS, "GameDVR_FSEBehavior", D(2), None),
            Reg(GCS, "GameDVR_HonorUserFSEBehaviorMode", D(1), None),
            Reg(GCS, "GameDVR_DXGIHonorFSEWindowsCompatible", D(1), None),
        ],
        game: true,
        ..T
    },
    Tweak {
        name: "Отключить ускорение мыши",
        desc: "Убирает «повышенную точность указателя»: движение мыши 1:1, как нужно в шутерах.",
        cat: Games,
        regs: &[
            Reg(r"HKCU:\Control Panel\Mouse", "MouseSpeed", S("0"), Some(S("1"))),
            Reg(r"HKCU:\Control Panel\Mouse", "MouseThreshold1", S("0"), Some(S("6"))),
            Reg(r"HKCU:\Control Panel\Mouse", "MouseThreshold2", S("0"), Some(S("10"))),
        ],
        game: true,
        ..T
    },
    Tweak {
        name: "Быстрый повтор клавиш",
        desc: "Минимальная задержка перед автоповтором нажатой клавиши.",
        cat: Games,
        regs: &[
            Reg(r"HKCU:\Control Panel\Keyboard", "KeyboardDelay", S("0"), Some(S("1"))),
            Reg(r"HKCU:\Control Panel\Keyboard", "KeyboardSpeed", S("31"), Some(S("31"))),
        ],
        game: true,
        ..T
    },
    Tweak {
        name: "Без окна залипания клавиш",
        desc: "Пятикратное нажатие Shift в игре больше не открывает окно залипания клавиш.",
        cat: Games,
        regs: &[
            Reg(r"HKCU:\Control Panel\Accessibility\StickyKeys", "Flags", S("506"), Some(S("510"))),
            Reg(r"HKCU:\Control Panel\Accessibility\ToggleKeys", "Flags", S("58"), Some(S("62"))),
            Reg(r"HKCU:\Control Panel\Accessibility\Keyboard Response", "Flags", S("122"), Some(S("126"))),
        ],
        rec: true,
        game: true,
        ..T
    },
    Tweak {
        name: "Глобальное разрешение таймера",
        desc: "В Windows 11 запрос высокой точности таймера от игры снова действует на всю систему. Стабильнее frametime.",
        cat: Games,
        regs: &[Reg(r"HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\kernel", "GlobalTimerResolutionRequests", D(1), None)],
        game: true,
        reboot: true,
        ..T
    },
    Tweak {
        name: "Отключить dynamic tick",
        desc: "Таймер ядра тикает равномерно. Меньше джиттер, но выше энергопотребление ноутбука.",
        cat: Games,
        apply: "bcdedit /set disabledynamictick yes",
        revert: "bcdedit /deletevalue disabledynamictick",
        check: "\"$(bcdedit /enum '{current}')\" -match 'disabledynamictick'",
        risk: true,
        reboot: true,
        ..T
    },
    Tweak {
        name: "Отключить VBS / изоляцию ядра",
        desc: "Virtualization-Based Security съедает 5–25% FPS. Отключение снижает защиту от эксплойтов ядра.",
        cat: Games,
        regs: &[
            Reg(r"HKLM:\SYSTEM\CurrentControlSet\Control\DeviceGuard", "EnableVirtualizationBasedSecurity", D(0), None),
            Reg(r"HKLM:\SYSTEM\CurrentControlSet\Control\DeviceGuard\Scenarios\HypervisorEnforcedCodeIntegrity", "Enabled", D(0), Some(D(1))),
        ],
        risk: true,
        reboot: true,
        ..T
    },
    Tweak {
        name: "Высокий приоритет для GPU-драйверов (MSI mode)",
        desc: "Переводит видеокарту на прерывания MSI: меньше задержка DPC. Поддерживают почти все GPU последних 10 лет.",
        cat: Games,
        apply: r#"Get-PnpDevice -Class Display -PresentOnly | % { $p = "HKLM:\SYSTEM\CurrentControlSet\Enum\$($_.InstanceId)\Device Parameters\Interrupt Management\MessageSignaledInterruptProperties"; Reg $p 'MSISupported' 1; "MSI: $($_.FriendlyName)" }"#,
        revert: r#"Get-PnpDevice -Class Display -PresentOnly | % { Unreg "HKLM:\SYSTEM\CurrentControlSet\Enum\$($_.InstanceId)\Device Parameters\Interrupt Management\MessageSignaledInterruptProperties" 'MSISupported' }"#,
        check: r#"@(Get-PnpDevice -Class Display -PresentOnly | ? { -not (IsReg "HKLM:\SYSTEM\CurrentControlSet\Enum\$($_.InstanceId)\Device Parameters\Interrupt Management\MessageSignaledInterruptProperties" 'MSISupported' 1) }).Count -eq 0"#,
        risk: true,
        reboot: true,
        ..T
    },
    // ───────────── Сеть ─────────────
    Tweak {
        name: "Снять ограничение сети для мультимедиа",
        desc: "NetworkThrottlingIndex = FFFFFFFF: Windows не режет пропускную способность во время воспроизведения.",
        cat: Network,
        regs: &[Reg(MM, "NetworkThrottlingIndex", D(0xFFFFFFFF), Some(D(10)))],
        rec: true,
        game: true,
        ..T
    },
    Tweak {
        name: "Отключить алгоритм Nagle",
        desc: "TCPNoDelay и TcpAckFrequency на всех адаптерах: пакеты уходят сразу. Ниже пинг в онлайн-играх.",
        cat: Network,
        apply: r"Get-ChildItem 'HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces' | % { Reg $_.PSPath 'TcpAckFrequency' 1; Reg $_.PSPath 'TCPNoDelay' 1 }",
        revert: r"Get-ChildItem 'HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces' | % { Unreg $_.PSPath 'TcpAckFrequency'; Unreg $_.PSPath 'TCPNoDelay' }",
        check: r"@(Get-ChildItem 'HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces' | ? { IsReg $_.PSPath 'TCPNoDelay' 1 }).Count -gt 0",
        game: true,
        ..T
    },
    Tweak {
        name: "Отключить P2P-раздачу обновлений",
        desc: "Delivery Optimization не раздаёт обновления другим ПК через ваш интернет.",
        cat: Network,
        regs: &[Reg(r"HKLM:\SOFTWARE\Policies\Microsoft\Windows\DeliveryOptimization", "DODownloadMode", D(0), None)],
        rec: true,
        ..T
    },
    Tweak {
        name: "Не резервировать 20% канала под QoS",
        desc: "NonBestEffortLimit = 0: весь канал доступен приложениям.",
        cat: Network,
        regs: &[Reg(r"HKLM:\SOFTWARE\Policies\Microsoft\Windows\Psched", "NonBestEffortLimit", D(0), None)],
        rec: true,
        ..T
    },
    Tweak {
        name: "DNS Cloudflare 1.1.1.1",
        desc: "Быстрый DNS на всех активных адаптерах. Откат возвращает DNS от провайдера (DHCP).",
        cat: Network,
        apply: "Get-NetAdapter | ? Status -eq 'Up' | Set-DnsClientServerAddress -ServerAddresses 1.1.1.1,1.0.0.1; Clear-DnsClientCache",
        revert: "Get-NetAdapter | % { Set-DnsClientServerAddress -InterfaceIndex $_.ifIndex -ResetServerAddresses }",
        check: "@(Get-DnsClientServerAddress -AddressFamily IPv4 | ? { $_.ServerAddresses -contains '1.1.1.1' }).Count -gt 0",
        ..T
    },
    Tweak {
        name: "TCP: автонастройка окна и RSS",
        desc: "Окно приёма подстраивается под канал, обработка пакетов распределяется по ядрам.",
        cat: Network,
        apply: "netsh int tcp set global autotuninglevel=normal; netsh int tcp set global rss=enabled; netsh int tcp set global ecncapability=enabled",
        revert: "netsh int tcp set global ecncapability=default",
        check: "\"$(netsh int tcp show global)\" -match 'ECN[^:]*:\\s*enabled'",
        rec: true,
        ..T
    },
    Tweak {
        name: "Больше одновременных соединений",
        desc: "Увеличивает лимиты DNS-кэша и разрешает больше быстрых подключений.",
        cat: Network,
        regs: &[
            Reg(r"HKLM:\SYSTEM\CurrentControlSet\Services\Dnscache\Parameters", "NegativeCacheTime", D(0), None),
            Reg(r"HKLM:\SYSTEM\CurrentControlSet\Services\Dnscache\Parameters", "MaxCacheTtl", D(86400), None),
            Reg(r"HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters", "MaxUserPort", D(65534), None),
            Reg(r"HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters", "TcpTimedWaitDelay", D(30), None),
        ],
        reboot: true,
        ..T
    },
    Tweak {
        name: "Сбросить сетевой стек",
        desc: "Winsock, TCP/IP, DNS-кэш и обновление IP-адреса. Помогает при глюках интернета.",
        cat: Network,
        apply: "netsh winsock reset; netsh int ip reset; ipconfig /flushdns; ipconfig /release | Out-Null; ipconfig /renew | Out-Null; 'Готово. Перезагрузите ПК.'",
        ..T
    },
    // ───────────── Приватность ─────────────
    Tweak {
        name: "Отключить телеметрию",
        desc: "Минимальный уровень диагностики и отключение служб DiagTrack и dmwappushservice.",
        cat: Privacy,
        regs: &[
            Reg(r"HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection", "AllowTelemetry", D(0), None),
            Reg(r"HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection", "DoNotShowFeedbackNotifications", D(1), None),
        ],
        apply: "SvcOff DiagTrack,dmwappushservice",
        revert: "SvcOn DiagTrack Automatic; SvcOn dmwappushservice Manual",
        rec: true,
        ..T
    },
    Tweak {
        name: "Отключить рекламный идентификатор",
        desc: "Приложения не получают ID для персонализированной рекламы.",
        cat: Privacy,
        regs: &[Reg(r"HKCU:\Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo", "Enabled", D(0), Some(D(1)))],
        rec: true,
        ..T
    },
    Tweak {
        name: "Отключить историю активности",
        desc: "Windows не собирает и не отправляет историю открытых программ и файлов.",
        cat: Privacy,
        regs: &[
            Reg(r"HKLM:\SOFTWARE\Policies\Microsoft\Windows\System", "EnableActivityFeed", D(0), None),
            Reg(r"HKLM:\SOFTWARE\Policies\Microsoft\Windows\System", "PublishUserActivities", D(0), None),
            Reg(r"HKLM:\SOFTWARE\Policies\Microsoft\Windows\System", "UploadUserActivities", D(0), None),
        ],
        rec: true,
        ..T
    },
    Tweak {
        name: "Отключить Recall",
        desc: "Запрещает Windows AI делать снимки экрана для «Воспоминаний».",
        cat: Privacy,
        regs: &[
            Reg(r"HKCU:\Software\Policies\Microsoft\Windows\WindowsAI", "DisableAIDataAnalysis", D(1), None),
            Reg(r"HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsAI", "DisableAIDataAnalysis", D(1), None),
        ],
        rec: true,
        ..T
    },
    Tweak {
        name: "Без рекламы и предложений Windows",
        desc: "Убирает рекомендации в «Пуске», тихую установку приложений, советы и «интересное» на экране блокировки.",
        cat: Privacy,
        regs: &[
            Reg(CDM, "SystemPaneSuggestionsEnabled", D(0), Some(D(1))),
            Reg(CDM, "SilentInstalledAppsEnabled", D(0), Some(D(1))),
            Reg(CDM, "SoftLandingEnabled", D(0), Some(D(1))),
            Reg(CDM, "SubscribedContent-338388Enabled", D(0), Some(D(1))),
            Reg(CDM, "SubscribedContent-338389Enabled", D(0), Some(D(1))),
            Reg(CDM, "SubscribedContent-353694Enabled", D(0), Some(D(1))),
            Reg(CDM, "SubscribedContent-353696Enabled", D(0), Some(D(1))),
            Reg(CDM, "RotatingLockScreenOverlayEnabled", D(0), Some(D(1))),
            Reg(r"HKLM:\SOFTWARE\Policies\Microsoft\Windows\CloudContent", "DisableWindowsConsumerFeatures", D(1), None),
        ],
        rec: true,
        ..T
    },
    Tweak {
        name: "Без Bing в поиске «Пуска»",
        desc: "Поиск ищет только на компьютере. Быстрее и без отправки запросов в интернет.",
        cat: Privacy,
        regs: &[
            Reg(r"HKCU:\Software\Policies\Microsoft\Windows\Explorer", "DisableSearchBoxSuggestions", D(1), None),
            Reg(SEARCH, "BingSearchEnabled", D(0), None),
        ],
        rec: true,
        ..T
    },
    Tweak {
        name: "Отключить персонализированный опыт",
        desc: "Microsoft не использует диагностические данные для советов и рекламы.",
        cat: Privacy,
        regs: &[Reg(r"HKCU:\Software\Microsoft\Windows\CurrentVersion\Privacy", "TailoredExperiencesWithDiagnosticDataEnabled", D(0), Some(D(1)))],
        rec: true,
        ..T
    },
    Tweak {
        name: "Отключить запросы отзывов",
        desc: "Windows больше не спрашивает «Как вам Windows?».",
        cat: Privacy,
        regs: &[Reg(r"HKCU:\Software\Microsoft\Siuf\Rules", "NumberOfSIUFInPeriod", D(0), None)],
        rec: true,
        ..T
    },
    Tweak {
        name: "Отключить отчёты об ошибках",
        desc: "Windows Error Reporting не собирает дампы и не отправляет их.",
        cat: Privacy,
        regs: &[Reg(r"HKLM:\SOFTWARE\Microsoft\Windows\Windows Error Reporting", "Disabled", D(1), Some(D(0)))],
        apply: "SvcOff WerSvc",
        revert: "SvcOn WerSvc Manual",
        ..T
    },
    Tweak {
        name: "Отключить Cortana",
        desc: "Запрещает Cortana политикой.",
        cat: Privacy,
        regs: &[Reg(r"HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search", "AllowCortana", D(0), None)],
        rec: true,
        ..T
    },
    Tweak {
        name: "Отключить сбор ввода с клавиатуры",
        desc: "Windows не отправляет образцы рукописного и клавиатурного ввода.",
        cat: Privacy,
        regs: &[
            Reg(r"HKCU:\Software\Microsoft\InputPersonalization", "RestrictImplicitTextCollection", D(1), Some(D(0))),
            Reg(r"HKCU:\Software\Microsoft\InputPersonalization", "RestrictImplicitInkCollection", D(1), Some(D(0))),
            Reg(r"HKCU:\Software\Microsoft\Input\TIPC", "Enabled", D(0), Some(D(1))),
        ],
        rec: true,
        ..T
    },
    Tweak {
        name: "Отключить задания телеметрии",
        desc: "Отключает задания планировщика: Compatibility Appraiser, CEIP, сбор данных о дисках.",
        cat: Privacy,
        apply: r#"'\Microsoft\Windows\Application Experience\Microsoft Compatibility Appraiser','\Microsoft\Windows\Application Experience\ProgramDataUpdater','\Microsoft\Windows\Customer Experience Improvement Program\Consolidator','\Microsoft\Windows\Customer Experience Improvement Program\UsbCeip','\Microsoft\Windows\DiskDiagnostic\Microsoft-Windows-DiskDiagnosticDataCollector' | % { $n = Split-Path $_ -Leaf; $p = (Split-Path $_) + '\'; Disable-ScheduledTask -TaskPath $p -TaskName $n -EA SilentlyContinue | Out-Null; "  − $n" }"#,
        revert: r"'\Microsoft\Windows\Application Experience\Microsoft Compatibility Appraiser','\Microsoft\Windows\Application Experience\ProgramDataUpdater','\Microsoft\Windows\Customer Experience Improvement Program\Consolidator','\Microsoft\Windows\Customer Experience Improvement Program\UsbCeip','\Microsoft\Windows\DiskDiagnostic\Microsoft-Windows-DiskDiagnosticDataCollector' | % { Enable-ScheduledTask -TaskPath ((Split-Path $_) + '\') -TaskName (Split-Path $_ -Leaf) -EA SilentlyContinue | Out-Null }",
        check: r"(Get-ScheduledTask -TaskPath '\Microsoft\Windows\Customer Experience Improvement Program\' -TaskName 'Consolidator' -EA Stop).State -eq 'Disabled'",
        rec: true,
        ..T
    },
    // ───────────── PowerShell ─────────────
    Tweak {
        name: "Ускорить запуск PowerShell (NGen)",
        desc: "Компилирует сборки .NET, которые грузит PowerShell, в машинный код. Запуск становится в 2–5 раз быстрее.",
        cat: PowerShell,
        apply: r#"$ngen = Join-Path ([Runtime.InteropServices.RuntimeEnvironment]::GetRuntimeDirectory()) 'ngen.exe'; [AppDomain]::CurrentDomain.GetAssemblies() | ? { $_.Location } | % { & $ngen install $_.Location /nologo | Out-Null; "  ✓ $(Split-Path $_.Location -Leaf)" }"#,
        rec: true,
        ..T
    },
    Tweak {
        name: "Отключить телеметрию PowerShell",
        desc: "POWERSHELL_TELEMETRY_OPTOUT = 1 для всех версий PowerShell 7.",
        cat: PowerShell,
        regs: &[Reg(ENV, "POWERSHELL_TELEMETRY_OPTOUT", S("1"), None)],
        rec: true,
        ..T
    },
    Tweak {
        name: "Без проверки обновлений PowerShell",
        desc: "PowerShell 7 при запуске не ходит в сеть проверять новую версию.",
        cat: PowerShell,
        regs: &[Reg(ENV, "POWERSHELL_UPDATECHECK", S("Off"), None)],
        rec: true,
        ..T
    },
    Tweak {
        name: "Без телеметрии .NET CLI",
        desc: "DOTNET_CLI_TELEMETRY_OPTOUT = 1.",
        cat: PowerShell,
        regs: &[Reg(ENV, "DOTNET_CLI_TELEMETRY_OPTOUT", S("1"), None)],
        rec: true,
        ..T
    },
    Tweak {
        name: "Разрешить локальные скрипты (RemoteSigned)",
        desc: "Можно запускать свои .ps1. Скрипты из интернета по-прежнему требуют подписи.",
        cat: PowerShell,
        apply: "Set-ExecutionPolicy RemoteSigned -Scope CurrentUser -Force",
        revert: "Set-ExecutionPolicy Undefined -Scope CurrentUser -Force",
        check: "(Get-ExecutionPolicy -Scope CurrentUser) -eq 'RemoteSigned'",
        ..T
    },
    Tweak {
        name: "Отключить PowerShell 2.0",
        desc: "Старый движок без защиты AMSI часто используют для атак. Безопасно удалить.",
        cat: PowerShell,
        apply: "Disable-WindowsOptionalFeature -Online -FeatureName MicrosoftWindowsPowerShellV2Root -NoRestart | Out-Null; 'PowerShell 2.0 отключён'",
        revert: "Enable-WindowsOptionalFeature -Online -FeatureName MicrosoftWindowsPowerShellV2Root -NoRestart | Out-Null",
        check: "(Get-WindowsOptionalFeature -Online -FeatureName MicrosoftWindowsPowerShellV2Root -EA SilentlyContinue).State -ne 'Enabled'",
        rec: true,
        ..T
    },
    Tweak {
        name: "Журнал блоков скриптов",
        desc: "Script Block Logging: все выполненные PowerShell-скрипты пишутся в журнал событий. Полезно для безопасности.",
        cat: PowerShell,
        regs: &[Reg(r"HKLM:\SOFTWARE\Policies\Microsoft\Windows\PowerShell\ScriptBlockLogging", "EnableScriptBlockLogging", D(1), None)],
        ..T
    },
    Tweak {
        name: "Установить/обновить PowerShell 7",
        desc: "Ставит последнюю версию PowerShell 7 через winget.",
        cat: PowerShell,
        apply: "winget install --id Microsoft.PowerShell -e --accept-source-agreements --accept-package-agreements --silent",
        ..T
    },
    Tweak {
        name: "Обновить PSReadLine",
        desc: "Свежий PSReadLine: подсказки по истории, подсветка синтаксиса, быстрее ввод.",
        cat: PowerShell,
        apply: "Install-PackageProvider NuGet -Force -Scope CurrentUser | Out-Null; Install-Module PSReadLine -Force -Scope CurrentUser -AllowClobber -SkipPublisherCheck; 'PSReadLine обновлён'",
        ..T
    },
    // ───────────── Интерфейс ─────────────
    Tweak {
        name: "Классическое контекстное меню",
        desc: "В Windows 11 полное меню по правому клику открывается сразу, без «Показать дополнительные параметры».",
        cat: Interface,
        apply: r"New-Item 'HKCU:\Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32' -Force -Value '' | Out-Null; RestartExplorer",
        revert: r"Remove-Item 'HKCU:\Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}' -Recurse -Force; RestartExplorer",
        check: r"Test-Path 'HKCU:\Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32'",
        rec: true,
        ..T
    },
    Tweak {
        name: "Показывать расширения файлов",
        desc: "Видно, что «фото.jpg.exe» — это программа. Удобно и безопаснее.",
        cat: Interface,
        regs: &[Reg(ADV, "HideFileExt", D(0), Some(D(1)))],
        rec: true,
        ..T
    },
    Tweak {
        name: "Показывать скрытые файлы",
        desc: "Explorer показывает скрытые файлы и папки.",
        cat: Interface,
        regs: &[Reg(ADV, "Hidden", D(1), Some(D(2)))],
        ..T
    },
    Tweak {
        name: "Explorer открывает «Этот компьютер»",
        desc: "Вместо «Главной» с недавними файлами.",
        cat: Interface,
        regs: &[Reg(ADV, "LaunchTo", D(1), None)],
        ..T
    },
    Tweak {
        name: "Тёмная тема системы",
        desc: "Тёмная тема для Windows и приложений.",
        cat: Interface,
        regs: &[Reg(PERS, "AppsUseLightTheme", D(0), Some(D(1))), Reg(PERS, "SystemUsesLightTheme", D(0), Some(D(1)))],
        ..T
    },
    Tweak {
        name: "Отключить прозрачность Windows",
        desc: "Меньше нагрузка на GPU от размытия в системном интерфейсе.",
        cat: Interface,
        regs: &[Reg(PERS, "EnableTransparency", D(0), Some(D(1)))],
        ..T
    },
    Tweak {
        name: "Панель задач слева",
        desc: "Кнопка «Пуск» и значки слева, как в Windows 10.",
        cat: Interface,
        regs: &[Reg(ADV, "TaskbarAl", D(0), Some(D(1)))],
        ..T
    },
    Tweak {
        name: "«Завершить задачу» на панели задач",
        desc: "Правый клик по значку на панели задач → «Завершить задачу». Убить зависшую игру одним кликом.",
        cat: Interface,
        regs: &[Reg(r"HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced\TaskbarDeveloperSettings", "TaskbarEndTask", D(1), Some(D(0)))],
        rec: true,
        ..T
    },
    Tweak {
        name: "Секунды в часах панели задач",
        desc: "Часы в трее показывают секунды.",
        cat: Interface,
        regs: &[Reg(ADV, "ShowSecondsInSystemClock", D(1), Some(D(0)))],
        ..T
    },
    Tweak {
        name: "Убрать кнопку «Представление задач»",
        desc: "Скрывает кнопку Task View на панели задач.",
        cat: Interface,
        regs: &[Reg(ADV, "ShowTaskViewButton", D(0), Some(D(1)))],
        ..T
    },
    Tweak {
        name: "Убрать поиск с панели задач",
        desc: "Скрывает поле поиска. Поиск по-прежнему открывается клавишей Win.",
        cat: Interface,
        regs: &[Reg(SEARCH, "SearchboxTaskbarMode", D(0), Some(D(2)))],
        ..T
    },
    Tweak {
        name: "Отключить ярлык «- Ярлык»",
        desc: "Новые ярлыки создаются без приписки «- Ярлык».",
        cat: Interface,
        apply: r"Reg 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer' 'link' ([byte[]](0,0,0,0)) Binary",
        revert: r"Unreg 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer' 'link'",
        check: r"$v = (Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer' -EA Stop).link; $v -and $v[0] -eq 0",
        ..T
    },
    Tweak {
        name: "Без экрана блокировки",
        desc: "Сразу экран ввода пароля, без картинки, которую надо смахивать.",
        cat: Interface,
        regs: &[Reg(r"HKLM:\SOFTWARE\Policies\Microsoft\Windows\Personalization", "NoLockScreen", D(1), None)],
        ..T
    },
    Tweak {
        name: "Отключить залипание окон (Aero Shake)",
        desc: "Встряхивание окна больше не сворачивает остальные.",
        cat: Interface,
        regs: &[Reg(ADV, "DisallowShaking", D(1), None)],
        ..T
    },
    Tweak {
        name: "Отключить Snap-подсказки",
        desc: "При прилипании окна к краю не появляется предложение разместить остальные окна.",
        cat: Interface,
        regs: &[Reg(ADV, "SnapAssist", D(0), Some(D(1)))],
        ..T
    },
    Tweak {
        name: "Перезапустить Проводник",
        desc: "Применяет изменения интерфейса без перезагрузки.",
        cat: Interface,
        apply: "RestartExplorer",
        ..T
    },
    // ───────────── Службы ─────────────
    Tweak {
        name: "Службы Xbox",
        desc: "Отключает XblAuthManager, XblGameSave, XboxNetApiSvc и XboxGipSvc. Не включайте, если играете через Xbox app или Game Pass.",
        cat: Services,
        apply: "SvcOff XblAuthManager,XblGameSave,XboxNetApiSvc,XboxGipSvc",
        revert: "SvcOn XblAuthManager,XblGameSave,XboxNetApiSvc,XboxGipSvc Manual",
        check: "SvcIsOff XblAuthManager,XblGameSave,XboxNetApiSvc,XboxGipSvc",
        risk: true,
        ..T
    },
    Tweak {
        name: "Диспетчер печати",
        desc: "Отключает Spooler. Включайте, только если у вас нет принтера.",
        cat: Services,
        apply: "SvcOff Spooler",
        revert: "SvcOn Spooler Automatic; Start-Service Spooler -EA SilentlyContinue",
        check: "SvcIsOff Spooler",
        risk: true,
        ..T
    },
    Tweak {
        name: "Факс и удалённый реестр",
        desc: "Fax, RemoteRegistry — почти никому не нужны.",
        cat: Services,
        apply: "SvcOff Fax,RemoteRegistry",
        revert: "SvcOn Fax,RemoteRegistry Manual",
        check: "SvcIsOff Fax,RemoteRegistry",
        rec: true,
        ..T
    },
    Tweak {
        name: "Географическое положение и карты",
        desc: "Отключает lfsvc и MapsBroker (автообновление офлайн-карт).",
        cat: Services,
        apply: "SvcOff lfsvc,MapsBroker",
        revert: "SvcOn lfsvc,MapsBroker Manual",
        check: "SvcIsOff lfsvc,MapsBroker",
        ..T
    },
    Tweak {
        name: "Демо-режим и телефон",
        desc: "RetailDemo, PhoneSvc — демо-режим для магазинов и телефония.",
        cat: Services,
        apply: "SvcOff RetailDemo,PhoneSvc",
        revert: "SvcOn RetailDemo,PhoneSvc Manual",
        check: "SvcIsOff RetailDemo,PhoneSvc",
        rec: true,
        ..T
    },
    Tweak {
        name: "Служба биометрии",
        desc: "WbioSrvc. Не отключайте, если входите по отпечатку пальца или лицу (Windows Hello).",
        cat: Services,
        apply: "SvcOff WbioSrvc",
        revert: "SvcOn WbioSrvc Automatic",
        check: "SvcIsOff WbioSrvc",
        risk: true,
        ..T
    },
    Tweak {
        name: "Беспроводная связь Bluetooth",
        desc: "Отключает bthserv и BTAGService. Не трогайте, если у вас Bluetooth-наушники или мышь.",
        cat: Services,
        apply: "SvcOff bthserv,BTAGService",
        revert: "SvcOn bthserv,BTAGService Manual",
        check: "SvcIsOff bthserv,BTAGService",
        risk: true,
        ..T
    },
    Tweak {
        name: "Hyper-V гостевые службы",
        desc: "Службы интеграции Hyper-V нужны только внутри виртуальной машины.",
        cat: Services,
        apply: "SvcOff vmickvpexchange,vmicguestinterface,vmicshutdown,vmicheartbeat,vmicvmsession,vmicrdv,vmictimesync,vmicvss",
        revert: "SvcOn vmickvpexchange,vmicguestinterface,vmicshutdown,vmicheartbeat,vmicvmsession,vmicrdv,vmictimesync,vmicvss Manual",
        check: "SvcIsOff vmickvpexchange,vmicguestinterface,vmicshutdown,vmicheartbeat,vmicvmsession,vmicrdv,vmictimesync,vmicvss",
        risk: true,
        ..T
    },
    Tweak {
        name: "Не перезагружать ПК после обновлений сам",
        desc: "Windows Update не перезагружает компьютер, пока вы в системе.",
        cat: Services,
        regs: &[
            Reg(r"HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU", "NoAutoRebootWithLoggedOnUsers", D(1), None),
            Reg(r"HKLM:\SOFTWARE\Microsoft\WindowsUpdate\UX\Settings", "IsExpedited", D(0), None),
        ],
        rec: true,
        ..T
    },
    // ───────────── Очистка ─────────────
    Tweak {
        name: "Временные файлы",
        desc: "%TEMP% пользователя и C:\\Windows\\Temp.",
        cat: Cleanup,
        apply: r#"Remove-Item "$env:TEMP\*","$env:windir\Temp\*" -Recurse -Force -EA SilentlyContinue"#,
        ..T
    },
    Tweak {
        name: "Кэш Windows Update",
        desc: "Скачанные установщики обновлений из SoftwareDistribution\\Download.",
        cat: Cleanup,
        apply: r#"Stop-Service wuauserv,bits -Force -EA SilentlyContinue; Remove-Item "$env:windir\SoftwareDistribution\Download\*" -Recurse -Force -EA SilentlyContinue; Start-Service wuauserv,bits -EA SilentlyContinue"#,
        ..T
    },
    Tweak {
        name: "Кэш Delivery Optimization",
        desc: "Файлы, накопленные для P2P-раздачи обновлений.",
        cat: Cleanup,
        apply: "Delete-DeliveryOptimizationCache -Force -EA SilentlyContinue",
        ..T
    },
    Tweak {
        name: "Корзина",
        desc: "Безвозвратно очищает корзину на всех дисках.",
        cat: Cleanup,
        apply: "Clear-RecycleBin -Force -EA SilentlyContinue",
        ..T
    },
    Tweak {
        name: "Кэш миниатюр",
        desc: "thumbcache_*.db. Windows пересоздаст миниатюры по мере просмотра.",
        cat: Cleanup,
        apply: r#"Remove-Item "$env:LOCALAPPDATA\Microsoft\Windows\Explorer\thumbcache_*.db","$env:LOCALAPPDATA\Microsoft\Windows\Explorer\iconcache_*.db" -Force -EA SilentlyContinue"#,
        ..T
    },
    Tweak {
        name: "Дампы сбоев и отчёты об ошибках",
        desc: "CrashDumps, Minidump, WER и memory.dmp.",
        cat: Cleanup,
        apply: r#"Remove-Item "$env:LOCALAPPDATA\CrashDumps\*","$env:ProgramData\Microsoft\Windows\WER\ReportArchive\*","$env:ProgramData\Microsoft\Windows\WER\ReportQueue\*","$env:windir\Minidump\*","$env:windir\MEMORY.DMP" -Recurse -Force -EA SilentlyContinue"#,
        ..T
    },
    Tweak {
        name: "Кэш браузеров",
        desc: "Кэш Chrome, Edge, Brave, Яндекс.Браузера и Firefox. Пароли, история и вкладки не затрагиваются.",
        cat: Cleanup,
        apply: r#"$roots = "$env:LOCALAPPDATA\Google\Chrome\User Data","$env:LOCALAPPDATA\Microsoft\Edge\User Data","$env:LOCALAPPDATA\BraveSoftware\Brave-Browser\User Data","$env:LOCALAPPDATA\Yandex\YandexBrowser\User Data"; foreach ($r in $roots) { Get-ChildItem $r -Directory -EA SilentlyContinue | % { Remove-Item "$($_.FullName)\Cache\*","$($_.FullName)\Code Cache\*","$($_.FullName)\GPUCache\*" -Recurse -Force -EA SilentlyContinue } }; Remove-Item "$env:LOCALAPPDATA\Mozilla\Firefox\Profiles\*\cache2\*" -Recurse -Force -EA SilentlyContinue"#,
        ..T
    },
    Tweak {
        name: "Кэш шейдеров DirectX и видеодрайверов",
        desc: "D3DSCache, NVIDIA DXCache/GLCache, AMD DxCache. Игры пересоберут шейдеры при первом запуске.",
        cat: Cleanup,
        apply: r#"Remove-Item "$env:LOCALAPPDATA\D3DSCache\*","$env:LOCALAPPDATA\NVIDIA\DXCache\*","$env:LOCALAPPDATA\NVIDIA\GLCache\*","$env:LOCALAPPDATA\AMD\DxCache\*","$env:LOCALAPPDATA\AMD\GLCache\*" -Recurse -Force -EA SilentlyContinue"#,
        ..T
    },
    Tweak {
        name: "Кэш DNS",
        desc: "Сбрасывает кэш DNS-имён.",
        cat: Cleanup,
        apply: "Clear-DnsClientCache; 'DNS-кэш очищен'",
        ..T
    },
    Tweak {
        name: "Журналы событий Windows",
        desc: "Очищает все журналы Event Viewer.",
        cat: Cleanup,
        apply: "wevtutil el | % { wevtutil cl \"$_\" 2>$null }; 'Журналы очищены'",
        ..T
    },
    Tweak {
        name: "Старые точки восстановления и теневые копии",
        desc: "Удаляет все теневые копии, кроме самой свежей.",
        cat: Cleanup,
        apply: "vssadmin delete shadows /for=C: /oldest /quiet",
        risk: true,
        ..T
    },
    Tweak {
        name: "Хранилище компонентов WinSxS",
        desc: "DISM удаляет старые версии системных компонентов. Долго (5–15 мин), освобождает гигабайты.",
        cat: Cleanup,
        apply: "Dism.exe /Online /Cleanup-Image /StartComponentCleanup /NoRestart",
        ..T
    },
    // ───────────── Драйверы ─────────────
    Tweak {
        name: "Найти обновления драйверов",
        desc: "Ищет новые драйверы через Windows Update для всех устройств.",
        cat: Drivers,
        apply: r#"'Поиск драйверов в Windows Update…'; $s = New-Object -ComObject Microsoft.Update.Session; $r = $s.CreateUpdateSearcher().Search("IsInstalled=0 and Type='Driver'"); if ($r.Updates.Count -eq 0) { '✓ Все драйверы актуальны' } else { "Найдено обновлений: $($r.Updates.Count)"; foreach ($u in $r.Updates) { "  • $($u.Title)" } }"#,
        ..T
    },
    Tweak {
        name: "Установить все обновления драйверов",
        desc: "Скачивает и ставит все найденные драйверы. Перед этим создаётся точка восстановления.",
        cat: Drivers,
        apply: r#"RestorePoint 'SparkTweak: драйверы'; $s = New-Object -ComObject Microsoft.Update.Session; $r = $s.CreateUpdateSearcher().Search("IsInstalled=0 and Type='Driver'"); if ($r.Updates.Count -eq 0) { '✓ Все драйверы актуальны'; return }; $c = New-Object -ComObject Microsoft.Update.UpdateColl; foreach ($u in $r.Updates) { if (-not $u.EulaAccepted) { $u.AcceptEula() }; [void]$c.Add($u); "  • $($u.Title)" }; 'Загрузка…'; $d = $s.CreateUpdateDownloader(); $d.Updates = $c; [void]$d.Download(); 'Установка…'; $i = $s.CreateUpdateInstaller(); $i.Updates = $c; $res = $i.Install(); "✓ Готово (код $($res.ResultCode))"; if ($res.RebootRequired) { '⟳ Требуется перезагрузка' }"#,
        ..T
    },
    Tweak {
        name: "Резервная копия всех драйверов",
        desc: "Экспортирует сторонние драйверы в папку SparkTweak_Drivers на рабочем столе. Пригодится после переустановки Windows.",
        cat: Drivers,
        apply: r#"$d = Join-Path ([Environment]::GetFolderPath('Desktop')) "SparkTweak_Drivers_$(Get-Date -f yyyy-MM-dd)"; New-Item $d -ItemType Directory -Force | Out-Null; pnputil /export-driver * "$d" | Select-Object -Last 2; "✓ Сохранено в $d""#,
        ..T
    },
    Tweak {
        name: "Список проблемных устройств",
        desc: "Показывает устройства без драйвера или с ошибкой.",
        cat: Drivers,
        apply: r#"$p = Get-PnpDevice -PresentOnly | ? { $_.Status -ne 'OK' }; if (!$p) { '✓ Проблемных устройств нет' } else { $p | % { "  ⚠ $($_.FriendlyName) [$($_.Status)] $($_.InstanceId)" } }"#,
        ..T
    },
    Tweak {
        name: "Обновить видеодрайвер (NVIDIA / AMD / Intel)",
        desc: "Ищет новую версию через winget и открывает официальную страницу драйвера.",
        cat: Drivers,
        apply: r#"$g = (Get-CimInstance Win32_VideoController).Name -join ' '; "GPU: $g"; if ($g -match 'NVIDIA') { Start-Process 'https://www.nvidia.com/ru-ru/drivers/' } elseif ($g -match 'AMD|Radeon') { Start-Process 'https://www.amd.com/ru/support/download/drivers.html' } elseif ($g -match 'Intel') { winget install --id Intel.IntelDriverAndSupportAssistant -e --accept-source-agreements --accept-package-agreements --silent }"#,
        ..T
    },
    Tweak {
        name: "Удалить старые версии драйверов",
        desc: "Убирает из хранилища драйверов устаревшие дубликаты, которые не используются.",
        cat: Drivers,
        apply: r#"$list = pnputil /enum-drivers | Out-String; $blocks = $list -split '(\r?\n){2,}' | ? { $_ -match 'oem\d+\.inf' }; $d = foreach ($b in $blocks) { [pscustomobject]@{ Inf = [regex]::Match($b, 'oem\d+\.inf').Value; Orig = [regex]::Match($b, ':\s*((?!oem\d)\S+\.inf)', 'IgnoreCase').Groups[1].Value; Ver = [regex]::Match($b, '\d{1,4}[./-]\d{1,2}[./-]\d{1,4}\s+(\d+(\.\d+)+)').Groups[1].Value } }; $d | ? Orig | Group-Object Orig | ? Count -gt 1 | % { $_.Group | Sort-Object { try { [version]$_.Ver } catch { [version]'0.0' } } -Descending | Select-Object -Skip 1 | % { pnputil /delete-driver $_.Inf | Out-Null; "  − $($_.Inf) ($($_.Orig) $($_.Ver))" } }; '✓ Готово'"#,
        risk: true,
        ..T
    },
    // ───────────── Система ─────────────
    Tweak {
        name: "Создать точку восстановления",
        desc: "Снимок системы, к которому можно вернуться, если что-то пойдёт не так.",
        cat: System,
        apply: "RestorePoint 'SparkTweak: вручную'",
        ..T
    },
    Tweak {
        name: "Резервная копия реестра",
        desc: "Экспортирует HKCU и HKLM\\SOFTWARE в .reg-файлы на рабочем столе.",
        cat: System,
        apply: r#"$d = Join-Path ([Environment]::GetFolderPath('Desktop')) "SparkTweak_Registry_$(Get-Date -f yyyy-MM-dd_HH-mm)"; New-Item $d -ItemType Directory -Force | Out-Null; reg export HKCU "$d\HKCU.reg" /y | Out-Null; reg export HKLM\SOFTWARE "$d\HKLM_SOFTWARE.reg" /y | Out-Null; reg export HKLM\SYSTEM\CurrentControlSet "$d\HKLM_SYSTEM.reg" /y | Out-Null; "✓ Сохранено в $d""#,
        ..T
    },
    Tweak {
        name: "Проверка системных файлов (SFC)",
        desc: "sfc /scannow — находит и восстанавливает повреждённые файлы Windows.",
        cat: System,
        apply: "sfc /scannow | % { $_ -replace \"`0\", '' } | ? { $_.Trim() }",
        ..T
    },
    Tweak {
        name: "Восстановление образа (DISM)",
        desc: "DISM /RestoreHealth чинит хранилище компонентов. Запускайте перед SFC, если SFC не помогает.",
        cat: System,
        apply: "Dism.exe /Online /Cleanup-Image /RestoreHealth",
        ..T
    },
    Tweak {
        name: "TRIM для всех SSD",
        desc: "Отправляет SSD команду TRIM: восстанавливает скорость записи.",
        cat: System,
        apply: "Get-Volume | ? { $_.DriveLetter -and $_.DriveType -eq 'Fixed' } | % { Optimize-Volume -DriveLetter $_.DriveLetter -ReTrim -EA SilentlyContinue; \"  ✓ $($_.DriveLetter):\" }",
        ..T
    },
    Tweak {
        name: "Проверка диска при следующей загрузке",
        desc: "Планирует chkdsk C: /f при перезагрузке.",
        cat: System,
        apply: "cmd /c 'echo Y| chkdsk C: /f' | Select-Object -Last 2",
        ..T
    },
    Tweak {
        name: "Обновить все программы (winget)",
        desc: "winget upgrade --all: обновляет установленные программы до последних версий.",
        cat: System,
        apply: "winget upgrade --all --accept-source-agreements --accept-package-agreements --silent --include-unknown",
        ..T
    },
    Tweak {
        name: "Пересобрать кэш значков",
        desc: "Чинит пустые или неправильные значки в Проводнике.",
        cat: System,
        apply: r#"ie4uinit.exe -show; Remove-Item "$env:LOCALAPPDATA\IconCache.db" -Force -EA SilentlyContinue; RestartExplorer; '✓ Кэш значков сброшен'"#,
        ..T
    },
    Tweak {
        name: "Отчёт о батарее",
        desc: "Создаёт battery-report.html на рабочем столе: износ и реальная ёмкость (для ноутбуков).",
        cat: System,
        apply: r#"$f = Join-Path ([Environment]::GetFolderPath('Desktop')) 'battery-report.html'; powercfg /batteryreport /output "$f" | Out-Null; if (Test-Path $f) { Start-Process $f; "✓ $f" } else { 'Батарея не найдена' }"#,
        ..T
    },
    Tweak {
        name: "Синхронизировать время",
        desc: "Принудительная синхронизация часов с time.windows.com.",
        cat: System,
        apply: "Start-Service w32time -EA SilentlyContinue; w32tm /resync /force",
        ..T
    },
];

/// Встроенные приложения для модуля «Удаление мусора» (Appx-пакеты).
pub const BLOAT: &[(&str, &str)] = &[
    ("Microsoft.BingNews", "Новости"),
    ("Microsoft.BingWeather", "Погода"),
    ("Microsoft.BingSearch", "Поиск Bing"),
    ("Microsoft.GetHelp", "Техподдержка"),
    ("Microsoft.Getstarted", "Советы"),
    ("Microsoft.MicrosoftSolitaireCollection", "Solitaire Collection"),
    ("Microsoft.MicrosoftOfficeHub", "Microsoft 365 (Office Hub)"),
    ("Microsoft.People", "Люди"),
    ("Microsoft.WindowsFeedbackHub", "Центр отзывов"),
    ("Microsoft.WindowsMaps", "Карты"),
    ("Microsoft.ZuneMusic", "Медиаплеер (Groove)"),
    ("Microsoft.ZuneVideo", "Кино и ТВ"),
    ("Clipchamp.Clipchamp", "Clipchamp"),
    ("Microsoft.Todos", "Microsoft To Do"),
    ("MSTeams", "Teams (персональный)"),
    ("MicrosoftTeams", "Teams (старый)"),
    ("Microsoft.549981C3F5F10", "Cortana"),
    ("Microsoft.PowerAutomateDesktop", "Power Automate"),
    ("Microsoft.OutlookForWindows", "Новый Outlook"),
    ("Microsoft.Copilot", "Copilot"),
    ("Microsoft.Windows.DevHome", "Dev Home"),
    ("Microsoft.MicrosoftStickyNotes", "Записки"),
    ("Microsoft.WindowsAlarms", "Часы и будильник"),
    ("Microsoft.WindowsSoundRecorder", "Звукозапись"),
    ("Microsoft.YourPhone", "Связь с телефоном"),
    ("Microsoft.Microsoft3DViewer", "3D Viewer"),
    ("Microsoft.MixedReality.Portal", "Портал смешанной реальности"),
    ("Microsoft.SkypeApp", "Skype"),
    ("Microsoft.XboxApp", "Xbox Console Companion"),
    ("Microsoft.GamingApp", "Xbox (нужен для Game Pass)"),
    ("Microsoft.XboxGamingOverlay", "Xbox Game Bar"),
    ("king.com.CandyCrushSaga", "Candy Crush Saga"),
    ("king.com.CandyCrushSodaSaga", "Candy Crush Soda"),
    ("SpotifyAB.SpotifyMusic", "Spotify (предустановленный)"),
    ("Disney.37853FC22B2CE", "Disney+"),
    ("Facebook.Facebook", "Facebook"),
    ("BytedancePte.Ltd.TikTok", "TikTok"),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Убить Проводник можно только через RestartExplorer: на ПК с AutoRestartShell=0 он сам не поднимется.
    #[test]
    fn explorer_is_never_killed_without_restart() {
        for t in TWEAKS {
            for s in [t.apply, t.revert] {
                assert!(!s.contains("-Name explorer"), "{}: используйте RestartExplorer", t.name);
            }
        }
    }

    #[test]
    fn every_toggle_can_be_checked_and_reverted() {
        for t in TWEAKS {
            assert!(!t.name.is_empty() && !t.desc.is_empty());
            if t.is_action() {
                assert!(!t.apply.is_empty(), "{}: пустое действие", t.name);
            } else {
                // ps-твику нужна проверка, иначе переключатель всегда «выкл»
                assert!(!t.regs.is_empty() || !t.check.is_empty(), "{}: нет check", t.name);
                assert!(!t.regs.is_empty() || !t.revert.is_empty(), "{}: нет revert", t.name);
            }
        }
    }
}

// ─────────────── Генерация PowerShell ───────────────

fn val(v: V) -> String {
    match v {
        V::D(d) => format!("0x{d:X}"), // в PS 5.1 hex-литерал = int32 с тем же битовым шаблоном
        V::S(s) => format!("{} String", crate::sys::ps_quote(s)),
    }
}

impl Tweak {
    pub fn apply_script(&self) -> String {
        let mut s: String = self
            .regs
            .iter()
            .map(|r| format!("Reg {} {} {}\n", crate::sys::ps_quote(r.0), crate::sys::ps_quote(r.1), val(r.2)))
            .collect();
        s += self.apply;
        s
    }

    pub fn revert_script(&self) -> String {
        let mut s: String = self
            .regs
            .iter()
            .map(|r| match r.3 {
                Some(v) => format!("Reg {} {} {}\n", crate::sys::ps_quote(r.0), crate::sys::ps_quote(r.1), val(v)),
                None => format!("Unreg {} {}\n", crate::sys::ps_quote(r.0), crate::sys::ps_quote(r.1)),
            })
            .collect();
        s += self.revert;
        s
    }

    /// PowerShell-выражение: включён ли твик сейчас.
    pub fn check_expr(&self) -> String {
        if !self.check.is_empty() {
            return self.check.to_string();
        }
        let parts: Vec<String> = self
            .regs
            .iter()
            .map(|r| {
                let v = match r.2 {
                    V::D(d) => format!("0x{d:X}"),
                    V::S(s) => crate::sys::ps_quote(s),
                };
                format!("(IsReg {} {} {v})", crate::sys::ps_quote(r.0), crate::sys::ps_quote(r.1))
            })
            .collect();
        parts.join(" -and ")
    }

    /// Строка, печатающая `@@STATE <i> <0|1>` для разбора в интерфейсе.
    pub fn state_line(&self, i: usize) -> String {
        format!("try {{ if ($({})) {{ '@@STATE {i} 1' }} else {{ '@@STATE {i} 0' }} }} catch {{ '@@STATE {i} 0' }}", self.check_expr())
    }
}

#[cfg(test)]
mod gen_tests {
    use super::*;

    #[test]
    fn reg_script_shapes() {
        let t = TWEAKS.iter().find(|t| t.name == "Мгновенные меню").unwrap();
        assert_eq!(t.apply_script().trim_end(), r"Reg 'HKCU:\Control Panel\Desktop' 'MenuShowDelay' '0' String");
        assert_eq!(t.revert_script().trim_end(), r"Reg 'HKCU:\Control Panel\Desktop' 'MenuShowDelay' '400' String");
        assert!(t.check_expr().contains(r"IsReg 'HKCU:\Control Panel\Desktop' 'MenuShowDelay' '0'"));
    }

    /// Реальный прогон всех проверок в PowerShell: ни одна не должна падать с синтаксической ошибкой.
    #[test]
    fn all_checks_run_in_powershell() {
        let script: String = TWEAKS.iter().enumerate().filter(|(_, t)| !t.is_action()).map(|(i, t)| t.state_line(i) + "\n").collect();
        let mut states = 0;
        let mut junk = vec![];
        crate::sys::run_ps(&script, |l| if l.starts_with("@@STATE") { states += 1 } else { junk.push(l) });
        let toggles = TWEAKS.iter().filter(|t| !t.is_action()).count();
        assert_eq!(states, toggles, "лишний вывод: {junk:?}");
    }
}

#[cfg(test)]
mod parse_tests {
    use super::*;

    /// Каждый apply/revert-скрипт должен разбираться парсером PowerShell (без выполнения).
    #[test]
    fn all_scripts_parse() {
        let dir = std::env::temp_dir().join("SparkTweak_parse");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (i, t) in TWEAKS.iter().enumerate() {
            std::fs::write(dir.join(format!("{i}_apply.ps1")), format!("\u{feff}{}", t.apply_script())).unwrap();
            std::fs::write(dir.join(format!("{i}_revert.ps1")), format!("\u{feff}{}", t.revert_script())).unwrap();
        }
        let mut errs = vec![];
        let script = format!(
            "Get-ChildItem {} -Filter *.ps1 | % {{ $e = $null; [void][Management.Automation.Language.Parser]::ParseFile($_.FullName, [ref]$null, [ref]$e); if ($e) {{ \"$($_.Name): $($e[0].Message)\" }} }}",
            crate::sys::ps_quote(&dir.to_string_lossy())
        );
        crate::sys::run_ps(&script, |l| errs.push(l));
        let _ = std::fs::remove_dir_all(&dir);
        let named: Vec<String> = errs.iter().map(|e| {
            let i: usize = e.split('_').next().unwrap_or("0").parse().unwrap_or(0);
            format!("[{}] {e}", TWEAKS[i].name)
        }).collect();
        assert!(errs.is_empty(), "{named:#?}");
    }
}
