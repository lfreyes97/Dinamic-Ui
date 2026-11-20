//! Panel de control: pestañas, helpers y constantes propias.

use super::*;

// ── Panel de control ────────────────────────────────────────────────────
// Los colores del panel son constantes propias (grises fijos), no la paleta base16.
pub(super) const P_SURFACE: Color32 = Color32::from_gray(0x15);
pub(super) const P_BORDER: Color32 = Color32::from_gray(0x29);
pub(super) const P_TEXT: Color32 = Color32::from_gray(0xf5);
pub(super) const P_MUTED: Color32 = Color32::from_gray(0x77);
pub(super) const P_TABBAR: Color32 = Color32::from_gray(0x10);
pub(super) const P_HEADING: Color32 = Color32::from_gray(0xaa);
pub(super) const P_DIM: Color32 = Color32::from_gray(0x66);
pub(super) const P_SOFT: Color32 = Color32::from_gray(0xcc);
pub(super) const P_INVERT: Color32 = Color32::from_gray(0xee);
pub(super) const P_INK: Color32 = Color32::from_gray(0x11);
pub(super) const P_ICON_BOX: Color32 = Color32::from_gray(0x25);
pub(super) const P_TRACK: Color32 = Color32::from_gray(0x30);
pub(super) const P_OK: Color32 = Color32::from_rgb(0x8b, 0xe0, 0x9b);

pub(super) const PANEL_TABS: [&str; 5] = ["Workspace", "Clock", "System", "Wallpaper", "Alertas"];

pub(super) const DAYS_LONG: [&str; 7] = [
    "lunes",
    "martes",
    "miércoles",
    "jueves",
    "viernes",
    "sábado",
    "domingo",
];
pub(super) const MONTHS_LONG: [&str; 12] = [
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];

/// Presets de layout: nombre + forma de la mini-vista previa.
const LAYOUT_PRESETS: [&str; 4] = ["Dwindle", "Master", "Scrolling", "Master Stack"];
/// Valor de `general:layout` para cada preset. "Master Stack" no tiene un
/// layout propio en Hyprland: queda sin aplicar.
const LAYOUT_IDS: [Option<&str>; 4] = [Some("dwindle"), Some("master"), Some("scrolling"), None];

/// Comando que cambia `general:layout` en tiempo de ejecución (config Lua).
pub(super) fn layout_command(id: &str) -> Vec<String> {
    vec![
        "hyprctl".to_string(),
        "eval".to_string(),
        format!("hl.config({{ general = {{ layout = \"{id}\" }} }})"),
    ]
}

/// Índice del preset que corresponde a un valor de `general:layout`.
pub fn layout_preset_index(layout: &str) -> Option<usize> {
    LAYOUT_IDS.iter().position(|id| *id == Some(layout))
}

/// Abre una ruta con la aplicación por defecto (vía `main`, sin bloquear).
pub(super) fn open_path(state: &mut UiState, path: &std::path::Path) {
    state.pending_spawns.push(vec![
        "xdg-open".to_string(),
        path.to_string_lossy().into_owned(),
    ]);
}

pub(super) fn draw_panel(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let width = ui.available_width();
    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

    // Cabecera: título + acciones redondas (ajustes, salir y volver a la isla).
    let (header, _) = ui.allocate_exact_size(egui::Vec2::new(width, 24.0), egui::Sense::hover());
    text_at(
        ui,
        egui::Pos2::new(header.min.x, header.center().y),
        egui::Align2::LEFT_CENTER,
        "Control Center",
        TEXT_DISPLAY_SM,
        P_TEXT,
    );
    let button = egui::Vec2::splat(header.height());
    let slot = |i: f32| {
        egui::Rect::from_min_size(
            egui::Pos2::new(header.max.x - button.x - i * (button.x + 7.0), header.min.y),
            button,
        )
    };
    if round_icon_button_at(ui, slot(0.0), "panel-gear", ic::GEAR).clicked() {
        // Comportamiento previo: el engranaje lleva a Alertas (no hay vista de ajustes).
        state.bar.panel_tab = 4;
    }
    if round_icon_button_at(ui, slot(1.0), "panel-power", ic::POWER).clicked() {
        state.should_exit = true;
    }
    if round_icon_button_at(ui, slot(2.0), "panel-back", ic::ARROW_LEFT).clicked() {
        state.bar.expanded = false;
        state.bar.show_panel = false;
    }
    ui.add_space(SP_L);

    // Barra de pestañas: fondo #101010, activa #292929.
    let (bar, _) = ui.allocate_exact_size(egui::Vec2::new(width, 26.0), egui::Sense::hover());
    ui.painter().rect(
        bar,
        CornerRadius::same(10),
        P_TABBAR,
        Stroke::new(1.0, P_BORDER),
        StrokeKind::Inside,
    );
    let inner = bar.shrink(3.0);
    let gap = 3.0;
    let tab_w = (inner.width() - gap * (PANEL_TABS.len() as f32 - 1.0)) / PANEL_TABS.len() as f32;
    for (i, label) in PANEL_TABS.iter().enumerate() {
        let rect = egui::Rect::from_min_size(
            egui::Pos2::new(inner.min.x + i as f32 * (tab_w + gap), inner.min.y),
            egui::Vec2::new(tab_w, inner.height()),
        );
        let response = ui
            .interact(
                rect,
                ui.scope_id().with(("panel-tab", i)),
                egui::Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        let active = i == state.bar.panel_tab;
        if active {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(7), P_BORDER);
        }
        let color = if active || response.hovered() {
            P_TEXT
        } else {
            P_MUTED
        };
        text_at(
            ui,
            rect.center(),
            egui::Align2::CENTER_CENTER,
            *label,
            TEXT_BODY,
            color,
        );
        if response.clicked() {
            state.bar.panel_tab = i;
        }
    }
    ui.add_space(SP_L);

    // Con panel_h = 300 quedan ~216 px para la página: Workspace cabe entera;
    // Clock, System, Wallpaper y Alertas son más altas y hacen scroll.
    // La página ocupa el resto de la tarjeta y se extiende hacia el padding
    // derecho para que la barra de scroll (sólida, fina) no tape el contenido:
    // el contenido mantiene el mismo ancho que la cabecera.
    let rest = ui.available_rect_before_wrap();
    let page_rect = egui::Rect::from_min_max(
        rest.min,
        egui::Pos2::new(rest.max.x + PANEL_SCROLLBAR, rest.max.y.max(rest.min.y)),
    );
    let mut page = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(page_rect)
            .layout(egui::Layout::top_down(egui::Align::Min))
            .id_salt("panel-page-area"),
    );
    page.spacing_mut().scroll = egui::style::ScrollStyle {
        bar_width: 4.0,
        bar_inner_margin: PANEL_SCROLLBAR - 4.0,
        bar_outer_margin: 0.0,
        // Visible aunque no se pase el ratón: indica que hay más contenido.
        dormant_handle_opacity: 0.35,
        ..egui::style::ScrollStyle::solid()
    };
    let tab = state.bar.panel_tab.min(PANEL_TABS.len() - 1);
    egui::ScrollArea::vertical()
        .id_salt(("panel-page", tab))
        .auto_shrink([false, false])
        .show(&mut page, |ui| {
            // Mismo ancho con o sin barra de scroll visible.
            ui.set_max_width(width);
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            match tab {
                0 => panel_workspaces(ui, state, palette),
                1 => panel_clock(ui, state),
                2 => panel_system(ui, state),
                3 => panel_wallpaper(ui, state),
                _ => panel_notifications(ui, state),
            }
        });
}

// ── Piezas comunes del panel ────────────────────────────────────────────
// ── Workspace ───────────────────────────────────────────────────────────

pub(super) fn panel_workspaces(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let count = config::get().workspaces.count as usize;
    section_heading(ui, "Workspaces", &format!("{count} disponibles"));

    // Rejilla de 5 columnas; el activo va invertido (#eee sobre #111).
    // Medidas para que la pestaña entera quepa en panel_h = 300 sin scroll
    // (con 10 workspaces): 2 filas de 34 px + 4 presets en una sola fila.
    let columns = 5usize;
    let gap = 4.0;
    let cell_h = 34.0;
    let rows = count.div_ceil(columns);
    let width = ui.available_width();
    let cell_w = (width - gap * (columns as f32 - 1.0)) / columns as f32;
    let (grid, _) = ui.allocate_exact_size(
        egui::Vec2::new(width, rows as f32 * cell_h + (rows as f32 - 1.0) * gap),
        egui::Sense::hover(),
    );
    for i in 0..count {
        let n = i + 1;
        let rect = egui::Rect::from_min_size(
            grid.min
                + egui::Vec2::new(
                    (i % columns) as f32 * (cell_w + gap),
                    (i / columns) as f32 * (cell_h + gap),
                ),
            egui::Vec2::new(cell_w, cell_h),
        );
        let response = ui
            .interact(
                rect,
                ui.scope_id().with(("panel-ws", n)),
                egui::Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        let active = state.snapshot.workspace == n.to_string();
        let occupied = state.snapshot.occupied.contains(&(n as u32));
        let (fill, stroke, fg) = if active {
            (P_INVERT, P_INVERT, P_INK)
        } else if response.hovered() {
            (P_SURFACE, Color32::from_gray(0x44), P_SOFT)
        } else {
            (P_SURFACE, P_BORDER, P_SOFT)
        };
        ui.painter().rect(
            rect,
            CornerRadius::same(10),
            fill,
            Stroke::new(1.0, stroke),
            StrokeKind::Inside,
        );
        let c = rect.center();
        text_at(
            ui,
            c - egui::Vec2::new(0.0, 5.0),
            egui::Align2::CENTER_CENTER,
            n,
            TEXT_DISPLAY_SM,
            fg,
        );
        text_at(
            ui,
            c + egui::Vec2::new(0.0, 9.0),
            egui::Align2::CENTER_CENTER,
            if active {
                "ACTIVO"
            } else if occupied {
                "OCUPADO"
            } else {
                "WS"
            },
            TEXT_CAPTION - 1.0,
            if occupied && !active {
                palette.accent
            } else {
                fg.gamma_multiply(0.65)
            },
        );
        if occupied {
            ui.painter().circle_filled(
                rect.max - egui::Vec2::new(10.0, rect.height() - 10.0),
                3.0,
                occupied_color(palette, active),
            );
        }
        if response.clicked() {
            state.pending_commands.push(switch_workspace_cmd(n as u32));
        }
    }
    ui.add_space(SP_L);

    // Placeholder: crear presets propios no está implementado.
    let _ = section_heading_action(ui, "Layout presets", ic::PLUS)
        .on_hover_text("Sin implementar: no hay presets propios");

    // Una fila de 4 tarjetas (antes 2×2 de 84 px): el activo se marca con
    // relleno y borde, sin icono de check (no cabe junto a "Master Stack").
    let gap = 6.0;
    let card_h = 52.0;
    let columns = LAYOUT_PRESETS.len();
    let width = ui.available_width();
    let card_w = (width - gap * (columns as f32 - 1.0)) / columns as f32;
    let (grid, _) = ui.allocate_exact_size(egui::Vec2::new(width, card_h), egui::Sense::hover());
    for (i, name) in LAYOUT_PRESETS.iter().enumerate() {
        let rect = egui::Rect::from_min_size(
            grid.min + egui::Vec2::new(i as f32 * (card_w + gap), 0.0),
            egui::Vec2::new(card_w, card_h),
        );
        let layout_id = LAYOUT_IDS[i];
        let mut response = ui.interact(
            rect,
            ui.scope_id().with(("panel-preset", i)),
            egui::Sense::click(),
        );
        response = match layout_id {
            Some(id) => response
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(format!("general.layout = \"{id}\"")),
            None => response.on_hover_text("Sin equivalente en Hyprland"),
        };
        let active = state.bar.layout_preset == i;
        let (fill, stroke) = if active {
            (P_BORDER, P_MUTED)
        } else {
            (P_SURFACE, P_BORDER)
        };
        ui.painter().rect(
            rect,
            CornerRadius::same(10),
            fill,
            Stroke::new(1.0, stroke),
            StrokeKind::Inside,
        );
        let inner = rect.shrink(SP_L);
        let name_color = if active { P_TEXT } else { P_SOFT };
        text_at(
            ui,
            egui::Pos2::new(inner.min.x, inner.min.y + 6.0),
            egui::Align2::LEFT_CENTER,
            *name,
            TEXT_BODY,
            name_color,
        );
        let preview =
            egui::Rect::from_min_max(egui::Pos2::new(inner.min.x, inner.min.y + 15.0), inner.max);
        paint_layout_preview(ui, preview, i);
        if response.clicked() {
            // `general:layout` es una opción de config, no un dispatcher. Con la
            // config en Lua de Hyprland 0.57 `hyprctl keyword` ya no existe
            // ("unknown request"): se aplica con `hyprctl eval` (proceso aparte,
            // lanzado por `main`). Hyprland acepta cualquier cadena sin validarla,
            // por eso solo se envían ids de `LAYOUT_IDS`.
            if let Some(id) = layout_id {
                state.bar.layout_preset = i;
                state.pending_spawns.push(layout_command(id));
            }
        }
    }

    let ws = if state.snapshot.workspace.is_empty() {
        "-"
    } else {
        &state.snapshot.workspace
    };
    // Layout real de Hyprland si se pudo leer; si no, el preset marcado.
    let layout = match &state.system.layout {
        Some(layout) => capitalize(layout),
        None => LAYOUT_PRESETS[state.bar.layout_preset.min(3)].to_string(),
    };
    let footer = format!("Workspace {ws} · {layout}");
    // Placeholder: no hay vista de ajustes de workspaces.
    let _ = panel_footer(ui, &footer, ic::GEAR, true, false);
}

/// Mini-vista previa de cada preset.
pub(super) fn paint_layout_preview(ui: &egui::Ui, rect: egui::Rect, preset: usize) {
    let dark = Color32::from_gray(0x66);
    let light = Color32::from_gray(0x99);
    let gap = 3.0;
    let mut blocks: Vec<(egui::Rect, Color32)> = Vec::new();
    match preset {
        // Dwindle y Master comparten la misma mini-vista.
        0 | 1 => {
            let cols = split(rect, &[2.0, 1.0], gap, true);
            blocks.push((cols[0], light));
            let rows = split(cols[1], &[1.0, 1.0], gap, false);
            blocks.push((rows[0], dark));
            blocks.push((rows[1], light));
        }
        2 => {
            for (i, col) in split(rect, &[1.0; 4], gap, true).into_iter().enumerate() {
                blocks.push((col, if i % 2 == 0 { light } else { dark }));
            }
        }
        _ => {
            let cols = split(rect, &[1.3, 1.0], gap, true);
            blocks.push((cols[0], light));
            for (i, row) in split(cols[1], &[1.0; 3], gap, false)
                .into_iter()
                .enumerate()
            {
                blocks.push((row, if i == 1 { light } else { dark }));
            }
        }
    }
    for (block, color) in blocks {
        ui.painter()
            .rect_filled(block, CornerRadius::same(3), color);
    }
}

// ── Clock ───────────────────────────────────────────────────────────────

pub(super) fn panel_clock(ui: &mut egui::Ui, state: &mut UiState) {
    let now = Local::now();
    let width = ui.available_width();
    let weekday = DAYS_LONG[now.weekday().num_days_from_monday() as usize];
    let month = MONTHS_LONG[(now.month() - 1) as usize];

    ui.add_space(SP_S);
    let (time, _) = ui.allocate_exact_size(egui::Vec2::new(width, 38.0), egui::Sense::hover());
    text_at(
        ui,
        time.center(),
        egui::Align2::CENTER_CENTER,
        now.format("%H:%M:%S"),
        TEXT_HERO,
        P_TEXT,
    );
    let (date, _) = ui.allocate_exact_size(egui::Vec2::new(width, 18.0), egui::Sense::hover());
    text_at(
        ui,
        date.center(),
        egui::Align2::CENTER_CENTER,
        format!("{}, {} de {month}", capitalize(weekday), now.day()),
        TEXT_CLOCK,
        Color32::from_gray(0xbb),
    );
    let (zone, _) = ui.allocate_exact_size(egui::Vec2::new(width, 18.0), egui::Sense::hover());
    let offset_h = now.offset().local_minus_utc() as f32 / 3600.0;
    text_at(
        ui,
        zone.center(),
        egui::Align2::CENTER_CENTER,
        format!("{} · UTC{}", timezone_name(), format_offset(offset_h)),
        TEXT_BODY,
        P_MUTED,
    );
    divider(ui);

    // Calendario del mes actual con navegación ‹ › (el desplazamiento vive en
    // la memoria de egui, no en `UiState`).
    let offset_id = ui.scope_id().with("panel-calendar-offset");
    let mut month_offset: i32 = ui.data(|d| d.get_temp(offset_id)).unwrap_or(0);
    let total = now.year() * 12 + now.month0() as i32 + month_offset;
    let (year, month0) = (total.div_euclid(12), total.rem_euclid(12) as u32);

    section_heading(ui, "Calendario", &year.to_string());
    let (header, _) = ui.allocate_exact_size(egui::Vec2::new(width, 20.0), egui::Sense::hover());
    text_at(
        ui,
        egui::Pos2::new(header.min.x, header.center().y),
        egui::Align2::LEFT_CENTER,
        format!("{} {year}", capitalize(MONTHS_LONG[month0 as usize])),
        TEXT_TITLE,
        P_SOFT,
    );
    for (i, (icon, step)) in [(ic::CARET_RIGHT, 1), (ic::CARET_LEFT, -1)]
        .into_iter()
        .enumerate()
    {
        let rect = egui::Rect::from_center_size(
            egui::Pos2::new(header.max.x - 8.0 - i as f32 * 26.0, header.center().y),
            egui::Vec2::splat(20.0),
        );
        let response = ui
            .interact(
                rect,
                ui.scope_id().with(("panel-month", i)),
                egui::Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        let color = if response.hovered() {
            P_TEXT
        } else {
            Color32::from_gray(0x99)
        };
        icon_at(ui, rect.center(), icon, ICON_SM, color);
        if response.clicked() {
            month_offset += step;
            ui.data_mut(|d| d.insert_temp(offset_id, month_offset));
        }
    }
    ui.add_space(SP_L);

    let first = chrono::NaiveDate::from_ymd_opt(year, month0 + 1, 1).unwrap_or_default();
    let lead = first.weekday().num_days_from_monday() as usize;
    let days = days_in_month(year, month0 + 1);
    let today = (month_offset == 0).then(|| now.day());

    let gap = 2.0;
    let cell_w = (width - gap * 6.0) / 7.0;
    let row_h = 20.0;
    let rows = (lead + days as usize).div_ceil(7);
    let (grid, _) = ui.allocate_exact_size(
        egui::Vec2::new(width, row_h * (rows + 1) as f32 + gap * rows as f32),
        egui::Sense::hover(),
    );
    let cell = |slot: usize| {
        egui::Rect::from_min_size(
            grid.min
                + egui::Vec2::new(
                    (slot % 7) as f32 * (cell_w + gap),
                    (slot / 7) as f32 * (row_h + gap),
                ),
            egui::Vec2::new(cell_w, row_h),
        )
    };
    for (i, day) in ["L", "M", "X", "J", "V", "S", "D"].iter().enumerate() {
        text_at(
            ui,
            cell(i).center(),
            egui::Align2::CENTER_CENTER,
            *day,
            TEXT_CAPTION,
            P_MUTED,
        );
    }
    for day in 1..=days {
        let rect = cell(7 + lead + day as usize - 1);
        let is_today = today == Some(day);
        if is_today {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(6), P_INVERT);
        }
        let color = if is_today { P_INK } else { P_SOFT };
        text_at(
            ui,
            rect.center(),
            egui::Align2::CENTER_CENTER,
            day,
            TEXT_BODY,
            color,
        );
    }
    divider(ui);

    // Placeholder: no hay sistema de alarmas (nada las haría sonar); filas de
    // ejemplo marcadas como tales.
    let _ = section_heading_action(ui, "Alarmas", ic::PLUS)
        .on_hover_text("Sin implementar: no hay sistema de alarmas");
    for (time, days, active) in [
        ("07:00", "Lunes a viernes", true),
        ("08:30", "Sábados", false),
    ] {
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(width, 36.0), egui::Sense::hover());
        list_row_at(ui, rect, ic::ALARM, time, &format!("{days} · ejemplo"));
        let (label, color) = if active {
            ("Activa", P_OK)
        } else {
            ("Inactiva", P_MUTED)
        };
        text_at(
            ui,
            egui::Pos2::new(rect.max.x, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            label,
            TEXT_CAPTION,
            color,
        );
    }
    divider(ui);

    // Reloj mundial: ciudades de `[clock] cities` en config.toml (por defecto
    // Bogotá, Madrid y Tokio), con desplazamientos fijos (sin base de datos de zonas).
    // ＋ y el engranaje del pie abren config.toml para editarlas.
    let config_path = config::path().filter(|p| p.is_file());
    let add = section_heading_action(ui, "Reloj mundial", ic::PLUS).on_hover_text(
        if config_path.is_some() {
            "Editar [[clock.cities]] en config.toml"
        } else {
            "Crea ~/.config/dinamic-ui/config.toml para añadir ciudades"
        },
    );
    let utc = chrono::Utc::now();
    let summer = eu_summer_time(utc);
    for city in &config::get().clock.cities {
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(width, 24.0), egui::Sense::hover());
        let y = rect.center().y;
        text_at(
            ui,
            egui::Pos2::new(rect.min.x, y),
            egui::Align2::LEFT_CENTER,
            &city.name,
            TEXT_TITLE,
            P_TEXT,
        );
        let dst = if summer && city.dst.as_deref() == Some("eu") {
            60
        } else {
            0
        };
        let minutes = (city.utc_offset * 60.0).round() as i64 + dst;
        let local = utc + chrono::Duration::minutes(minutes);
        text_at(
            ui,
            egui::Pos2::new(rect.max.x, y),
            egui::Align2::RIGHT_CENTER,
            local.format("%H:%M"),
            TEXT_BODY,
            Color32::from_gray(0xdd),
        );
    }

    let settings = panel_footer(ui, "Clock settings", ic::GEAR, true, config_path.is_some());
    if let Some(path) = &config_path {
        if add.clicked() || settings.clicked() {
            open_path(state, path);
        }
    }
}

pub(super) fn days_in_month(year: i32, month: u32) -> u32 {
    let (next_y, next_m) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    chrono::NaiveDate::from_ymd_opt(next_y, next_m, 1)
        .and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .unwrap_or(30)
}

/// Zona horaria del sistema según `/etc/localtime` (p. ej. "America/Bogota").
pub(super) fn timezone_name() -> &'static str {
    static NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    NAME.get_or_init(|| {
        let from_link = std::fs::read_link("/etc/localtime").ok().and_then(|path| {
            let path = path.to_string_lossy().into_owned();
            path.split_once("zoneinfo/")
                .map(|(_, zone)| zone.to_string())
        });
        from_link
            .or_else(|| std::env::var("TZ").ok())
            .unwrap_or_else(|| "Hora local".to_string())
            .replace('_', " ")
    })
}

pub(super) fn format_offset(hours: f32) -> String {
    let sign = if hours < 0.0 { "-" } else { "+" };
    let abs = hours.abs();
    if abs.fract() == 0.0 {
        format!("{sign}{}", abs as i32)
    } else {
        format!(
            "{sign}{}:{:02}",
            abs as i32,
            (abs.fract() * 60.0).round() as i32
        )
    }
}

/// Horario de verano de la UE: del último domingo de marzo al último de
/// octubre, ambos a las 01:00 UTC.
pub(super) fn eu_summer_time(utc: chrono::DateTime<chrono::Utc>) -> bool {
    let last_sunday = |month: u32| {
        let last =
            chrono::NaiveDate::from_ymd_opt(utc.year(), month, days_in_month(utc.year(), month))
                .unwrap_or_default();
        let back = last.weekday().num_days_from_sunday() as i64;
        (last - chrono::Duration::days(back))
            .and_hms_opt(1, 0, 0)
            .unwrap_or_default()
            .and_utc()
    };
    utc >= last_sunday(3) && utc < last_sunday(10)
}

// ── System ──────────────────────────────────────────────────────────────

pub(super) fn panel_system(ui: &mut egui::Ui, state: &mut UiState) {
    let width = ui.available_width();
    let battery = &state.battery;

    section_heading(ui, "BATTERY", "Power status");
    let (card, _) = ui.allocate_exact_size(egui::Vec2::new(width, 78.0), egui::Sense::hover());
    card_at(ui, card, 12);
    let inner = card.shrink(12.0);
    let icon_rect = egui::Rect::from_min_size(inner.min, egui::Vec2::splat(34.0));
    let icon = if battery.charging {
        ic::LIGHTNING
    } else {
        ic::BATTERY_HIGH
    };
    ui.painter()
        .rect_filled(icon_rect, CornerRadius::same(10), P_ICON_BOX);
    icon_at(ui, icon_rect.center(), icon, ICON_LG, P_TEXT);
    let status = match (battery.present, battery.charging) {
        (false, _) => "Sin batería detectada".to_string(),
        (true, charging) => {
            let verb = if charging { "Cargando" } else { "Descargando" };
            match battery.minutes_left {
                Some(m) if charging => format!("{verb} · {} para completar", format_minutes(m)),
                Some(m) => format!("{verb} · {} restantes", format_minutes(m)),
                None => verb.to_string(),
            }
        }
    };
    let x = icon_rect.max.x + 10.0;
    text_at(
        ui,
        egui::Pos2::new(x, icon_rect.center().y - 7.0),
        egui::Align2::LEFT_CENTER,
        "Battery",
        TEXT_CLOCK,
        P_TEXT,
    );
    text_at(
        ui,
        egui::Pos2::new(x, icon_rect.center().y + 9.0),
        egui::Align2::LEFT_CENTER,
        status,
        TEXT_CAPTION,
        P_MUTED,
    );
    let tone = battery_tone(battery.percent, battery.charging);
    let percent = if battery.present {
        format!("{}%", battery.percent)
    } else {
        "--".to_string()
    };
    text_at(
        ui,
        egui::Pos2::new(inner.max.x, icon_rect.center().y),
        egui::Align2::RIGHT_CENTER,
        percent,
        TEXT_DISPLAY,
        tone,
    );
    let bar = egui::Rect::from_min_size(
        egui::Pos2::new(inner.min.x, inner.max.y - 5.0),
        egui::Vec2::new(inner.width(), 5.0),
    );
    bar_at(ui, bar, battery.percent as f32 / 100.0, P_TRACK, tone);
    ui.add_space(15.0);

    // Wi-Fi (nmcli) y Bluetooth (bluetoothctl), leídos por `sysinfo.rs`.
    let system = &state.system;
    section_heading(ui, "CONNECTIVITY", "Network status");
    let wifi = match &system.wifi {
        None => (None, "Sin datos".to_string()),
        Some(w) if !w.enabled => (Some(false), "Desactivado".to_string()),
        Some(w) => match &w.ssid {
            Some(ssid) => (Some(true), truncate(ssid, 20)),
            None => (Some(false), "Sin conexión".to_string()),
        },
    };
    let bluetooth = match &system.bluetooth {
        None => (None, "Sin datos".to_string()),
        Some(b) if !b.powered => (Some(false), "Apagado".to_string()),
        Some(b) => (
            Some(true),
            match b.connected {
                0 => "Encendido · sin dispositivos".to_string(),
                1 => "1 dispositivo".to_string(),
                n => format!("{n} dispositivos"),
            },
        ),
    };
    let gap = 8.0;
    let (row, _) = ui.allocate_exact_size(egui::Vec2::new(width, 72.0), egui::Sense::hover());
    for (rect, (icon, name, (on, detail))) in split(row, &[1.0, 1.0], gap, true).into_iter().zip([
        (ic::WIFI_HIGH, "Wi-Fi", wifi),
        (ic::BLUETOOTH, "Bluetooth", bluetooth),
    ]) {
        card_at(ui, rect, 11);
        let inner = rect.shrink(11.0);
        icon_at(
            ui,
            inner.min + egui::Vec2::new(8.0, 8.0),
            icon,
            ICON_LG,
            Color32::from_gray(0xdd),
        );
        // Punto: verde activo, gris oscuro apagado, gris medio desconocido.
        let dot = match on {
            Some(true) => P_OK,
            Some(false) => Color32::from_gray(0x44),
            None => Color32::from_gray(0x33),
        };
        ui.painter().circle_filled(
            egui::Pos2::new(inner.max.x - 3.0, inner.min.y + 8.0),
            3.0,
            dot,
        );
        text_at(
            ui,
            egui::Pos2::new(inner.min.x, inner.max.y - 14.0),
            egui::Align2::LEFT_CENTER,
            name,
            TEXT_TITLE,
            P_TEXT,
        );
        text_at(
            ui,
            egui::Pos2::new(inner.min.x, inner.max.y - 2.0),
            egui::Align2::LEFT_CENTER,
            detail,
            TEXT_CAPTION,
            P_MUTED,
        );
    }
    ui.add_space(15.0);

    // Brillo (brightnessctl) y volumen (pactl): solo lectura.
    section_heading(ui, "LEVELS", "Output controls");
    let (card, _) = ui.allocate_exact_size(egui::Vec2::new(width, 66.0), egui::Sense::hover());
    card_at(ui, card, 12);
    let inner = card.shrink2(egui::Vec2::new(13.0, 12.0));
    let volume_icon = match &system.volume {
        Some(v) if v.muted => ic::SPEAKER_SLASH,
        _ => ic::SPEAKER_HIGH,
    };
    let levels = [
        (ic::SUN, system.brightness.map(|b| (b, false))),
        (
            volume_icon,
            system.volume.as_ref().map(|v| (v.percent, v.muted)),
        ),
    ];
    for (rect, (icon, level)) in split(inner, &[1.0, 1.0], 12.0, false)
        .into_iter()
        .zip(levels)
    {
        let y = rect.center().y;
        icon_at(
            ui,
            egui::Pos2::new(rect.min.x + 8.0, y),
            icon,
            ICON_MD,
            P_HEADING,
        );
        let track = egui::Rect::from_min_max(
            egui::Pos2::new(rect.min.x + 27.0, y - 2.5),
            egui::Pos2::new(rect.max.x - 38.0, y + 2.5),
        );
        let (fraction, label) = match level {
            Some((_, true)) => (0.0, "Mute".to_string()),
            Some((percent, false)) => (percent as f32 / 100.0, format!("{percent}%")),
            None => (0.0, "--".to_string()),
        };
        bar_at(
            ui,
            track,
            fraction,
            Color32::from_gray(0x33),
            Color32::from_gray(0xdd),
        );
        text_at(
            ui,
            egui::Pos2::new(rect.max.x, y),
            egui::Align2::RIGHT_CENTER,
            label,
            TEXT_CAPTION,
            Color32::from_gray(0x99),
        );
    }
    ui.add_space(15.0);

    // Dispositivos: salida de audio (pactl), teclado y pantalla (hyprctl).
    section_heading(ui, "DEVICES", "Hardware");
    let or_none = |v: &Option<String>| truncate(v.as_deref().unwrap_or("Sin datos"), 44);
    let devices = [
        (
            ic::SPEAKER_HIGH,
            "Audio output",
            or_none(&system.audio_output),
        ),
        (ic::KEYBOARD, "Keyboard", or_none(&system.keyboard)),
        (ic::MONITOR, "Display", or_none(&system.display)),
    ];
    let row_h = 44.0;
    let (card, _) = ui.allocate_exact_size(
        egui::Vec2::new(width, row_h * devices.len() as f32 + 12.0),
        egui::Sense::hover(),
    );
    card_at(ui, card, 12);
    let inner = card.shrink2(egui::Vec2::new(13.0, 6.0));
    for (i, (icon, name, detail)) in devices.into_iter().enumerate() {
        let rect = egui::Rect::from_min_size(
            inner.min + egui::Vec2::new(0.0, i as f32 * row_h),
            egui::Vec2::new(inner.width(), row_h),
        );
        if i > 0 {
            ui.painter()
                .hline(rect.x_range(), rect.min.y, Stroke::new(1.0, P_BORDER));
        }
        list_row_at(ui, rect, icon, name, &detail);
        icon_at(
            ui,
            egui::Pos2::new(rect.max.x - 6.0, rect.center().y),
            ic::CARET_RIGHT,
            ICON_SM,
            P_DIM,
        );
    }

    // Placeholder: Hyprland no tiene una aplicación de ajustes estándar.
    let _ = panel_footer(ui, "System overview", "System settings ↗", false, false);
}

/// "2 h 05 min" / "40 min".
pub(super) fn format_minutes(minutes: u32) -> String {
    if minutes >= 60 {
        format!("{} h {:02} min", minutes / 60, minutes % 60)
    } else {
        format!("{minutes} min")
    }
}

// ── Alertas (centro de notificaciones, cola de la Fase B) ───────────────

pub(super) fn panel_notifications(ui: &mut egui::Ui, state: &mut UiState) {
    let width = ui.available_width();

    // DND: tarjeta con interruptor (suprime la isla vía `notifications.rs`).
    let (card, _) = ui.allocate_exact_size(egui::Vec2::new(width, 50.0), egui::Sense::hover());
    card_at(ui, card, 10);
    let icon_rect = egui::Rect::from_center_size(
        egui::Pos2::new(card.min.x + 12.0 + 14.0, card.center().y),
        egui::Vec2::splat(28.0),
    );
    icon_box_at(ui, icon_rect, ic::MOON, P_BORDER);
    let x = icon_rect.max.x + 9.0;
    text_at(
        ui,
        egui::Pos2::new(x, card.center().y - 6.0),
        egui::Align2::LEFT_CENTER,
        "Do Not Disturb",
        TEXT_BODY,
        P_TEXT,
    );
    text_at(
        ui,
        egui::Pos2::new(x, card.center().y + 8.0),
        egui::Align2::LEFT_CENTER,
        "Silence incoming notifications",
        TEXT_CAPTION,
        P_MUTED,
    );
    let toggle = egui::Rect::from_center_size(
        egui::Pos2::new(card.max.x - 12.0 - 17.0, card.center().y),
        egui::Vec2::new(34.0, 19.0),
    );
    let response = ui
        .interact(
            toggle,
            ui.scope_id().with("panel-dnd"),
            egui::Sense::click(),
        )
        .on_hover_cursor(CursorIcon::PointingHand);
    let (track, knob, knob_x) = if state.notifications.dnd {
        (P_HEADING, P_INK, toggle.max.x - 9.5)
    } else {
        (Color32::from_gray(0x44), P_INVERT, toggle.min.x + 9.5)
    };
    ui.painter()
        .rect_filled(toggle, CornerRadius::same(10), track);
    ui.painter()
        .circle_filled(egui::Pos2::new(knob_x, toggle.center().y), 7.5, knob);
    if response.clicked() {
        state.notifications.toggle_dnd = true;
    }
    ui.add_space(17.0);

    // Cabecera de la lista: fecha, contador y limpiar todo.
    let (header, _) = ui.allocate_exact_size(egui::Vec2::new(width, 24.0), egui::Sense::hover());
    let y = header.center().y;
    text_at(
        ui,
        egui::Pos2::new(header.min.x + 1.0, y),
        egui::Align2::LEFT_CENTER,
        Local::now()
            .format("TODAY · %B %-d")
            .to_string()
            .to_uppercase(),
        TEXT_BODY,
        P_HEADING,
    );
    let clear = egui::Rect::from_min_size(
        egui::Pos2::new(header.max.x - header.height(), header.min.y),
        egui::Vec2::splat(header.height()),
    );
    if round_icon_button_at(ui, clear, "panel-clear-all", ic::BACKSPACE).clicked() {
        state.notifications.clear_all_notifications = true;
    }
    text_at(
        ui,
        egui::Pos2::new(clear.min.x - 8.0, y),
        egui::Align2::RIGHT_CENTER,
        format!("{} new", state.notifications.notification_list.len()),
        TEXT_BODY,
        P_DIM,
    );
    ui.add_space(SP_L + 1.0);

    if state.notifications.notification_list.is_empty() {
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(width, 90.0), egui::Sense::hover());
        icon_at(
            ui,
            rect.center() - egui::Vec2::new(0.0, 10.0),
            ic::MOON,
            ICON_XL,
            P_MUTED,
        );
        text_at(
            ui,
            rect.center() + egui::Vec2::new(0.0, 16.0),
            egui::Align2::CENTER_CENTER,
            "You're all caught up.",
            TEXT_BODY,
            P_MUTED,
        );
    }

    let mut dismiss = None;
    let mut open = None;
    for notification in &state.notifications.notification_list {
        Frame::new()
            .fill(P_SURFACE)
            .stroke(Stroke::new(1.0, P_BORDER))
            .corner_radius(11.0)
            .inner_margin(Margin::symmetric(11, 7))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing = egui::Vec2::new(SP_L, 0.0);
                ui.horizontal(|ui| {
                    let (icon_rect, _) =
                        ui.allocate_exact_size(egui::Vec2::splat(28.0), egui::Sense::hover());
                    icon_box_at(ui, icon_rect, ic::BELL, P_BORDER);
                    ui.label(
                        RichText::new(truncate(&notification.app, 28))
                            .size(TEXT_BODY)
                            .color(P_TEXT),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(&notification.time)
                                .size(TEXT_CAPTION)
                                .color(P_DIM),
                        );
                    });
                });
                ui.label(
                    RichText::new(truncate(&notification.summary, 60))
                        .size(TEXT_TITLE)
                        .color(P_TEXT),
                );
                if !notification.body.is_empty() {
                    ui.label(
                        RichText::new(truncate(&notification.body, 140))
                            .size(TEXT_BODY)
                            .color(Color32::from_gray(0x99)),
                    );
                }
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(ui.available_width(), 16.0),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if text_link(ui, "Abrir aplicación ↗", P_INVERT).clicked() {
                            open = Some(notification.app.clone());
                        }
                        if text_link(ui, "Descartar", Color32::from_gray(0x99)).clicked() {
                            dismiss = Some(notification.id);
                        }
                    },
                );
            });
        ui.add_space(SP_S);
    }
    if let Some(id) = dismiss {
        state.notifications.dismiss_notification_id = Some(id);
    }
    if let Some(app) = open {
        // Comportamiento previo: enfocar la ventana por clase = nombre de la app.
        state
            .pending_commands
            .push(format!("focuswindow class:{app}"));
    }

    // Placeholder: historial y ajustes de notificaciones sin implementar
    // (Mako no tiene interfaz de ajustes).
    // No hay agrupación por app (Downloads…): no hay fuente de datos.
    let _ = panel_footer(
        ui,
        "Notification history",
        "Notification settings ↗",
        false,
        false,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_minimum_grows_with_workspaces() {
        // 10 × 20 + 9 × 3 + 6 + 22 = 255 → 2 × (255 + 12 + 40 + 6) = 626.
        assert_eq!(min_bar_w(10), 626);
        assert!(min_bar_w(20) > min_bar_w(10));
        assert_eq!(min_bar_w(0), min_bar_w(1));
    }
}
