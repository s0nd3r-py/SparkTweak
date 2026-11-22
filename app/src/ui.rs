use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use eframe::egui::{self, Align, Color32, FontId, Layout, Rect, RichText, ScrollArea, Sense, Stroke, Ui, pos2, vec2};

use crate::style::{self, AMBER, CYAN, DIM, FAINT, MINT, RED, TEXT, VIOLET, bold, glass, glass_accent, ic, white};
use crate::sys::{self, MemCmd};
use crate::tweaks::{BLOAT, CATS, Cat, TWEAKS, Tweak};

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

fn worker(jobs: Receiver<Job>, out: Sender<Msg>, ctx: egui::Context) {
    for job in jobs {
        let send = |s: String| {
            let _ = out.send(Msg::Line(s));
            ctx.request_repaint();
        };
        match job {
            Job::Ps { title, script } => {
                if !title.is_empty() {
                    send(format!("▶ {title}"));
                }
                // код выхода PowerShell ненадёжен (его портят нативные команды), ошибки и так видны в журнале
                sys::run_ps(&script, &send);
            }
            Job::Mem(cmds) => {
                send("▶ Очистка оперативной памяти".into());
                let before = sys::mem_status().avail;
                for c in cmds {
                    match sys::mem_purge(c) {
                        Ok(()) => send(format!("  ✓ {}", c.label())),
                        Err(st) => send(format!("  ✗ {} (NTSTATUS 0x{:08X}, нужны права администратора)", c.label(), st as u32)),
                    }
                }
                let freed = sys::mem_status().avail.saturating_sub(before);
                send(format!("✓ Освобождено {} МБ", freed / 1024 / 1024));
            }
        }
        let _ = out.send(Msg::Done);
        ctx.request_repaint();
    }
}

const INFO_SCRIPT: &str = r#"
"@@INFO cpu " + (Get-ItemProperty 'HKLM:\HARDWARE\DESCRIPTION\System\CentralProcessor\0').ProcessorNameString.Trim()
"@@INFO gpu " + ((Get-CimInstance Win32_VideoController | % Name) -join ' + ')
$o = Get-CimInstance Win32_OperatingSystem; "@@INFO os $($o.Caption -replace 'Microsoft ', '') · сборка $($o.BuildNumber)"
$d = Get-PSDrive C; "@@INFO disk {0:N0} ГБ свободно из {1:N0} ГБ" -f ($d.Free / 1GB), (($d.Used + $d.Free) / 1GB)
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
        std::thread::spawn(move || worker(read_jobs, out2, ctx2));
        let mut app = Self {
            tx,
            tx_read,
            rx,
            busy: 0,
            states: vec![None; TWEAKS.len()],
            pending: HashSet::new(),
            log: vec![format!("SparkTweak v{} готов к работе", env!("CARGO_PKG_VERSION"))],
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
        let s: String = TWEAKS.iter().enumerate().filter(|(_, t)| !t.is_action()).map(|(i, t)| t.state_line(i) + "\n").collect();
        self.read_job("Проверка состояния твиков", s);
    }

    fn set_tweak(&mut self, i: usize, on: bool) {
        let t = &TWEAKS[i];
        let body = if on { t.apply_script() } else { t.revert_script() };
        let verb = if on { "Включение" } else { "Откат" };
        self.pending.insert(i);
        self.job(&format!("{verb}: {}", t.name), format!("{body}\n{}", t.state_line(i)));
    }

    fn run_action(&mut self, i: usize) {
        let t = &TWEAKS[i];
        let body = if t.cat == Cat::Cleanup { with_freed(t.apply) } else { t.apply.to_string() };
        self.job(t.name, body);
    }

    /// Применяет (или откатывает) набор твиков одним скриптом. Перед применением — точка восстановления.
    fn bulk(&mut self, title: &str, ids: Vec<usize>, on: bool) {
        if ids.is_empty() {
            self.log.push(format!("✓ {title}: всё уже сделано"));
            return;
        }
        let mut s = if on { "RestorePoint 'SparkTweak: перед оптимизацией'\n".to_string() } else { String::new() };
        for &i in &ids {
            let t = &TWEAKS[i];
            self.pending.insert(i);
            let body = if on { t.apply_script() } else { t.revert_script() };
            s += &format!("'• {}'\n& {{\n{body}\n}}\n{}\n", t.name.replace('\'', "''"), t.state_line(i));
        }
        if ids.iter().any(|&i| TWEAKS[i].reboot) {
            s += "'⟳ Часть изменений вступит в силу после перезагрузки'\n";
        }
        self.job(&format!("{title} ({} твиков)", ids.len()), s);
    }

    fn profile_ids(&self, f: impl Fn(&Tweak) -> bool) -> Vec<usize> {
        (0..TWEAKS.len()).filter(|&i| !TWEAKS[i].is_action() && f(&TWEAKS[i]) && self.states[i] != Some(true)).collect()
    }

    fn clean_all(&mut self) {
        let body: String = TWEAKS
            .iter()
            .filter(|t| t.cat == Cat::Cleanup && !t.risk && !t.name.contains("WinSxS"))
            .map(|t| format!("'• {}'\n{}\n", t.name, t.apply))
            .collect();
        self.job("Очистка мусора", with_freed(&body));
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

    // ─────────────── каркас ───────────────

    /// Узкая панель из иконок, как в Nitro.
    fn rail(&mut self, ui: &mut Ui, rect: Rect) {
        let p = ui.painter();
        p.rect_filled(rect, 26.0, Color32::from_black_alpha(55));
        p.rect_stroke(rect, 26.0, Stroke::new(1.0, white(24)), egui::StrokeKind::Inside);
        let cx = rect.center().x;
        let mut y = rect.top() + 14.0;
        let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));

        let logo_r = Rect::from_center_size(pos2(cx, y + 24.0), vec2(48.0, 48.0));
        let lr = ui.scope_builder(egui::UiBuilder::new().max_rect(logo_r), |ui| style::logo(ui, 48.0)).inner;
        if lr.on_hover_text("SparkTweak").clicked() {
            self.go(Page::Home);
        }
        y += 48.0 + 18.0;

        let mut item = |ui: &mut Ui, y: &mut f32, page: Page, icon: &str, tip: &str, dot: Option<Color32>| {
            let r = Rect::from_center_size(pos2(cx, *y + 19.0), vec2(40.0, 40.0));
            let resp = ui.interact(r, ui.scope_id().with(tip), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            let active = self.page == page && self.search.is_empty();
            let p = ui.painter();
            if active {
                p.rect_filled(r, 13.0, white(30));
                p.rect_stroke(r, 13.0, Stroke::new(1.0, white(50)), egui::StrokeKind::Inside);
            } else if resp.hovered() {
                p.rect_filled(r, 13.0, white(14));
            }
            p.text(r.center(), egui::Align2::CENTER_CENTER, icon, FontId::proportional(19.0), if active { TEXT } else { DIM });
            if let Some(c) = dot {
                p.circle_filled(r.right_bottom() - vec2(7.0, 7.0), 4.5, Color32::from_rgb(30, 32, 35));
                p.circle_filled(r.right_bottom() - vec2(7.0, 7.0), 3.0, c);
            }
            if resp.on_hover_text(tip).clicked() {
                self.page = page;
                self.search.clear();
            }
            *y += 43.0;
        };
        item(&mut ui, &mut y, Page::Home, ic::GRID, "Обзор", None);
        y += 4.0;
        ui.painter().hline((cx - 14.0)..=(cx + 14.0), y, Stroke::new(1.0, white(30)));
        y += 6.0;
        for &(cat, icon, name, _) in CATS {
            let any_on = TWEAKS.iter().enumerate().any(|(i, t)| t.cat == cat && self.states[i] == Some(true));
            item(&mut ui, &mut y, Page::Cat(cat), icon, name, any_on.then_some(MINT));
        }
        y += 4.0;
        ui.painter().hline((cx - 14.0)..=(cx + 14.0), y, Stroke::new(1.0, white(30)));
        y += 6.0;
        item(&mut ui, &mut y, Page::Startup, ic::POWER, "Автозагрузка", None);
        item(&mut ui, &mut y, Page::Debloat, ic::TRASH, "Удаление приложений", None);
        item(&mut ui, &mut y, Page::About, ic::INFO, "О программе", None);
    }

    fn go(&mut self, p: Page) {
        self.page = p;
        self.search.clear();
    }

    /// Заголовок страницы: мелкая строка сверху, крупное название, подпись; справа — поиск и кнопки.
    fn header(&mut self, ui: &mut Ui, title: &str, sub: &str, right: impl FnOnce(&mut Self, &mut Ui)) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(self.pc.to_uppercase()).size(11.5).color(DIM).font(FontId::new(11.5, bold())));
                ui.add_space(-4.0);
                ui.label(RichText::new(title).font(FontId::new(30.0, bold())));
                ui.add_space(-6.0);
                ui.label(RichText::new(sub).color(DIM).font(FontId::new(14.0, bold())));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                right(self, ui);
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text(RichText::new(format!("{}  Поиск по названию или описанию твика", ic::SEARCH)).color(FAINT))
                        .desired_width(320.0)
                        .margin(vec2(12.0, 9.0))
                        .background_color(Color32::from_black_alpha(110)),
                );
            });
        });
        ui.add_space(12.0);
    }

    fn admin_banner(&mut self, ui: &mut Ui) {
        // SPARKTWEAK_NOELEVATE — режим разработки и скриншотов
        if self.admin || std::env::var_os("SPARKTWEAK_NOELEVATE").is_some() {
            return;
        }
        glass_accent(AMBER).inner_margin(12).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(ic::WARNING).size(18.0).color(AMBER));
                ui.label(RichText::new("Нет прав администратора — большинство твиков не применится").color(AMBER));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if style::button(ui, ic::SHIELD, "Перезапустить от администратора").clicked() && sys::relaunch_as_admin() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });
        ui.add_space(12.0);
    }

    // ─────────────── страницы ───────────────

    fn home(&mut self, ui: &mut Ui) {
        let (on, all) = self.applied();
        self.header(ui, "Обзор", &format!("{} твиков · {on} включено", all), |s, ui| {
            if style::primary_button(ui, &format!("{}  Оптимизировать", ic::BOLT)).clicked() {
                let ids = s.profile_ids(|t| t.rec);
                s.bulk("Профиль «Рекомендуемый»", ids, true);
            }
        });
        self.admin_banner(ui);

        let m = self.mem;
        let up = sys::uptime_secs();
        let stats = [
            (ic::CHECK, "Твиков включено", format!("{on}"), format!("из {all}")),
            (ic::MEMORY, "Память", format!("{:.1} ГБ", (m.total - m.avail) as f64 / GB), format!("из {:.0} ГБ · {}%", m.total as f64 / GB, m.load)),
            (ic::STACK, "Выделено", format!("{:.1} ГБ", m.commit_total as f64 / GB), format!("лимит {:.0} ГБ", m.commit_limit as f64 / GB)),
            (ic::CLOCK, "Время работы", format!("{}ч {}м", up / 3600, up / 60 % 60), "без перезагрузки".into()),
        ];
        ui.columns(4, |c| {
            for (i, (icon, label, value, sub)) in stats.iter().enumerate() {
                stat(&mut c[i], icon, label, value, sub);
            }
        });
        ui.add_space(14.0);

        section(ui, "Профили");
        let profiles: [(&str, &str, &str, fn(&Tweak) -> bool); 3] = [
            (ic::SHIELD_CHECK, "Рекомендуемый", "Безопасный набор для любого ПК", |t| t.rec),
            (ic::GAME, "Игровой", "Максимум FPS и минимальная задержка", |t| t.rec || t.game),
            (ic::FLAME, "Максимум", "Всё, кроме твиков с риском", |t| !t.risk),
        ];
        let mut run: Option<(String, Vec<usize>)> = None;
        let cards: Vec<usize> = (0..3).collect();
        grid(ui, &cards, 222.0, 4, |ui, &i| {
            let (icon, name, desc, f) = profiles[i];
            let ids = self.profile_ids(f);
            let total = TWEAKS.iter().filter(|t| !t.is_action() && f(t)).count();
            let done = total - ids.len();
            card_head(ui, icon, name, desc, if ids.is_empty() { ("Применён", MINT) } else { ("Не применён", FAINT) });
            ui.add_space(8.0);
            ui.label(RichText::new(format!("{done} из {total} твиков включено")).color(DIM).size(12.5));
            progress(ui, done as f32 / total.max(1) as f32);
            card_foot(ui, |ui| {
                if style::button(ui, ic::PLAY, "Применить").clicked() {
                    run = Some((format!("Профиль «{name}»"), ids));
                }
            });
        }, Some(|ui: &mut Ui| {
            // «пустая» карточка, как «Новый сервер» в Nitro
            ui.vertical_centered(|ui| {
                ui.add_space(38.0);
                ui.label(RichText::new("+").size(28.0).color(TEXT));
                ui.label(RichText::new("Свой набор").font(FontId::new(16.0, bold())));
                ui.label(RichText::new("Включайте твики по одному в разделах слева").color(DIM).size(12.5));
            });
        }));
        if let Some((t, ids)) = run {
            self.bulk(&t, ids, true);
        }
        ui.add_space(6.0);

        section(ui, "Быстрые действия");
        let actions: [(&str, &str, &str); 5] = [
            (ic::MEMORY, "Очистить RAM", "Рабочие наборы, standby и изменённые страницы"),
            (ic::BROOM, "Удалить мусор", "Temp, кэши, корзина, дампы"),
            (ic::DOWNLOAD, "Драйверы", "Поиск новых драйверов в Windows Update"),
            (ic::SHIELD, "Точка восстановления", "Снимок системы перед изменениями"),
            (ic::UNDO, "Откатить всё", "Вернуть все твики к значениям Windows"),
        ];
        let ids: Vec<usize> = (0..actions.len()).collect();
        let mut clicked = None;
        grid(ui, &ids, 112.0, 5, |ui, &i| {
            let (icon, name, desc) = actions[i];
            let r = ui.horizontal(|ui| {
                style::icon_tile(ui, icon, 38.0, TEXT);
                ui.vertical(|ui| {
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
        }, None::<fn(&mut Ui)>);
        match clicked {
            Some(0) => self.mem_job(vec![MemCmd::WorkingSets, MemCmd::ModifiedList, MemCmd::StandbyList]),
            Some(1) => self.clean_all(),
            Some(2) => self.run_action(TWEAKS.iter().position(|t| t.name == "Найти обновления драйверов").unwrap()),
            Some(3) => self.job("Точка восстановления", "RestorePoint 'SparkTweak: вручную'".into()),
            Some(4) => {
                let ids = (0..TWEAKS.len()).filter(|&i| self.states[i] == Some(true)).collect();
                self.bulk("Откат всех твиков", ids, false);
            }
            _ => {}
        }
        ui.add_space(6.0);

        section(ui, "Система");
        glass().show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::Grid::new("sys").num_columns(4).spacing(vec2(28.0, 10.0)).show(ui, |ui| {
                let rows = [("os", ic::DESKTOP, "Windows"), ("cpu", ic::CPU, "Процессор"), ("gpu", ic::GAUGE, "Видеокарта"), ("board", ic::STACK, "Плата"), ("disk", ic::DRIVES, "Диск C:"), ("power", ic::BOLT, "Питание")];
                for (n, (k, icon, label)) in rows.iter().enumerate() {
                    ui.label(RichText::new(format!("{icon}  {label}")).color(DIM));
                    ui.label(self.info.get(*k).map_or("…", |s| s.as_str()));
                    if n % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
        });
    }

    fn tweak_cards(&mut self, ui: &mut Ui, ids: &[usize]) {
        let mut toggle = None;
        let mut action = None;
        grid(ui, ids, 214.0, 0, |ui, &i| {
            let t = &TWEAKS[i];
            let icon = CATS.iter().find(|c| c.0 == t.cat).map_or(ic::BOLT, |c| c.1);
            let st = if t.is_action() {
                ("Действие", CYAN)
            } else if self.pending.contains(&i) || self.states[i].is_none() {
                ("Проверка…", FAINT)
            } else if self.states[i] == Some(true) {
                ("Включено", MINT)
            } else {
                ("Выключено", FAINT)
            };
            card_head(ui, icon, t.name, "", st);
            ui.add_space(6.0);
            ui.label(RichText::new(t.desc).color(DIM).size(12.5));
            card_foot(ui, |ui| {
                if t.is_action() {
                    if style::button(ui, ic::PLAY, "Запустить").clicked() {
                        action = Some(i);
                    }
                } else if self.pending.contains(&i) {
                    ui.spinner();
                } else if self.states[i] == Some(true) {
                    if style::button(ui, ic::UNDO, "Откатить").clicked() {
                        toggle = Some((i, false));
                    }
                } else if style::button(ui, ic::PLAY, "Включить").clicked() {
                    toggle = Some((i, true));
                }
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if t.rec {
                        style::chip(ui, "рекомендуется", MINT);
                    }
                    if t.game {
                        style::chip(ui, "игры", VIOLET);
                    }
                    if t.risk {
                        style::chip(ui, "риск", RED);
                    }
                    if t.reboot {
                        style::chip(ui, "ребут", AMBER);
                    }
                });
            });
        }, None::<fn(&mut Ui)>);
        if let Some((i, on)) = toggle {
            self.set_tweak(i, on);
        }
        if let Some(i) = action {
            self.run_action(i);
        }
    }

    fn category(&mut self, ui: &mut Ui, cat: Cat) {
        let &(_, _, name, sub) = CATS.iter().find(|c| c.0 == cat).unwrap();
        let ids: Vec<usize> = (0..TWEAKS.len()).filter(|&i| TWEAKS[i].cat == cat).collect();
        let toggles: Vec<usize> = ids.iter().copied().filter(|&i| !TWEAKS[i].is_action()).collect();
        let on = toggles.iter().filter(|&&i| self.states[i] == Some(true)).count();
        let subtitle = if toggles.is_empty() { format!("{} · {} действий", sub, ids.len()) } else { format!("{} · {on} из {} включено", sub, toggles.len()) };
        self.header(ui, name, &subtitle, |s, ui| {
            if !toggles.is_empty() {
                if style::primary_button(ui, &format!("{}  Рекомендуемые", ic::BOLT)).clicked() {
                    let r = toggles.iter().copied().filter(|&i| TWEAKS[i].rec && s.states[i] != Some(true)).collect();
                    s.bulk(&format!("Рекомендуемое «{name}»"), r, true);
                }
                if style::button(ui, ic::UNDO, "Откатить все").clicked() {
                    let on = toggles.iter().copied().filter(|&i| s.states[i] == Some(true)).collect();
                    s.bulk(&format!("Откат «{name}»"), on, false);
                }
            }
            if cat == Cat::Cleanup && style::primary_button(ui, &format!("{}  Очистить всё", ic::BROOM)).clicked() {
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
        glass().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ring(ui, m.load as f32 / 100.0);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(format!("{:.1} ГБ из {:.1} ГБ", (m.total - m.avail) as f64 / GB, m.total as f64 / GB)).font(FontId::new(22.0, bold())));
                    ui.label(RichText::new(format!("Свободно {:.1} ГБ · выделено {:.1} из {:.1} ГБ с файлом подкачки", m.avail as f64 / GB, m.commit_total as f64 / GB, m.commit_limit as f64 / GB)).color(DIM));
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        if style::primary_button(ui, &format!("{}  Полная очистка", ic::SPARKLE)).clicked() {
                            self.mem_job(vec![MemCmd::WorkingSets, MemCmd::ModifiedList, MemCmd::StandbyList]);
                        }
                        for (cmd, tip) in [
                            (MemCmd::WorkingSets, "Сбрасывает неиспользуемые страницы всех процессов"),
                            (MemCmd::StandbyList, "Освобождает файловый кэш, как ISLC"),
                            (MemCmd::LowPriorityStandby, "Только кэш низкого приоритета"),
                            (MemCmd::ModifiedList, "Записывает изменённые страницы на диск"),
                        ] {
                            if style::button(ui, "", cmd.label()).on_hover_text(tip).clicked() {
                                self.mem_job(vec![cmd]);
                            }
                        }
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if style::toggle(ui, self.auto_clean).clicked() {
                            self.auto_clean = !self.auto_clean;
                            self.auto_at = Instant::now();
                        }
                        ui.label("Авто-очистка каждые");
                        egui::ComboBox::from_id_salt("every").width(84.0).selected_text(format!("{} мин", self.auto_every)).show_ui(ui, |ui| {
                            for v in [2, 5, 10, 15, 30, 60] {
                                ui.selectable_value(&mut self.auto_every, v, format!("{v} мин"));
                            }
                        });
                        ui.label(RichText::new("пока SparkTweak открыт").color(DIM));
                    });
                });
            });
        });
        ui.add_space(14.0);
    }

    fn startup_page(&mut self, ui: &mut Ui) {
        if !self.startup_loaded {
            self.startup_loaded = true;
            self.read_job("Чтение автозагрузки", STARTUP_SCRIPT.into());
        }
        let on = self.startup.iter().filter(|s| s.on).count();
        self.header(ui, "Автозагрузка", &format!("{} программ · {on} запускается с Windows", self.startup.len()), |s, ui| {
            if style::button(ui, ic::REFRESH, "Обновить").clicked() {
                s.read_job("Чтение автозагрузки", STARTUP_SCRIPT.into());
            }
        });
        if self.startup.is_empty() {
            glass().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Записей нет (или список ещё загружается)").color(DIM));
            });
            return;
        }
        let ids: Vec<usize> = (0..self.startup.len()).collect();
        let mut flip = None;
        grid(ui, &ids, 168.0, 0, |ui, &i| {
            let s = &self.startup[i];
            let scope = if s.key.starts_with("HKLM") { "все пользователи" } else { "текущий пользователь" };
            card_head(ui, ic::APP, &s.name, scope, if s.on { ("Включено", MINT) } else { ("Отключено", FAINT) });
            ui.add_space(4.0);
            ui.label(RichText::new(&s.cmd).monospace().size(11.0).color(FAINT));
            card_foot(ui, |ui| {
                if style::toggle(ui, s.on).clicked() {
                    flip = Some(i);
                }
            });
        }, None::<fn(&mut Ui)>);
        if let Some(i) = flip {
            let s = &mut self.startup[i];
            s.on = !s.on;
            let script = format!(
                "if (!(Test-Path {k})) {{ New-Item {k} -Force | Out-Null }}; Set-ItemProperty -Path {k} -Name {n} -Value ([byte[]]({b},0,0,0,0,0,0,0,0,0,0,0)) -Type Binary; '✓ {state}'",
                k = sys::ps_quote(&s.key),
                n = sys::ps_quote(&s.name),
                b = if s.on { 2 } else { 3 },
                state = if s.on { "Включено" } else { "Отключено" },
            );
            let title = format!("Автозагрузка: {}", s.name);
            self.job(&title, script);
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
            self.read_job("Поиск встроенных приложений", Self::appx_check_script());
        }
        let installed = self.appx.iter().filter(|a| **a == Some(true)).count();
        let n = self.appx_sel.iter().filter(|s| **s).count();
        self.header(ui, "Удаление приложений", &format!("{installed} из {} предустановленных найдено", BLOAT.len()), |s, ui| {
            if style::primary_button(ui, &format!("{}  Удалить ({n})", ic::TRASH)).clicked() && n > 0 {
                let mut script = String::new();
                let sel: Vec<usize> = (0..BLOAT.len()).filter(|&i| s.appx_sel[i]).collect();
                for i in sel {
                    let (pkg, label) = BLOAT[i];
                    let q = sys::ps_quote(pkg);
                    script += &format!(
                        "'• {}'; Get-AppxPackage -AllUsers -Name {q} | Remove-AppxPackage -AllUsers -EA SilentlyContinue; Get-AppxProvisionedPackage -Online | ? DisplayName -eq {q} | Remove-AppxProvisionedPackage -Online -EA SilentlyContinue | Out-Null\n",
                        label.replace('\'', "''")
                    );
                    s.appx_sel[i] = false;
                }
                script += &Self::appx_check_script();
                s.job("Удаление приложений", script);
            }
            if style::button(ui, ic::LIST, "Выбрать все").clicked() {
                for i in 0..BLOAT.len() {
                    s.appx_sel[i] = s.appx[i] == Some(true) && !BLOAT[i].1.contains("Game Pass");
                }
            }
        });
        self.admin_banner(ui);
        let ids: Vec<usize> = (0..BLOAT.len()).collect();
        grid(ui, &ids, 82.0, 0, |ui, &i| {
            let (pkg, label) = BLOAT[i];
            let inst = self.appx[i];
            ui.horizontal(|ui| {
                style::icon_tile(ui, ic::PACKAGE, 34.0, if inst == Some(true) { TEXT } else { FAINT });
                ui.vertical(|ui| {
                    ui.label(RichText::new(label).font(FontId::new(14.0, bold())).color(if inst == Some(true) { TEXT } else { FAINT }));
                    ui.label(RichText::new(pkg).size(11.0).color(FAINT));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| match inst {
                    Some(true) => {
                        ui.checkbox(&mut self.appx_sel[i], "");
                    }
                    Some(false) => {
                        ui.label(RichText::new("нет").size(12.0).color(FAINT));
                    }
                    None => {
                        ui.spinner();
                    }
                });
            });
        }, None::<fn(&mut Ui)>);
    }

    fn about(&mut self, ui: &mut Ui) {
        self.header(ui, "О программе", &format!("SparkTweak v{} · MIT License", env!("CARGO_PKG_VERSION")), |_, _| {});
        glass().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                style::logo(ui, 64.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("SparkTweak").font(FontId::new(22.0, bold())));
                    ui.label(RichText::new("Оптимизация Windows на Rust. Каждый твик открыт и откатывается одной кнопкой.").color(DIM));
                });
            });
            ui.add_space(10.0);
            ui.label(format!("{}  Твиков и действий: {}", ic::LIST, TWEAKS.len()));
            ui.label(format!("{}  Приложений в списке удаления: {}", ic::PACKAGE, BLOAT.len()));
            ui.label(format!("{}  Перед профилями автоматически создаётся точка восстановления", ic::SHIELD));
            ui.add_space(6.0);
            ui.hyperlink_to(format!("{}  github.com/s0nd3r-py/SparkTweak", ic::GLOBE), "https://github.com/s0nd3r-py/SparkTweak");
        });
    }

    fn search_results(&mut self, ui: &mut Ui) {
        let q = self.search.to_lowercase();
        let ids: Vec<usize> = (0..TWEAKS.len()).filter(|&i| TWEAKS[i].name.to_lowercase().contains(&q) || TWEAKS[i].desc.to_lowercase().contains(&q)).collect();
        let search = self.search.clone();
        self.header(ui, "Поиск", &format!("«{search}» · найдено {}", ids.len()), |_, _| {});
        self.tweak_cards(ui, &ids);
    }

    /// Нижняя «таблетка» статуса с раскрывающимся журналом.
    fn activity(&mut self, ui: &mut Ui) {
        glass().inner_margin(egui::Margin::symmetric(14, 8)).corner_radius(14).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if self.busy > 0 {
                    ui.spinner();
                    ui.label(RichText::new(format!("Выполняется · в очереди {}", self.busy)).color(MINT).font(FontId::new(13.0, bold())));
                } else {
                    style::status(ui, "Готово", MINT);
                }
                if let Some(last) = self.log.last() {
                    ui.label(RichText::new(last).color(DIM).size(12.5));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let label = if self.log_open { format!("{}  Журнал", ic::CARET_DOWN) } else { format!("{}  Журнал", ic::CARET_UP) };
                    if ui.small_button(label).clicked() {
                        self.log_open = !self.log_open;
                    }
                    if self.log_open && ui.small_button(format!("{}  Копировать", ic::COPY)).clicked() {
                        ui.ctx().copy_text(self.log.join("\n"));
                    }
                });
            });
            if self.log_open {
                ui.add_space(4.0);
                ScrollArea::vertical().stick_to_bottom(true).max_height(180.0).auto_shrink([false, true]).show(ui, |ui| {
                    for l in &self.log {
                        let c = if l.starts_with('▶') {
                            TEXT
                        } else if l.contains('✓') {
                            MINT
                        } else if l.contains('✗') || l.contains("Error") || l.contains("Ошибка") {
                            RED
                        } else if l.contains('⚠') || l.contains('⟳') {
                            AMBER
                        } else {
                            DIM
                        };
                        ui.label(RichText::new(l).monospace().size(12.0).color(c));
                    }
                });
            }
        });
    }
}

// ─────────────── мелкие виджеты ───────────────

fn with_freed(body: &str) -> String {
    format!("$f0 = (Get-PSDrive C).Free\n{body}\n\"✓ Освобождено на C: {{0:N0}} МБ\" -f ([math]::Max(0, ((Get-PSDrive C).Free - $f0)) / 1MB)")
}

fn section(ui: &mut Ui, title: &str) {
    ui.label(RichText::new(title).font(FontId::new(17.0, bold())));
    ui.add_space(2.0);
}

/// Сетка карточек одинакового размера. `cols = 0` — по ширине окна.
fn grid<T>(ui: &mut Ui, items: &[T], h: f32, cols: usize, mut f: impl FnMut(&mut Ui, &T), extra: Option<impl FnOnce(&mut Ui)>) {
    let gap = 14.0;
    let w = ui.available_width();
    let cols = if cols > 0 { cols } else { ((w + gap) / (330.0 + gap)).floor().max(1.0) as usize };
    let cols_total = cols;
    let cw = (w - gap * (cols_total - 1) as f32) / cols_total as f32;
    let mut extra = extra;
    let n = items.len() + extra.is_some() as usize;
    for row in 0..n.div_ceil(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for c in 0..cols {
                let idx = row * cols + c;
                if idx >= n {
                    break;
                }
                ui.allocate_ui_with_layout(vec2(cw, h), Layout::top_down(Align::Min), |ui| {
                    if idx < items.len() {
                        glass().show(ui, |ui| {
                            ui.set_width(cw - 32.0);
                            ui.set_height(h - 32.0);
                            f(ui, &items[idx]);
                        });
                    } else if let Some(e) = extra.take() {
                        // пунктирная «пустая» карточка
                        let (r, _) = ui.allocate_exact_size(vec2(cw, h), Sense::hover());
                        ui.painter().rect_filled(r, 18.0, white(14));
                        ui.painter().rect_stroke(r, 18.0, Stroke::new(1.0, white(34)), egui::StrokeKind::Inside);
                        ui.scope_builder(egui::UiBuilder::new().max_rect(r.shrink(16.0)), e);
                    }
                });
            }
        });
        ui.add_space(gap - 8.0);
    }
}

/// Верх карточки: плитка-иконка, название, подпись, статус справа.
fn card_head(ui: &mut Ui, icon: &str, title: &str, sub: &str, st: (&str, Color32)) {
    // в узкой карточке статус уходит под заголовок, иначе название рвётся по буквам
    let narrow = ui.available_width() < 320.0;
    ui.horizontal(|ui| {
        style::icon_tile(ui, icon, 36.0, TEXT);
        let w = ui.available_width() - if narrow { 0.0 } else { 118.0 };
        ui.vertical(|ui| {
            ui.set_max_width(w);
            ui.add(egui::Label::new(RichText::new(title).font(FontId::new(15.5, bold()))).wrap());
            if !sub.is_empty() {
                ui.label(RichText::new(sub).color(DIM).size(12.5));
            }
            if narrow {
                style::status(ui, st.0, st.1);
            }
        });
        if !narrow {
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| style::status(ui, st.0, st.1));
        }
    });
}

/// Низ карточки: разделитель и кнопки справа, прижатые к нижнему краю.
fn card_foot(ui: &mut Ui, f: impl FnOnce(&mut Ui)) {
    ui.with_layout(Layout::bottom_up(Align::Max), |ui| {
        ui.allocate_ui_with_layout(vec2(ui.available_width(), 30.0), Layout::right_to_left(Align::Center), f);
        let y = ui.min_rect().top() - 6.0;
        ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0, white(22)));
    });
}

fn stat(ui: &mut Ui, icon: &str, label: &str, value: &str, sub: &str) {
    glass().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            style::icon_tile(ui, icon, 40.0, TEXT);
            ui.vertical(|ui| {
                ui.label(RichText::new(label).size(12.5).color(DIM).font(FontId::new(12.5, bold())));
                ui.add_space(-6.0);
                ui.label(RichText::new(value).font(FontId::new(22.0, bold())));
                ui.add_space(-6.0);
                ui.label(RichText::new(sub).size(12.0).color(DIM));
            });
        });
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
    p.text(center + vec2(0.0, 16.0), egui::Align2::CENTER_CENTER, "занято", FontId::proportional(11.5), DIM);
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
            (false, _) => "Поиск",
            (_, Page::Home) => "Обзор",
            (_, Page::Cat(c)) => CATS.iter().find(|x| x.0 == c).map_or("", |x| x.2),
            (_, Page::Startup) => "Автозагрузка",
            (_, Page::Debloat) => "Приложения",
            (_, Page::About) => "О программе",
        };
        let title = Rect::from_min_size(full.min, vec2(full.width(), 34.0));
        style::title_bar(ui, title, "SparkTweak", &format!("{section} · {}", env!("CARGO_PKG_VERSION")));

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
