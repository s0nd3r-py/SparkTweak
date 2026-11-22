//! Графитовый «liquid glass» стиль: матовое стекло, контурные иконки, мятный акцент.
//! Общий для приложения и установщика.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Mesh, Pos2, Rect,
    Response, RichText, Sense, Shadow, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};
use std::sync::Arc;

pub const MINT: Color32 = Color32::from_rgb(0xA3, 0xE9, 0xC9);
pub const MINT_DARK: Color32 = Color32::from_rgb(0x0E, 0x2B, 0x20);
pub const CYAN: Color32 = Color32::from_rgb(0x8E, 0xD4, 0xF5);
pub const AMBER: Color32 = Color32::from_rgb(0xF5, 0xC8, 0x6B);
pub const RED: Color32 = Color32::from_rgb(0xF2, 0x8B, 0x82);
pub const VIOLET: Color32 = Color32::from_rgb(0xC3, 0xB1, 0xF7);
pub const TEXT: Color32 = Color32::from_rgb(0xF3, 0xF3, 0xF4);
pub const DIM: Color32 = Color32::from_rgb(0xA6, 0xA8, 0xAD);
pub const FAINT: Color32 = Color32::from_rgb(0x7C, 0x7F, 0x86);

/// Контурные иконки Phosphor (шрифт лежит в assets/).
pub mod ic {
    pub const BOLT: &str = "\u{E2DE}";
    pub const ROCKET: &str = "\u{E3FE}";
    pub const CPU: &str = "\u{E610}";
    pub const GAME: &str = "\u{E26E}";
    pub const GLOBE: &str = "\u{E288}";
    pub const SHIELD_CHECK: &str = "\u{E40C}";
    pub const TERMINAL: &str = "\u{EAE8}";
    pub const PALETTE: &str = "\u{E6C8}";
    pub const GEAR: &str = "\u{E272}";
    pub const BROOM: &str = "\u{EC54}";
    pub const WRENCH: &str = "\u{E5D4}";
    pub const TOOLBOX: &str = "\u{ECA0}";
    pub const POWER: &str = "\u{E3DA}";
    pub const TRASH: &str = "\u{E4A6}";
    pub const INFO: &str = "\u{E2CE}";
    pub const GRID: &str = "\u{E464}";
    pub const SEARCH: &str = "\u{E30C}";
    pub const MEMORY: &str = "\u{E9C4}";
    pub const UNDO: &str = "\u{E038}";
    pub const PLAY: &str = "\u{E3D0}";
    pub const CHECK: &str = "\u{E182}";
    pub const CLOCK: &str = "\u{E19A}";
    pub const DRIVES: &str = "\u{E2A0}";
    pub const SPARKLE: &str = "\u{E6A2}";
    pub const FLAME: &str = "\u{E624}";
    pub const LOCK: &str = "\u{E2FE}";
    pub const DESKTOP: &str = "\u{E560}";
    pub const REFRESH: &str = "\u{E094}";
    pub const LIST: &str = "\u{EADC}";
    pub const CARET_DOWN: &str = "\u{E136}";
    pub const CARET_UP: &str = "\u{E13C}";
    pub const COPY: &str = "\u{E1CA}";
    pub const WARNING: &str = "\u{E4E0}";
    pub const SHIELD: &str = "\u{E40A}";
    pub const GAUGE: &str = "\u{E628}";
    pub const DOWNLOAD: &str = "\u{E20C}";
    pub const PACKAGE: &str = "\u{E390}";
    pub const STACK: &str = "\u{E466}";
    pub const APP: &str = "\u{E5DA}";
}

pub const fn white(a: u8) -> Color32 {
    Color32::from_rgba_premultiplied(a, a, a, a)
}

pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}

pub fn setup(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("phosphor".into(), Arc::new(FontData::from_static(include_bytes!("../assets/Phosphor.ttf"))));
    let load = |f: &str| std::fs::read(format!(r"C:\Windows\Fonts\{f}")).ok();
    let mut b = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    if let (Some(reg), Some(sb)) = (load("segoeui.ttf"), load("seguisb.ttf")) {
        fonts.font_data.insert("segoe".into(), Arc::new(FontData::from_owned(reg)));
        fonts.font_data.insert("segoe-sb".into(), Arc::new(FontData::from_owned(sb)));
        fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "segoe".into());
        b.insert(0, "segoe-sb".into());
    }
    // Phosphor раньше встроенных шрифтов egui: emoji-icon-font занимает те же коды в Private Use Area
    let prop = fonts.families.get_mut(&FontFamily::Proportional).unwrap();
    let at = prop.iter().position(|f| f != "segoe").unwrap_or(0);
    prop.insert(at, "phosphor".into());
    let at = b.iter().position(|f| f != "segoe-sb").unwrap_or(0);
    b.insert(at, "phosphor".into());
    fonts.families.insert(bold(), b);
    fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "phosphor".into());
    ctx.set_fonts(fonts);

    let mut v = egui::Visuals::dark();
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = Color32::from_rgb(0x2A, 0x2C, 0x30);
    v.window_stroke = Stroke::new(1.0, white(30));
    v.extreme_bg_color = Color32::from_rgba_premultiplied(8, 9, 10, 150);
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = MINT.gamma_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, MINT);
    v.hyperlink_color = MINT;
    for (w, a) in [
        (&mut v.widgets.inactive, 12u8),
        (&mut v.widgets.hovered, 24),
        (&mut v.widgets.active, 36),
        (&mut v.widgets.open, 24),
    ] {
        w.bg_fill = white(a);
        w.weak_bg_fill = white(a);
        w.corner_radius = CornerRadius::same(10);
        w.bg_stroke = Stroke::new(1.0, white(a + 22));
        w.fg_stroke.color = TEXT;
        w.expansion = 0.0;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, white(20));
    v.widgets.noninteractive.fg_stroke.color = TEXT;
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = vec2(10.0, 8.0);
        s.spacing.button_padding = vec2(12.0, 6.0);
        s.spacing.scroll.floating = true;
        s.interaction.selectable_labels = false;
        s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(14.0));
        s.text_styles.insert(egui::TextStyle::Button, FontId::new(14.0, bold()));
        s.text_styles.insert(egui::TextStyle::Small, FontId::proportional(12.5));
        s.text_styles.insert(egui::TextStyle::Heading, FontId::new(30.0, bold()));
    });
}

/// Фон: графитовое стекло. Без `solid` полупрозрачно — сквозь него видно размытый (acrylic) рабочий стол.
pub fn paint_background(ui: &Ui, rect: Rect, solid: bool) {
    let p = ui.painter();
    let a = if solid { 255 } else { 150 };
    let c1 = Color32::from_rgba_unmultiplied(46, 48, 52, a);
    let c2 = Color32::from_rgba_unmultiplied(30, 32, 35, a);
    let mut m = Mesh::default();
    m.colored_vertex(rect.left_top(), c1);
    m.colored_vertex(rect.right_top(), c2);
    m.colored_vertex(rect.right_bottom(), c2);
    m.colored_vertex(rect.left_bottom(), c1);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(m);
    // мягкие световые пятна, как отражения в стекле
    blob(ui, rect.center() + vec2(rect.width() * 0.08, rect.height() * 0.12), rect.height() * 0.48, white(9));
    blob(ui, rect.left_bottom() + vec2(rect.width() * 0.12, -rect.height() * 0.3), rect.height() * 0.35, Color32::from_rgba_unmultiplied(200, 120, 40, 14));
    blob(ui, rect.right_top() + vec2(-rect.width() * 0.15, 60.0), 260.0, Color32::from_rgba_unmultiplied(120, 200, 170, 8));
}

/// Радиальный градиент веером треугольников — имитация размытого света.
pub fn blob(ui: &Ui, c: Pos2, r: f32, color: Color32) {
    let mut m = Mesh::default();
    m.colored_vertex(c, color);
    let n = 72;
    for i in 0..=n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        m.colored_vertex(c + vec2(a.cos(), a.sin()) * r, Color32::TRANSPARENT);
    }
    for i in 1..=n {
        m.add_triangle(0, i, i + 1);
    }
    ui.painter().add(m);
}

/// Стеклянная карточка.
pub fn glass() -> egui::Frame {
    egui::Frame::new()
        .fill(white(9))
        .stroke(Stroke::new(1.0, white(28)))
        .corner_radius(CornerRadius::same(18))
        .inner_margin(Margin::same(16))
        .shadow(Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(45) })
}

/// Карточка с цветной подсветкой (включённый твик, профиль).
pub fn glass_accent(c: Color32) -> egui::Frame {
    glass().fill(c.gamma_multiply(0.07)).stroke(Stroke::new(1.0, c.gamma_multiply(0.45)))
}

/// Квадратная «плитка» с иконкой, как у карточек в Nitro.
pub fn icon_tile(ui: &mut Ui, icon: &str, size: f32, color: Color32) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 10.0, white(12));
    p.rect_stroke(r, 10.0, Stroke::new(1.0, white(26)), StrokeKind::Inside);
    p.text(r.center(), Align2::CENTER_CENTER, icon, FontId::proportional(size * 0.5), color);
}

/// Таблетка статуса: «● Включено».
pub fn status(ui: &mut Ui, text: &str, c: Color32) {
    egui::Frame::new()
        .fill(white(12))
        .stroke(Stroke::new(1.0, white(22)))
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin::symmetric(9, 3))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(6.0, 12.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 3.0, c);
                ui.label(RichText::new(text).size(12.0).color(if c == FAINT { DIM } else { c }));
            });
        });
}

pub fn chip(ui: &mut Ui, text: &str, c: Color32) {
    egui::Frame::new()
        .fill(c.gamma_multiply(0.10))
        .stroke(Stroke::new(1.0, c.gamma_multiply(0.35)))
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin::symmetric(7, 1))
        .show(ui, |ui| ui.label(RichText::new(text).size(11.0).color(c)));
}

/// Мятная главная кнопка с тёмным текстом.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    let galley = ui.painter().layout_no_wrap(text.into(), FontId::new(14.0, bold()), MINT_DARK);
    let size = galley.size() + vec2(30.0, 16.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let fill = if resp.is_pointer_button_down_on() {
            MINT.gamma_multiply(0.8)
        } else if resp.hovered() {
            Color32::from_rgb(0xB8, 0xF2, 0xD7)
        } else {
            MINT
        };
        p.rect_filled(rect, 11.0, fill);
        p.rect_stroke(rect, 11.0, Stroke::new(1.0, Color32::from_white_alpha(90)), StrokeKind::Inside);
        p.galley(rect.center() - galley.size() / 2.0, galley, MINT_DARK);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Вторичная стеклянная кнопка с иконкой.
pub fn button(ui: &mut Ui, icon: &str, text: &str) -> Response {
    let label = if icon.is_empty() { text.to_string() } else { format!("{icon}  {text}") };
    ui.add(egui::Button::new(RichText::new(label).font(FontId::new(13.5, bold()))).corner_radius(11))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Анимированный переключатель.
pub fn toggle(ui: &mut Ui, on: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(44.0, 24.0), Sense::click());
    let t = ui.ctx().animate_bool(resp.id, on);
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let r = rect.height() / 2.0;
        p.rect_filled(rect, r, lerp_color(white(if resp.hovered() { 34 } else { 22 }), MINT, t));
        p.rect_stroke(rect, r, Stroke::new(1.0, white(36)), StrokeKind::Inside);
        let x = egui::lerp((rect.left() + r)..=(rect.right() - r), t);
        p.circle_filled(pos2(x, rect.center().y), r - 4.0, lerp_color(TEXT, MINT_DARK, t));
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// Заголовок окна: «светофор» слева и путь «SparkTweak / раздел · версия».
pub fn title_bar(ui: &mut Ui, rect: Rect, product: &str, section: &str) {
    let resp = ui.interact(rect, ui.scope_id().with("title"), Sense::click_and_drag());
    if resp.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
    if resp.double_clicked() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }
    let cy = rect.center().y;
    let dots = [
        (Color32::from_rgb(0xFF, 0x5F, 0x57), "×", egui::ViewportCommand::Close),
        (Color32::from_rgb(0xFE, 0xBC, 0x2E), "−", egui::ViewportCommand::Minimized(true)),
        (Color32::from_rgb(0x28, 0xC8, 0x40), "+", egui::ViewportCommand::Maximized(!maximized)),
    ];
    let group = Rect::from_min_size(pos2(rect.left() + 12.0, cy - 9.0), vec2(64.0, 18.0));
    let group_hover = ui.rect_contains_pointer(group);
    for (i, (c, glyph, cmd)) in dots.into_iter().enumerate() {
        let center = pos2(rect.left() + 21.0 + i as f32 * 20.0, cy);
        let r = Rect::from_center_size(center, Vec2::splat(16.0));
        let b = ui.interact(r, ui.scope_id().with(("dot", i)), Sense::click());
        ui.painter().circle_filled(center, 6.0, c);
        if group_hover {
            ui.painter().text(center, Align2::CENTER_CENTER, glyph, FontId::new(11.0, bold()), Color32::from_black_alpha(150));
        }
        if b.clicked() {
            ui.ctx().send_viewport_cmd(cmd);
        }
    }
    let job = {
        let mut j = egui::text::LayoutJob::default();
        let f = FontId::new(13.0, bold());
        j.append(product, 0.0, egui::TextFormat::simple(f.clone(), TEXT));
        j.append(&format!(" / {section}"), 0.0, egui::TextFormat::simple(f, DIM));
        j
    };
    let g = ui.painter().layout_job(job);
    ui.painter().galley(pos2(rect.left() + 84.0, cy - g.size().y / 2.0), g, TEXT);
    ui.painter().hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, white(16)));
}

/// Иконка окна: белый скруглённый квадрат с тёмной молнией.
pub fn icon() -> egui::IconData {
    let n = 64usize;
    let mut rgba = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 - 31.5, y as f32 - 31.5);
            // скруглённый квадрат: superellipse
            let inside = (fx.abs() / 30.0).powi(5) + (fy.abs() / 30.0).powi(5) < 1.0;
            if !inside {
                continue;
            }
            let (u, v) = (fx / 30.0, fy / 30.0);
            let bolt = (u - v * 0.35 - if v < 0.0 { 0.1 } else { -0.1 }).abs() < 0.14 && v.abs() < 0.62;
            let c = if bolt { [0x12, 0x14, 0x16] } else { [0xF4, 0xF4, 0xF5] };
            rgba[(y * n + x) * 4..][..4].copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    egui::IconData { rgba, width: n as u32, height: n as u32 }
}

/// Логотип в стиле иконки приложения (для боковой панели и установщика).
pub fn logo(ui: &mut Ui, size: f32) -> Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let p = ui.painter();
    p.rect_filled(r, size * 0.28, Color32::from_rgb(0xF4, 0xF4, 0xF5));
    p.text(r.center(), Align2::CENTER_CENTER, ic::BOLT, FontId::proportional(size * 0.56), Color32::from_rgb(0x12, 0x14, 0x16));
    resp
}
