use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use eframe::egui::{self, Align, Color32, FontId, Layout, Rect, RichText, ScrollArea, Sense, Stroke, Ui, pos2, vec2};

use crate::i18n::{self, l};
use crate::lf;
use crate::style::{self, AMBER, CYAN, DIM, FAINT, MINT, RED, TEXT, VIOLET, bold, glass, glass_accent, ic, white};
use crate::sys::{self, MemCmd};
use crate::tweaks::{BLOAT, CATS, Cat, TWEAKS, Tweak, cat_info};

enum Job {
    Ps { title: String, script: String },
    Mem(Vec<MemCmd>),
}

enum Msg {
    Line(String),
    Done,
}

#[derive(PartialEq, Clone, Copy)]
enum Page {
    Home,
    Cat(Cat),
    Startup,
    Debloat,
    About,
}

struct StartItem {
    key: String,
    name: String,
    cmd: String,
    on: bool,
}

pub struct App {
    tx: Sender<Job>,
    /// Отдельная очередь для чтения состояния, чтобы долгие проверки не блокировали действия.
    tx_read: Sender<Job>,
    rx: Receiver<Msg>,
    busy: usize,
    states: Vec<Option<bool>>,
    pending: HashSet<usize>,
    log: Vec<String>,
    log_open: bool,
    page: Page,
    search: String,
    info: HashMap<String, String>,
    mem: sys::Mem,
    mem_at: Instant,
    auto_clean: bool,
    auto_every: u64,
    auto_at: Instant,
    startup: Vec<StartItem>,
    startup_loaded: bool,
    appx: Vec<Option<bool>>,
    appx_sel: Vec<bool>,
    appx_loaded: bool,
    admin: bool,
    solid: bool,
    pc: String,
}

fn run_job(job: Job, out: &Sender<Msg>, ctx: &egui::Context) {
    let send = |s: String| {
        let _ = out.send(Msg::Line(s));
        ctx.request_repaint();
    };
    match job {
        Job::Ps { title, script } => {
            if !title.is_empty() {
                send(format!("→ {title}"));
            }
            // код выхода PowerShell ненадёжен (его портят нативные команды), ошибки и так видны в журнале
            sys::run_ps(&script, &send);
        }
        Job::Mem(cmds) => {
            send(format!("→ {}", l("Очистка оперативной памяти", "Cleaning RAM")));
            let before = sys::mem_status().avail;
            for c in cmds {
                let label = c.label();
                match sys::mem_purge(c) {
                    Ok(()) => send(format!("  ✓ {label}")),
                    Err(st) => {
                        let st = st as u32;
                        send(lf!("  ✗ {label} (NTSTATUS 0x{st:08X}, нужны права администратора)", "  ✗ {label} (NTSTATUS 0x{st:08X}, administrator rights required)"))
                    }
                }
            }
            let mb = sys::mem_status().avail.saturating_sub(before) / 1024 / 1024;
            send(lf!("✓ Освобождено {mb} МБ", "✓ Freed {mb} MB"));
        }
    }
    let _ = out.send(Msg::Done);
    ctx.request_repaint();
}

/// Изменения системы выполняются строго по очереди.
fn worker(jobs: Receiver<Job>, out: Sender<Msg>, ctx: egui::Context) {
    for job in jobs {
        run_job(job, &out, &ctx);
    }
}

/// Чтение состояния ничего не меняет, поэтому каждая задача идёт сразу в своём потоке.
fn reader(jobs: Receiver<Job>, out: Sender<Msg>, ctx: egui::Context) {
    for job in jobs {
        let (out, ctx) = (out.clone(), ctx.clone());
        std::thread::spawn(move || run_job(job, &out, &ctx));
    }
}

const INFO_SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
"@@INFO cpu " + (Get-ItemProperty 'HKLM:\HARDWARE\DESCRIPTION\System\CentralProcessor\0').ProcessorNameString.Trim()
"@@INFO gpu " + ((Get-CimInstance Win32_VideoController | % Name) -join ' + ')
$o = Get-CimInstance Win32_OperatingSystem; "@@INFO os $($o.Caption -replace '^.*?(Windows)', '$1') · $(T 'сборка' 'build') $($o.BuildNumber)"
$d = Get-PSDrive C; "@@INFO disk " + ((T '{0:N0} ГБ свободно из {1:N0} ГБ' '{0:N0} GB free of {1:N0} GB') -f ($d.Free / 1GB), (($d.Used + $d.Free) / 1GB))
"@@INFO board " + (Get-CimInstance Win32_BaseBoard | % { "$($_.Manufacturer) $($_.Product)" })
"@@INFO power " + ("$(powercfg /getactivescheme)" -replace '^.*\(|\)\s*$', '')
"#;

const STARTUP_SCRIPT: &str = r#"
'@@RUNCLEAR'
$sa = 'Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved'
function Approved($k, $n) { $a = (Get-ItemProperty $k -Name $n -EA SilentlyContinue).$n; [int](-not ($a -and ($a[0] -band 1))) }
foreach ($s in @(
  @('HKCU:\Software\Microsoft\Windows\CurrentVersion\Run', "HKCU:\$sa\Run"),
  @('HKLM:\Software\Microsoft\Windows\CurrentVersion\Run', "HKLM:\$sa\Run"),
  @('HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run', "HKLM:\$sa\Run32"))) {
  $k = Get-Item $s[0] -EA SilentlyContinue; if (!$k) { continue }
  foreach ($n in $k.GetValueNames()) { if ($n) { "@@RUN $($s[1])`t$n`t$($k.GetValue($n))`t$(Approved $s[1] $n)" } }
}
foreach ($d in @(@([Environment]::GetFolderPath('Startup'), "HKCU:\$sa\StartupFolder"), @([Environment]::GetFolderPath('CommonStartup'), "HKLM:\$sa\StartupFolder"))) {
  Get-ChildItem $d[0] -File -EA SilentlyContinue | ? Name -ne 'desktop.ini' | % { "@@RUN $($d[1])`t$($_.Name)`t$($_.FullName)`t$(Approved $d[1] $_.Name)" }
}
"#;

const GB: f64 = 1073741824.0;

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        style::setup(&cc.egui_ctx);
        let solid = std::env::var_os("SPARKTWEAK_SOLID").is_some();
        if !solid {
            let _ = window_vibrancy::apply_acrylic(cc, Some((28, 30, 33, 110)));
        }
        if let Ok(h) = raw_window_handle::HasWindowHandle::window_handle(cc)
            && let raw_window_handle::RawWindowHandle::Win32(w) = h.as_raw()
        {
            sys::round_corners(w.hwnd.get());
        }
        let (tx, jobs) = channel();
        let (tx_read, read_jobs) = channel();
        let (out, rx) = channel();
        let (ctx, ctx2, out2) = (cc.egui_ctx.clone(), cc.egui_ctx.clone(), out.clone());
        std::thread::spawn(move || worker(jobs, out, ctx));
        std::thread::spawn(move || reader(read_jobs, out2, ctx2));
        let mut app = Self {
            tx,
            tx_read,
            rx,
            busy: 0,
            states: vec![None; TWEAKS.len()],
            pending: HashSet::new(),
            log: vec![{
                let v = env!("CARGO_PKG_VERSION");
                lf!("SparkTweak v{v} готов к работе", "SparkTweak v{v} is ready")
            }],
            log_open: false,
            page: Page::Home,
            search: String::new(),
            info: HashMap::new(),
            mem: sys::mem_status(),
            mem_at: Instant::now(),
            auto_clean: false,
            auto_every: 10,
            auto_at: Instant::now(),
            startup: vec![],
            startup_loaded: false,
            appx: vec![None; BLOAT.len()],
            appx_sel: vec![false; BLOAT.len()],
            appx_loaded: false,
            admin: sys::is_admin(),
            solid,
            pc: std::env::var("COMPUTERNAME").unwrap_or_default(),
        };
        // режим разработки: сразу открыть страницу (для скриншотов без кликов)
        if let Ok(p) = std::env::var("SPARKTWEAK_PAGE") {
            app.page = match p.as_str() {
                "startup" => Page::Startup,
                "debloat" => Page::Debloat,
                "about" => Page::About,
                n => n.parse::<usize>().ok().and_then(|i| CATS.get(i)).map_or(Page::Home, |c| Page::Cat(c.0)),
            };
        }
        app.read_job("", INFO_SCRIPT.into());
        app.refresh_states();
        app
    }

    // ─────────────── очередь задач ───────────────

    fn job(&mut self, title: &str, script: String) {
        self.busy += 1;
        let _ = self.tx.send(Job::Ps { title: title.into(), script });
    }

    fn read_job(&mut self, title: &str, script: String) {
        self.busy += 1;
        let _ = self.tx_read.send(Job::Ps { title: title.into(), script });
    }

    fn mem_job(&mut self, cmds: Vec<MemCmd>) {
        self.busy += 1;
        let _ = self.tx.send(Job::Mem(cmds));
    }

    fn refresh_states(&mut self) {
        // ошибки проверок (например, нет прав) не нужны в журнале: непроверенный твик просто «выключен»
        let mut s = String::from("$ErrorActionPreference = 'SilentlyContinue'\n");
        for (i, t) in TWEAKS.iter().enumerate().filter(|(_, t)| !t.is_action()) {
            s += &(t.state_line(i) + "\n");
        }
        self.read_job(l("Проверка состояния твиков", "Checking tweak states"), s);
    }

    fn set_tweak(&mut self, i: usize, on: bool) {
        let t = &TWEAKS[i];
        let body = if on { t.apply_script() } else { t.revert_script() };
        let verb = if on { l("Включение", "Enabling") } else { l("Откат", "Reverting") };
        self.pending.insert(i);
        self.job(&format!("{verb}: {}", t.title()), format!("{body}\n{}", t.state_line(i)));
    }

    fn run_action(&mut self, i: usize) {
        let t = &TWEAKS[i];
        let body = if t.cat == Cat::Cleanup { with_freed(t.apply) } else { t.apply.to_string() };
        self.job(t.title(), body);
    }

    /// Применяет (или откатывает) набор твиков одним скриптом. Перед применением — точка восстановления.
    fn bulk(&mut self, title: &str, ids: Vec<usize>, on: bool) {
        if ids.is_empty() {
            self.log.push(lf!("✓ {title}: всё уже сделано", "✓ {title}: nothing left to do"));
            return;
        }
        let mut s = if on { "RestorePoint 'SparkTweak'\n".to_string() } else { String::new() };
        for &i in &ids {
            let t = &TWEAKS[i];
            self.pending.insert(i);
            let body = if on { t.apply_script() } else { t.revert_script() };
            s += &format!("'• {}'\n& {{\n{body}\n}}\n{}\n", t.title().replace('\'', "''"), t.state_line(i));
        }
        if ids.iter().any(|&i| TWEAKS[i].reboot) {
            s += "T '⟳ Часть изменений вступит в силу после перезагрузки' '⟳ Some changes take effect after a restart'\n";
        }
        let n = ids.len();
        self.job(&lf!("{title} ({n} твиков)", "{title} ({n} tweaks)"), s);
    }

    fn profile_ids(&self, f: impl Fn(&Tweak) -> bool) -> Vec<usize> {
        (0..TWEAKS.len()).filter(|&i| !TWEAKS[i].is_action() && f(&TWEAKS[i]) && self.states[i] != Some(true)).collect()
    }

    fn clean_all(&mut self) {
        let body: String = TWEAKS
            .iter()
            .filter(|t| t.cat == Cat::Cleanup && !t.risk && !t.name.contains("WinSxS"))
            .map(|t| format!("'• {}'\n{}\n", t.title().replace('\'', "''"), t.apply))
            .collect();
        self.job(l("Очистка мусора", "Junk cleanup"), with_freed(&body));
    }

    fn handle(&mut self, line: String) {
        let Some(rest) = line.strip_prefix("@@") else {
            self.log.push(line);
            if self.log.len() > 3000 {
                self.log.drain(..1000);
            }
            return;
        };
        let (tag, val) = rest.split_once(' ').unwrap_or((rest, ""));
        match tag {
            "STATE" => {
                if let Some((i, v)) = val.split_once(' ')
                    && let Ok(i) = i.parse::<usize>()
                    && i < self.states.len()
                {
                    self.states[i] = Some(v == "1");
                    self.pending.remove(&i);
                }
            }
            "INFO" => {
                if let Some((k, v)) = val.split_once(' ') {
                    self.info.insert(k.into(), v.trim().into());
                }
            }
            "RUNCLEAR" => self.startup.clear(),
            "RUN" => {
                if let [key, name, cmd, on] = val.splitn(4, '\t').collect::<Vec<_>>()[..] {
                    self.startup.push(StartItem { key: key.into(), name: name.into(), cmd: cmd.into(), on: on == "1" });
                }
            }
            "APPX" => {
                if let Some((i, v)) = val.split_once(' ')
                    && let Ok(i) = i.parse::<usize>()
                    && i < self.appx.len()
                {
                    self.appx[i] = Some(v == "1");
                }
            }
            _ => self.log.push(line),
        }
    }

    fn applied(&self) -> (usize, usize) {
        let toggles = TWEAKS.iter().filter(|t| !t.is_action()).count();
        (self.states.iter().filter(|s| **s == Some(true)).count(), toggles)
    }

    fn go(&mut self, p: Page) {
        self.page = p;
        self.search.clear();
    }

    // ─────────────── каркас ───────────────

    /// Узкая панель из иконок, как в Nitro. Шаг иконок ужимается, чтобы всё влезало в низкое окно.
    fn rail(&mut self, ui: &mut Ui, rect: Rect) {
        let p = ui.painter();
        p.rect_filled(rect, 26.0, Color32::from_black_alpha(55));
        p.rect_stroke(rect, 26.0, Stroke::new(1.0, white(24)), egui::StrokeKind::Inside);
        let cx = rect.center().x;
        let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));

        let items = 1 + CATS.len() + 3;
        let free = rect.height() - 14.0 - 48.0 - 14.0 - 2.0 * 12.0 - 12.0;
        let step = (free / items as f32).clamp(26.0, 43.0);
        let size = (step - 4.0).clamp(24.0, 40.0);

        let mut y = rect.top() + 14.0;
        let logo_r = Rect::from_center_size(pos2(cx, y + 24.0), vec2(48.0, 48.0));
        let lr = ui.scope_builder(egui::UiBuilder::new().max_rect(logo_r), |ui| style::logo(ui, 48.0)).inner;
        if lr.on_hover_text("SparkTweak").clicked() {
            self.go(Page::Home);
        }
        y += 48.0 + 14.0;

        let mut item = |ui: &mut Ui, y: &mut f32, page: Page, icon: &str, tip: &str, dot: bool| {
            let r = Rect::from_center_size(pos2(cx, *y + step / 2.0), vec2(size, size));
            let resp = ui.interact(r, ui.scope_id().with(tip), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            let active = self.page == page && self.search.is_empty();
            let p = ui.painter();
            if active {
                p.rect_filled(r, size * 0.32, white(30));
                p.rect_stroke(r, size * 0.32, Stroke::new(1.0, white(50)), egui::StrokeKind::Inside);
            } else if resp.hovered() {
                p.rect_filled(r, size * 0.32, white(14));
            }
            p.text(r.center(), egui::Align2::CENTER_CENTER, icon, FontId::proportional(size * 0.48), if active { TEXT } else { DIM });
            if dot {
                let c = r.right_bottom() - vec2(size * 0.17, size * 0.17);
                p.circle_filled(c, 4.5, Color32::from_rgb(30, 32, 35));
                p.circle_filled(c, 3.0, MINT);
            }
            if resp.on_hover_text(tip).clicked() {
                self.page = page;
                self.search.clear();
            }
            *y += step;
        };
        let sep = |ui: &mut Ui, y: &mut f32| {
            *y += 6.0;
            ui.painter().hline((cx - 14.0)..=(cx + 14.0), *y, Stroke::new(1.0, white(30)));
            *y += 6.0;
        };
        item(&mut ui, &mut y, Page::Home, ic::GRID, l("Обзор", "Overview"), false);
        sep(&mut ui, &mut y);
        for &(cat, ..) in CATS {
            let (icon, name, _) = cat_info(cat);
            let any_on = TWEAKS.iter().enumerate().any(|(i, t)| t.cat == cat && self.states[i] == Some(true));
            item(&mut ui, &mut y, Page::Cat(cat), icon, name, any_on);
        }
        sep(&mut ui, &mut y);
        item(&mut ui, &mut y, Page::Startup, ic::POWER, l("Автозагрузка", "Startup apps"), false);
        item(&mut ui, &mut y, Page::Debloat, ic::TRASH, l("Удаление приложений", "Remove apps"), false);
        item(&mut ui, &mut y, Page::About, ic::INFO, l("О программе", "About"), false);
    }

    /// Переключатель языка в заголовке окна.
    fn lang_switch(&mut self, ui: &mut Ui, title: Rect) {
        let w = 74.0;
        let r = Rect::from_center_size(pos2(title.right() - 14.0 - w / 2.0, title.center().y), vec2(w, 22.0));
        let p = ui.painter();
        p.rect_filled(r, 11.0, white(10));
        p.rect_stroke(r, 11.0, Stroke::new(1.0, white(26)), egui::StrokeKind::Inside);
        for (k, (label, en)) in [("RU", false), ("EN", true)].into_iter().enumerate() {
            let seg = Rect::from_min_size(pos2(r.left() + k as f32 * w / 2.0, r.top()), vec2(w / 2.0, r.height())).shrink(2.0);
            let resp = ui.interact(seg, ui.scope_id().with(("lang", k)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            let on = i18n::en() == en;
            if on {
                ui.painter().rect_filled(seg, 9.0, white(40));
            } else if resp.hovered() {
                ui.painter().rect_filled(seg, 9.0, white(16));
            }
            ui.painter().text(seg.center(), egui::Align2::CENTER_CENTER, label, FontId::new(11.5, bold()), if on { TEXT } else { DIM });
            if resp.clicked() && !on {
                i18n::set(en);
                self.read_job("", INFO_SCRIPT.into()); // системная информация тоже на нужном языке
            }
        }
    }

    /// Заголовок страницы: мелкая строка, крупное название, подпись; справа поиск и кнопки.
    /// В узком окне поиск и кнопки переносятся на отдельную строку.
    fn header(&mut self, ui: &mut Ui, title: &str, sub: &str, right: impl FnOnce(&mut Self, &mut Ui)) {
        let wide = ui.available_width() >= 1000.0;
        let titles = |ui: &mut Ui, pc: &str| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new(pc.to_uppercase()).font(FontId::new(11.5, bold())).color(DIM));
                ui.label(RichText::new(title).font(FontId::new(30.0, bold())));
                ui.label(RichText::new(sub).color(DIM).font(FontId::new(13.5, bold())));
            });
        };
        let pc = self.pc.clone();
        let search_w = if wide { 300.0 } else { 240.0 };
        let right_side = |s: &mut Self, ui: &mut Ui| {
            right(s, ui);
            ui.add(
                egui::TextEdit::singleline(&mut s.search)
                    .hint_text(RichText::new(format!("{}  {}", ic::SEARCH, l("Поиск твиков", "Search tweaks"))).color(FAINT))
                    .desired_width(search_w)
                    .margin(vec2(12.0, 9.0))
                    .background_color(Color32::from_black_alpha(110)),
            );
        };
        if wide {
            ui.horizontal(|ui| {
                titles(ui, &pc);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| right_side(self, ui));
            });
        } else {
            titles(ui, &pc);
            ui.add_space(6.0);
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 38.0), Layout::right_to_left(Align::Center), |ui| right_side(self, ui));
        }
        ui.add_space(14.0);
    }

    fn admin_banner(&mut self, ui: &mut Ui) {
        // SPARKTWEAK_NOELEVATE — режим разработки и скриншотов
        if self.admin || std::env::var_os("SPARKTWEAK_NOELEVATE").is_some() {
            return;
        }
        glass_accent(AMBER).inner_margin(12).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(ic::WARNING).size(18.0).color(AMBER));
                ui.label(RichText::new(l("Нет прав администратора — большинство твиков не применится", "No administrator rights — most tweaks won't apply")).color(AMBER));
                if style::button(ui, ic::SHIELD, l("Перезапустить от администратора", "Restart as administrator")).clicked() && sys::relaunch_as_admin() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
        ui.add_space(12.0);
    }

    // ─────────────── страницы ───────────────

    fn home(&mut self, ui: &mut Ui) {
        let (on, all) = self.applied();
        self.header(ui, l("Обзор", "Overview"), &lf!("{all} твиков · {on} включено", "{all} tweaks · {on} enabled"), |s, ui| {
            if style::primary_button(ui, &format!("{}  {}", ic::BOLT, l("Оптимизировать", "Optimize"))).clicked() {
                let ids = s.profile_ids(|t| t.rec);
                s.bulk(l("Профиль «Рекомендуемый»", "Recommended profile"), ids, true);
            }
        });
        self.admin_banner(ui);

        let m = self.mem;
        let up = sys::uptime_secs();
        let (used, total, load) = ((m.total - m.avail) as f64 / GB, m.total as f64 / GB, m.load);
        let (commit, limit) = (m.commit_total as f64 / GB, m.commit_limit as f64 / GB);
        let (hrs, mins) = (up / 3600, up / 60 % 60);
        let stats = [
            (ic::CHECK, l("Твиков включено", "Tweaks enabled"), format!("{on}"), lf!("из {all}", "of {all}")),
            (ic::MEMORY, l("Память", "Memory"), lf!("{used:.1} ГБ", "{used:.1} GB"), lf!("из {total:.0} ГБ · {load}%", "of {total:.0} GB · {load}%")),
            (ic::STACK, l("Выделено", "Committed"), lf!("{commit:.1} ГБ", "{commit:.1} GB"), lf!("лимит {limit:.0} ГБ", "limit {limit:.0} GB")),
            (ic::CLOCK, l("Время работы", "Uptime"), lf!("{hrs}ч {mins}м", "{hrs}h {mins}m"), l("без перезагрузки", "since last restart").into()),
        ];
        let cols = if ui.available_width() < 900.0 { 2 } else { 4 };
        grid(ui, "stats", &stats, 0.0, cols, |ui, (icon, label, value, sub)| {
            ui.horizontal(|ui| {
                style::icon_tile(ui, icon, 40.0, TEXT);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(RichText::new(*label).font(FontId::new(12.5, bold())).color(DIM));
                    ui.label(RichText::new(value).font(FontId::new(22.0, bold())));
                    ui.label(RichText::new(sub).size(12.0).color(DIM));
                });
            });
        }, None::<fn(&mut Ui, &_)>, None::<fn(&mut Ui)>);

        section(ui, l("Профили", "Profiles"));
        type Pick = fn(&Tweak) -> bool;
        let profiles: [(&str, &str, &str, Pick); 3] = [
            (ic::SHIELD_CHECK, l("Рекомендуемый", "Recommended"), l("Безопасный набор для любого ПК", "A safe set for any PC"), |t| t.rec),
            (ic::GAME, l("Игровой", "Gaming"), l("Максимум FPS и минимальная задержка", "Max FPS and minimal input lag"), |t| t.rec || t.game),
            (ic::FLAME, l("Максимум", "Maximum"), l("Всё, кроме твиков с риском", "Everything except risky tweaks"), |t| !t.risk),
        ];
        let mut run: Option<(String, Vec<usize>)> = None;
        let cols = if ui.available_width() < 900.0 { 2 } else { 4 };
        let states = &self.states;
        let missing = |f: Pick| (0..TWEAKS.len()).filter(|&i| !TWEAKS[i].is_action() && f(&TWEAKS[i]) && states[i] != Some(true)).collect::<Vec<_>>();
        grid(ui, "profiles", &profiles, 150.0, cols, |ui, &(icon, name, desc, f)| {
            let left = missing(f).len();
            let total = TWEAKS.iter().filter(|t| !t.is_action() && f(t)).count();
            let done = total - left;
            card_head(ui, icon, name, desc, if left == 0 { (l("Применён", "Applied"), MINT) } else { (l("Не применён", "Not applied"), FAINT) });
            ui.add_space(6.0);
            ui.label(RichText::new(lf!("{done} из {total} твиков включено", "{done} of {total} tweaks enabled")).color(DIM).size(12.5));
            progress(ui, done as f32 / total.max(1) as f32);
        }, Some(|ui: &mut Ui, &(_, name, _, f): &(&str, &str, &str, Pick)| {
            if style::button(ui, ic::PLAY, l("Применить", "Apply")).clicked() {
                run = Some((lf!("Профиль «{name}»", "{name} profile"), missing(f)));
            }
        }), Some(|ui: &mut Ui| {
            // «пустая» карточка, как «Новый сервер» в Nitro
            ui.vertical_centered(|ui| {
                ui.add_space(24.0);
                ui.label(RichText::new("+").size(28.0).color(TEXT));
                ui.label(RichText::new(l("Свой набор", "Your own set")).font(FontId::new(16.0, bold())));
                ui.label(RichText::new(l("Включайте твики по одному в разделах слева", "Enable tweaks one by one in the sections on the left")).color(DIM).size(12.5));
            });
        }));
        if let Some((t, ids)) = run {
            self.bulk(&t, ids, true);
        }

        section(ui, l("Быстрые действия", "Quick actions"));
        let actions: [(&str, &str, &str); 5] = [
            (ic::MEMORY, l("Очистить RAM", "Clean RAM"), l("Рабочие наборы, standby и изменённые страницы", "Working sets, standby and modified pages")),
            (ic::BROOM, l("Удалить мусор", "Remove junk"), l("Temp, кэши, корзина, дампы", "Temp, caches, Recycle Bin, dumps")),
            (ic::DOWNLOAD, l("Драйверы", "Drivers"), l("Поиск новых драйверов в Windows Update", "Find new drivers in Windows Update")),
            (ic::SHIELD, l("Точка восстановления", "Restore point"), l("Снимок системы перед изменениями", "A system snapshot before changes")),
            (ic::UNDO, l("Откатить всё", "Revert all"), l("Вернуть все твики к значениям Windows", "Reset every tweak to Windows defaults")),
        ];
        let mut clicked = None;
        let cols = if ui.available_width() < 900.0 { 3 } else { 5 };
        let ids: Vec<usize> = (0..actions.len()).collect();
        grid(ui, "quick", &ids, 0.0, cols, |ui, &i| {
            let (icon, name, desc) = actions[i];
            let r = ui.horizontal(|ui| {
                style::icon_tile(ui, icon, 38.0, TEXT);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(RichText::new(name).font(FontId::new(14.5, bold())));
                    ui.label(RichText::new(desc).color(DIM).size(12.0));
                });
            });
            let hit = ui.interact(r.response.rect.expand(16.0), ui.scope_id().with(("qa", i)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            if hit.hovered() {
                ui.painter().rect_filled(hit.rect, 18.0, white(8));
            }
            if hit.clicked() {
                clicked = Some(i);
            }
        }, None::<fn(&mut Ui, &_)>, None::<fn(&mut Ui)>);
        match clicked {
            Some(0) => self.mem_job(vec![MemCmd::WorkingSets, MemCmd::ModifiedList, MemCmd::StandbyList]),
            Some(1) => self.clean_all(),
            Some(2) => self.run_action(TWEAKS.iter().position(|t| t.name == "Найти обновления драйверов").unwrap()),
            Some(3) => self.job(l("Точка восстановления", "Restore point"), "RestorePoint 'SparkTweak'".into()),
            Some(4) => {
                let ids = (0..TWEAKS.len()).filter(|&i| self.states[i] == Some(true)).collect();
                self.bulk(l("Откат всех твиков", "Reverting all tweaks"), ids, false);
            }
            _ => {}
        }

        section(ui, l("Система", "System"));
        glass().show(ui, |ui| {
            ui.set_width(ui.available_width());
            let cols = if ui.available_width() < 800.0 { 2 } else { 4 };
            egui::Grid::new("sys").num_columns(cols).spacing(vec2(28.0, 10.0)).show(ui, |ui| {
                let rows = [
                    ("os", ic::DESKTOP, "Windows"),
                    ("cpu", ic::CPU, l("Процессор", "CPU")),
                    ("gpu", ic::GAUGE, l("Видеокарта", "GPU")),
                    ("board", ic::STACK, l("Плата", "Motherboard")),
                    ("disk", ic::DRIVES, l("Диск C:", "Drive C:")),
                    ("power", ic::BOLT, l("Питание", "Power plan")),
                ];
                for (n, (k, icon, label)) in rows.iter().enumerate() {
                    ui.label(RichText::new(format!("{icon}  {label}")).color(DIM));
                    ui.label(self.info.get(*k).map_or("…", |s| s.as_str()));
                    if cols == 2 || n % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
        });
    }

    fn tweak_cards(&mut self, ui: &mut Ui, ids: &[usize]) {
        let mut toggle = None;
        let mut action = None;
        let (states, pending) = (&self.states, &self.pending);
        grid(ui, "tweaks", ids, 150.0, 0, |ui, &i| {
            let t = &TWEAKS[i];
            let st = if t.is_action() {
                (l("Действие", "Action"), CYAN)
            } else if pending.contains(&i) || states[i].is_none() {
                (l("Проверка…", "Checking…"), FAINT)
            } else if states[i] == Some(true) {
                (l("Включено", "Enabled"), MINT)
            } else {
                (l("Выключено", "Disabled"), FAINT)
            };
            card_head(ui, cat_info(t.cat).0, t.title(), "", st);
            ui.add_space(4.0);
            ui.label(RichText::new(t.about()).color(DIM).size(12.5));
        }, Some(|ui: &mut Ui, &i: &usize| {
            let t = &TWEAKS[i];
            if t.is_action() {
                if style::button(ui, ic::PLAY, l("Запустить", "Run")).clicked() {
                    action = Some(i);
                }
            } else if pending.contains(&i) {
                ui.spinner();
            } else if states[i] == Some(true) {
                if style::button(ui, ic::UNDO, l("Откатить", "Revert")).clicked() {
                    toggle = Some((i, false));
                }
            } else if style::button(ui, ic::PLAY, l("Включить", "Enable")).clicked() {
                toggle = Some((i, true));
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if t.rec {
                    style::chip(ui, l("рекомендуется", "recommended"), MINT);
                }
                if t.game {
                    style::chip(ui, l("игры", "gaming"), VIOLET);
                }
                if t.risk {
                    style::chip(ui, l("риск", "risky"), RED);
                }
                if t.reboot {
                    style::chip(ui, l("ребут", "reboot"), AMBER);
                }
            });
        }), None::<fn(&mut Ui)>);
        if let Some((i, on)) = toggle {
            self.set_tweak(i, on);
        }
        if let Some(i) = action {
            self.run_action(i);
        }
    }

    fn category(&mut self, ui: &mut Ui, cat: Cat) {
        let (_, name, sub) = cat_info(cat);
        let ids: Vec<usize> = (0..TWEAKS.len()).filter(|&i| TWEAKS[i].cat == cat).collect();
        let toggles: Vec<usize> = ids.iter().copied().filter(|&i| !TWEAKS[i].is_action()).collect();
        let on = toggles.iter().filter(|&&i| self.states[i] == Some(true)).count();
        let (n_all, n_tog) = (ids.len(), toggles.len());
        let subtitle = if toggles.is_empty() { lf!("{sub} · {n_all} действий", "{sub} · {n_all} actions") } else { lf!("{sub} · {on} из {n_tog} включено", "{sub} · {on} of {n_tog} enabled") };
        self.header(ui, name, &subtitle, |s, ui| {
            if !toggles.is_empty() {
                if style::primary_button(ui, &format!("{}  {}", ic::BOLT, l("Рекомендуемые", "Recommended"))).clicked() {
                    let r = toggles.iter().copied().filter(|&i| TWEAKS[i].rec && s.states[i] != Some(true)).collect();
                    s.bulk(&lf!("Рекомендуемое «{name}»", "Recommended: {name}"), r, true);
                }
                if style::button(ui, ic::UNDO, l("Откатить все", "Revert all")).clicked() {
                    let on = toggles.iter().copied().filter(|&i| s.states[i] == Some(true)).collect();
                    s.bulk(&lf!("Откат «{name}»", "Reverting: {name}"), on, false);
                }
            }
            if cat == Cat::Cleanup && style::primary_button(ui, &format!("{}  {}", ic::BROOM, l("Очистить всё", "Clean everything"))).clicked() {
                s.clean_all();
            }
        });
        self.admin_banner(ui);
        if cat == Cat::Memory {
            self.memory_panel(ui);
        }
        self.tweak_cards(ui, &ids);
    }

    fn memory_panel(&mut self, ui: &mut Ui) {
        let m = self.mem;
        let (used, total, free) = ((m.total - m.avail) as f64 / GB, m.total as f64 / GB, m.avail as f64 / GB);
        let (commit, limit) = (m.commit_total as f64 / GB, m.commit_limit as f64 / GB);
        glass().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ring(ui, m.load as f32 / 100.0);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width() - 6.0);
                    ui.label(RichText::new(lf!("{used:.1} ГБ из {total:.1} ГБ", "{used:.1} GB of {total:.1} GB")).font(FontId::new(22.0, bold())));
                    ui.label(RichText::new(lf!("Свободно {free:.1} ГБ · выделено {commit:.1} из {limit:.1} ГБ с файлом подкачки", "{free:.1} GB free · {commit:.1} of {limit:.1} GB committed incl. page file")).color(DIM));
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        if style::primary_button(ui, &format!("{}  {}", ic::SPARKLE, l("Полная очистка", "Full clean"))).clicked() {
                            self.mem_job(vec![MemCmd::WorkingSets, MemCmd::ModifiedList, MemCmd::StandbyList]);
                        }
                        for (cmd, tip) in [
                            (MemCmd::WorkingSets, l("Сбрасывает неиспользуемые страницы всех процессов", "Trims unused pages from every process")),
                            (MemCmd::StandbyList, l("Освобождает файловый кэш, как ISLC", "Frees the file cache, like ISLC")),
                            (MemCmd::LowPriorityStandby, l("Только кэш низкого приоритета", "Low-priority cache only")),
                            (MemCmd::ModifiedList, l("Записывает изменённые страницы на диск", "Writes modified pages to disk")),
                        ] {
                            if style::button(ui, "", cmd.label()).on_hover_text(tip).clicked() {
                                self.mem_job(vec![cmd]);
                            }
                        }
                    });
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        if style::toggle(ui, self.auto_clean).clicked() {
                            self.auto_clean = !self.auto_clean;
                            self.auto_at = Instant::now();
                        }
                        ui.label(l("Авто-очистка каждые", "Auto-clean every"));
                        let every = self.auto_every;
                        egui::ComboBox::from_id_salt("every").width(84.0).selected_text(lf!("{every} мин", "{every} min")).show_ui(ui, |ui| {
                            for v in [2, 5, 10, 15, 30, 60] {
                                ui.selectable_value(&mut self.auto_every, v, lf!("{v} мин", "{v} min"));
                            }
                        });
                        ui.label(RichText::new(l("пока SparkTweak открыт", "while SparkTweak is open")).color(DIM));
                    });
                });
            });
        });
        ui.add_space(14.0);
    }

    fn startup_page(&mut self, ui: &mut Ui) {
        if !self.startup_loaded {
            self.startup_loaded = true;
            self.read_job(l("Чтение автозагрузки", "Reading startup apps"), STARTUP_SCRIPT.into());
        }
        let (n, on) = (self.startup.len(), self.startup.iter().filter(|s| s.on).count());
        self.header(ui, l("Автозагрузка", "Startup apps"), &lf!("{n} программ · {on} запускается с Windows", "{n} apps · {on} start with Windows"), |s, ui| {
            if style::button(ui, ic::REFRESH, l("Обновить", "Refresh")).clicked() {
                s.read_job(l("Чтение автозагрузки", "Reading startup apps"), STARTUP_SCRIPT.into());
            }
        });
        if self.startup.is_empty() {
            glass().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(l("Записей нет (или список ещё загружается)", "No entries (or the list is still loading)")).color(DIM));
            });
            return;
        }
        let ids: Vec<usize> = (0..self.startup.len()).collect();
        let mut flip = None;
        let items = &self.startup;
        grid(ui, "startup", &ids, 120.0, 0, |ui, &i| {
            let s = &items[i];
            let scope = if s.key.starts_with("HKLM") { l("все пользователи", "all users") } else { l("текущий пользователь", "current user") };
            card_head(ui, ic::APP, &s.name, scope, if s.on { (l("Включено", "Enabled"), MINT) } else { (l("Отключено", "Disabled"), FAINT) });
            ui.add_space(4.0);
            ui.add(egui::Label::new(RichText::new(&s.cmd).monospace().size(11.0).color(FAINT)).wrap());
        }, Some(|ui: &mut Ui, &i: &usize| {
            if style::toggle(ui, items[i].on).clicked() {
                flip = Some(i);
            }
        }), None::<fn(&mut Ui)>);
        if let Some(i) = flip {
            let s = &mut self.startup[i];
            s.on = !s.on;
            let script = format!(
                "if (!(Test-Path {k})) {{ New-Item {k} -Force | Out-Null }}; Set-ItemProperty -Path {k} -Name {n} -Value ([byte[]]({b},0,0,0,0,0,0,0,0,0,0,0)) -Type Binary; '✓ {state}'",
                k = sys::ps_quote(&s.key),
                n = sys::ps_quote(&s.name),
                b = if s.on { 2 } else { 3 },
                state = if s.on { l("Включено", "Enabled") } else { l("Отключено", "Disabled") },
            );
            let name = s.name.clone();
            self.job(&lf!("Автозагрузка: {name}", "Startup: {name}"), script);
        }
    }

    fn appx_check_script() -> String {
        let list: Vec<String> = BLOAT.iter().map(|b| sys::ps_quote(b.0)).collect();
        format!(
            "$names = try {{ Get-AppxPackage -AllUsers -EA Stop | % Name }} catch {{ Get-AppxPackage | % Name }}\n$i = 0; foreach ($p in @({})) {{ \"@@APPX $i $([int]($names -contains $p))\"; $i++ }}",
            list.join(",")
        )
    }

    fn debloat_page(&mut self, ui: &mut Ui) {
        if !self.appx_loaded {
            self.appx_loaded = true;
            self.read_job(l("Поиск встроенных приложений", "Looking for built-in apps"), Self::appx_check_script());
        }
        let installed = self.appx.iter().filter(|a| **a == Some(true)).count();
        let (n, total) = (self.appx_sel.iter().filter(|s| **s).count(), BLOAT.len());
        self.header(ui, l("Удаление приложений", "Remove apps"), &lf!("{installed} из {total} предустановленных найдено", "{installed} of {total} preinstalled apps found"), |s, ui| {
            if style::primary_button(ui, &format!("{}  {} ({n})", ic::TRASH, l("Удалить", "Remove"))).clicked() && n > 0 {
                let mut script = String::new();
                let sel: Vec<usize> = (0..BLOAT.len()).filter(|&i| s.appx_sel[i]).collect();
                for i in sel {
                    let (pkg, ru, en) = BLOAT[i];
                    let q = sys::ps_quote(pkg);
                    script += &format!(
                        "'• {}'; Get-AppxPackage -AllUsers -Name {q} | Remove-AppxPackage -AllUsers -EA SilentlyContinue; Get-AppxProvisionedPackage -Online | ? DisplayName -eq {q} | Remove-AppxProvisionedPackage -Online -EA SilentlyContinue | Out-Null\n",
                        l(ru, en).replace('\'', "''")
                    );
                    s.appx_sel[i] = false;
                }
                script += &Self::appx_check_script();
                s.job(l("Удаление приложений", "Removing apps"), script);
            }
            if style::button(ui, ic::LIST, l("Выбрать все", "Select all")).clicked() {
                for i in 0..BLOAT.len() {
                    s.appx_sel[i] = s.appx[i] == Some(true) && BLOAT[i].0 != "Microsoft.GamingApp";
                }
            }
        });
        self.admin_banner(ui);
        let ids: Vec<usize> = (0..BLOAT.len()).collect();
        let (appx, sel) = (&self.appx, &mut self.appx_sel);
        grid(ui, "appx", &ids, 0.0, 0, |ui, &i| {
            let (pkg, ru, en) = BLOAT[i];
            let inst = appx[i];
            let c = if inst == Some(true) { TEXT } else { FAINT };
            ui.horizontal(|ui| {
                style::icon_tile(ui, ic::PACKAGE, 34.0, c);
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width() - 40.0);
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.add(egui::Label::new(RichText::new(l(ru, en)).font(FontId::new(14.0, bold())).color(c)).truncate());
                    ui.add(egui::Label::new(RichText::new(pkg).size(11.0).color(FAINT)).truncate());
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| match inst {
                    Some(true) => {
                        ui.checkbox(&mut sel[i], "");
                    }
                    Some(false) => {
                        ui.label(RichText::new(l("нет", "none")).size(12.0).color(FAINT));
                    }
                    None => {
                        ui.spinner();
                    }
                });
            });
        }, None::<fn(&mut Ui, &_)>, None::<fn(&mut Ui)>);
    }

    fn about(&mut self, ui: &mut Ui) {
        let v = env!("CARGO_PKG_VERSION");
        self.header(ui, l("О программе", "About"), &format!("SparkTweak v{v} · MIT License"), |_, _| {});
        glass().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                style::logo(ui, 64.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("SparkTweak").font(FontId::new(22.0, bold())));
                    ui.label(RichText::new(l("Оптимизация Windows на Rust. Каждый твик открыт и откатывается одной кнопкой.", "Windows optimization in Rust. Every tweak is open source and reverts with one click.")).color(DIM));
                });
            });
            ui.add_space(10.0);
            let (n, b) = (TWEAKS.len(), BLOAT.len());
            ui.label(format!("{}  {}", ic::LIST, lf!("Твиков и действий: {n}", "Tweaks and actions: {n}")));
            ui.label(format!("{}  {}", ic::PACKAGE, lf!("Приложений в списке удаления: {b}", "Apps in the removal list: {b}")));
            ui.label(format!("{}  {}", ic::SHIELD, l("Перед профилями автоматически создаётся точка восстановления", "A restore point is created before applying profiles")));
            ui.add_space(6.0);
            ui.hyperlink_to(format!("{}  github.com/s0nd3r-py/SparkTweak", ic::GLOBE), "https://github.com/s0nd3r-py/SparkTweak");
        });
    }

    fn search_results(&mut self, ui: &mut Ui) {
        let q = self.search.to_lowercase();
        let hit = |t: &Tweak| [t.name, t.desc, t.en.0, t.en.1].iter().any(|s| s.to_lowercase().contains(&q));
        let ids: Vec<usize> = (0..TWEAKS.len()).filter(|&i| hit(&TWEAKS[i])).collect();
        let (search, n) = (self.search.clone(), ids.len());
        self.header(ui, l("Поиск", "Search"), &lf!("«{search}» · найдено {n}", "\"{search}\" · {n} found"), |_, _| {});
        self.tweak_cards(ui, &ids);
    }

    /// Нижняя «таблетка» статуса с раскрывающимся журналом.
    fn activity(&mut self, ui: &mut Ui) {
        glass().inner_margin(egui::Margin::symmetric(14, 8)).corner_radius(14).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if self.busy > 0 {
                    ui.spinner();
                    let n = self.busy;
                    ui.label(RichText::new(lf!("Выполняется · в очереди {n}", "Working · {n} queued")).color(MINT).font(FontId::new(13.0, bold())));
                } else {
                    style::status(ui, l("Готово", "Ready"), MINT);
                }
                if let Some(last) = self.log.last() {
                    let w = (ui.available_width() - 120.0).max(0.0);
                    ui.allocate_ui_with_layout(vec2(w, 18.0), Layout::left_to_right(Align::Center), |ui| {
                        ui.add(egui::Label::new(RichText::new(last).color(DIM).size(12.5)).truncate());
                    });
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let caret = if self.log_open { ic::CARET_DOWN } else { ic::CARET_UP };
                    if ui.small_button(format!("{caret}  {}", l("Журнал", "Log"))).clicked() {
                        self.log_open = !self.log_open;
                    }
                });
            });
            if self.log_open {
                ui.add_space(4.0);
                ScrollArea::vertical().stick_to_bottom(true).max_height(180.0).auto_shrink([false, true]).show(ui, |ui| {
                    for line in &self.log {
                        let c = if line.starts_with('→') {
                            TEXT
                        } else if line.contains('✓') {
                            MINT
                        } else if line.contains('✗') || line.contains("Error") || line.contains("Ошибка") {
                            RED
                        } else if line.contains('⚠') || line.contains('⟳') {
                            AMBER
                        } else {
                            DIM
                        };
                        ui.label(RichText::new(line).monospace().size(12.0).color(c));
                    }
                });
                ui.horizontal(|ui| {
                    if ui.small_button(format!("{}  {}", ic::COPY, l("Копировать", "Copy"))).clicked() {
                        ui.ctx().copy_text(self.log.join("\n"));
                    }
                });
            }
        });
    }
}

// ─────────────── мелкие виджеты ───────────────

fn with_freed(body: &str) -> String {
    format!("$f0 = (Get-PSDrive C).Free\n{body}\n(T '✓ Освобождено на C: {{0:N0}} МБ' '✓ Freed on C: {{0:N0}} MB') -f ([math]::Max(0, ((Get-PSDrive C).Free - $f0)) / 1MB)")
}

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(title).font(FontId::new(17.0, bold())));
    ui.add_space(2.0);
}

const GAP: f32 = 14.0;
const PAD: f32 = 16.0;
const FOOT: f32 = 30.0 + 13.0;

/// Сетка стеклянных карточек. Высота строки = самая высокая карточка в ней
/// (меряется на прошлом кадре), поэтому текст не обрезается, а низ карточек совпадает.
/// `cols = 0` — сколько влезет по ширине. `foot` рисуется внизу карточки справа налево.
fn grid<T>(
    ui: &mut Ui,
    salt: &str,
    items: &[T],
    min_h: f32,
    cols: usize,
    mut body: impl FnMut(&mut Ui, &T),
    mut foot: Option<impl FnMut(&mut Ui, &T)>,
    extra: Option<impl FnOnce(&mut Ui)>,
) {
    let w = ui.available_width();
    let cols = if cols > 0 { cols } else { ((w + GAP) / (320.0 + GAP)).floor().max(1.0) as usize };
    let cw = (w - GAP * (cols - 1) as f32) / cols as f32;
    let mut extra = extra;
    let n = items.len() + extra.is_some() as usize;
    for row in 0..n.div_ceil(cols) {
        let id = ui.scope_id().with((salt, row, cols));
        let h = ui.ctx().data(|d| d.get_temp::<f32>(id)).unwrap_or(min_h).max(min_h);
        let mut need: f32 = min_h;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for c in 0..cols {
                let idx = row * cols + c;
                if idx >= n {
                    break;
                }
                ui.allocate_ui_with_layout(vec2(cw, h), Layout::top_down(Align::Min), |ui| {
                    if idx < items.len() {
                        glass().show(ui, |ui| {
                            ui.set_width(cw - 2.0 * PAD - 2.0); // 2 px — рамка карточки
                            let top = ui.cursor().top();
                            body(ui, &items[idx]);
                            let body_h = ui.cursor().top() - top;
                            let foot_h = if foot.is_some() { FOOT } else { 0.0 };
                            need = need.max(body_h + foot_h + 2.0 * PAD);
                            ui.add_space((h - 2.0 * PAD - body_h - foot_h).max(0.0));
                            if let Some(f) = foot.as_mut() {
                                let y = ui.cursor().top() + 4.0;
                                ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0, white(22)));
                                ui.add_space(13.0);
                                ui.allocate_ui_with_layout(vec2(cw - 2.0 * PAD, 30.0), Layout::right_to_left(Align::Center), |ui| f(ui, &items[idx]));
                            }
                        });
                    } else if let Some(e) = extra.take() {
                        // пунктирная «пустая» карточка
                        let (r, _) = ui.allocate_exact_size(vec2(cw, h.max(min_h)), Sense::hover());
                        ui.painter().rect_filled(r, 18.0, white(14));
                        ui.painter().rect_stroke(r, 18.0, Stroke::new(1.0, white(34)), egui::StrokeKind::Inside);
                        ui.scope_builder(egui::UiBuilder::new().max_rect(r.shrink(PAD)), e);
                    }
                });
            }
        });
        if (need - h).abs() > 0.5 {
            ui.ctx().data_mut(|d| d.insert_temp(id, need));
            ui.ctx().request_repaint();
        }
        ui.add_space(GAP - ui.spacing().item_spacing.y);
    }
}

/// Верх карточки: плитка-иконка, название, подпись, статус справа (в узкой карточке — под названием).
fn card_head(ui: &mut Ui, icon: &str, title: &str, sub: &str, st: (&str, Color32)) {
    let narrow = ui.available_width() < 300.0;
    ui.horizontal_top(|ui| {
        style::icon_tile(ui, icon, 36.0, TEXT);
        let w = ui.available_width() - if narrow { 0.0 } else { 120.0 };
        ui.vertical(|ui| {
            ui.set_max_width(w);
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.add(egui::Label::new(RichText::new(title).font(FontId::new(15.0, bold()))).wrap());
            if !sub.is_empty() {
                ui.add(egui::Label::new(RichText::new(sub).color(DIM).size(12.5)).wrap());
            }
            if narrow {
                ui.add_space(2.0);
                style::status(ui, st.0, st.1);
            }
        });
        if !narrow {
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| style::status(ui, st.0, st.1));
        }
    });
}

fn progress(ui: &mut Ui, frac: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 5.0), Sense::hover());
    ui.painter().rect_filled(r, 3.0, white(20));
    if frac > 0.0 {
        let mut f = r;
        f.set_width(r.width() * frac.clamp(0.0, 1.0));
        ui.painter().rect_filled(f, 3.0, MINT);
    }
}

fn ring(ui: &mut Ui, frac: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(112.0, 112.0), Sense::hover());
    let p = ui.painter();
    let (center, rad) = (r.center(), 46.0);
    p.circle_stroke(center, rad, Stroke::new(9.0, white(20)));
    let c = match (frac * 100.0) as u32 {
        0..=59 => MINT,
        60..=84 => AMBER,
        _ => RED,
    };
    let n = 96;
    let pts: Vec<egui::Pos2> = (0..=((n as f32 * frac.clamp(0.0, 1.0)) as usize))
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2 + i as f32 / n as f32 * std::f32::consts::TAU;
            center + vec2(a.cos(), a.sin()) * rad
        })
        .collect();
    if pts.len() > 1 {
        p.add(egui::Shape::line(pts, Stroke::new(9.0, c)));
    }
    p.text(center - vec2(0.0, 6.0), egui::Align2::CENTER_CENTER, format!("{:.0}%", frac * 100.0), FontId::new(24.0, bold()), TEXT);
    p.text(center + vec2(0.0, 16.0), egui::Align2::CENTER_CENTER, l("занято", "used"), FontId::proportional(11.5), DIM);
}

impl eframe::App for App {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Line(l) => self.handle(l),
                Msg::Done => self.busy = self.busy.saturating_sub(1),
            }
        }
        if self.mem_at.elapsed() > Duration::from_secs(1) {
            self.mem = sys::mem_status();
            self.mem_at = Instant::now();
        }
        if self.auto_clean && self.auto_at.elapsed() > Duration::from_secs(self.auto_every * 60) {
            self.auto_at = Instant::now();
            self.mem_job(vec![MemCmd::WorkingSets, MemCmd::StandbyList]);
        }
        ctx.request_repaint_after(Duration::from_secs(1));
    }

    fn ui(&mut self, ui: &mut Ui, _: &mut eframe::Frame) {
        let full = ui.max_rect();
        style::paint_background(ui, full, self.solid);
        let section = match (self.search.is_empty(), self.page) {
            (false, _) => l("Поиск", "Search"),
            (_, Page::Home) => l("Обзор", "Overview"),
            (_, Page::Cat(c)) => cat_info(c).1,
            (_, Page::Startup) => l("Автозагрузка", "Startup apps"),
            (_, Page::Debloat) => l("Приложения", "Apps"),
            (_, Page::About) => l("О программе", "About"),
        };
        let title = Rect::from_min_size(full.min, vec2(full.width(), 34.0));
        style::title_bar(ui, title, "SparkTweak", &format!("{section} · {}", env!("CARGO_PKG_VERSION")));
        self.lang_switch(ui, title);

        let rail = Rect::from_min_max(pos2(full.left() + 16.0, title.bottom() + 16.0), pos2(full.left() + 90.0, full.bottom() - 16.0));
        self.rail(ui, rail);

        let content = Rect::from_min_max(pos2(rail.right() + 20.0, title.bottom() + 18.0), pos2(full.right() - 18.0, full.bottom() - 14.0));
        let mut root = ui.new_child(egui::UiBuilder::new().max_rect(content));
        egui::Panel::bottom("activity").frame(egui::Frame::new().inner_margin(egui::Margin { top: 8, ..Default::default() })).resizable(false).show(&mut root, |ui| self.activity(ui));
        egui::CentralPanel::default().frame(egui::Frame::new()).show(&mut root, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(ui.available_width() - 12.0);
                if !self.search.is_empty() {
                    self.search_results(ui);
                } else {
                    match self.page {
                        Page::Home => self.home(ui),
                        Page::Cat(c) => self.category(ui, c),
                        Page::Startup => self.startup_page(ui),
                        Page::Debloat => self.debloat_page(ui),
                        Page::About => self.about(ui),
                    }
                }
                ui.add_space(8.0);
            });
        });

        // уголок для изменения размера окна без системной рамки
        let grip = Rect::from_min_size(full.max - vec2(16.0, 16.0), vec2(16.0, 16.0));
        let g = ui.interact(grip, ui.scope_id().with("grip"), Sense::drag()).on_hover_cursor(egui::CursorIcon::ResizeNwSe);
        if g.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::BeginResize(egui::ResizeDirection::SouthEast));
        }
    }
}
