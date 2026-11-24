//! Panel de control: pestañas, helpers y constantes propias.

#[cfg(feature = "wallpaper")]
use super::wallpaper::panel_wallpaper;
use super::*;

// ── Panel de control ────────────────────────────────────────────────────

/// Pestañas del panel. Las de componentes opcionales solo existen si se compilaron.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PanelTab {
    Workspace,
    Clock,
    System,
    #[cfg(feature = "wallpaper")]
    Wallpaper,
    #[cfg(feature = "notifications")]
    Alerts,
}

pub(super) const PANEL_TABS: &[(PanelTab, &str)] = &[
    (PanelTab::Workspace, "Workspace"),
    (PanelTab::Clock, "Clock"),
    (PanelTab::System, "System"),
    #[cfg(feature = "wallpaper")]
    (PanelTab::Wallpaper, "Wallpaper"),
    #[cfg(feature = "notifications")]
    (PanelTab::Alerts, "Alertas"),
];

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
    state.shared.pending_spawns.push(vec![
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
        pal().text_hi,
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
        // Sin esa pestaña compilada no hay a dónde ir.
        #[cfg(feature = "notifications")]
        if let Some(i) = PANEL_TABS.iter().position(|(t, _)| *t == PanelTab::Alerts) {
            state.bar.panel_tab = i;
        }
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
        pal().inset,
        Stroke::new(1.0, pal().border),
        StrokeKind::Inside,
    );
    let inner = bar.shrink(3.0);
    let gap = 3.0;
    let tab_w = (inner.width() - gap * (PANEL_TABS.len() as f32 - 1.0)) / PANEL_TABS.len() as f32;
    for (i, (_, label)) in PANEL_TABS.iter().enumerate() {
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
                .rect_filled(rect, CornerRadius::same(7), pal().border);
        }
        let color = if active || response.hovered() {
            pal().text_hi
        } else {
            pal().muted
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
            match PANEL_TABS[tab].0 {
                PanelTab::Workspace => panel_workspaces(ui, state, palette),
                PanelTab::Clock => panel_clock(ui, state),
                PanelTab::System => panel_system(ui, state),
                #[cfg(feature = "wallpaper")]
                PanelTab::Wallpaper => panel_wallpaper(ui, state),
                #[cfg(feature = "notifications")]
                PanelTab::Alerts => panel_notifications(ui, state),
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
        let active = state.shared.snapshot.workspace == n.to_string();
        let occupied = state.shared.snapshot.occupied.contains(&(n as u32));
        let (fill, stroke, fg) = if active {
            (pal().text_hi, pal().text_hi, pal().solid())
        } else if response.hovered() {
            (pal().solid(), pal().faint, pal().text)
        } else {
            (pal().solid(), pal().border, pal().text)
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
            state
                .shared
                .pending_commands
                .push(switch_workspace_cmd(n as u32));
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
            (pal().border, pal().muted)
        } else {
            (pal().solid(), pal().border)
        };
        ui.painter().rect(
            rect,
            CornerRadius::same(10),
            fill,
            Stroke::new(1.0, stroke),
            StrokeKind::Inside,
        );
        let inner = rect.shrink(SP_L);
        let name_color = if active { pal().text_hi } else { pal().text };
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
                state.shared.pending_spawns.push(layout_command(id));
            }
        }
    }

    let ws = if state.shared.snapshot.workspace.is_empty() {
        "-"
    } else {
        &state.shared.snapshot.workspace
    };
    // Layout real de Hyprland si se pudo leer; si no, el preset marcado.
    let layout = match &state.shared.system.layout {
        Some(layout) => capitalize(layout),
        None => LAYOUT_PRESETS[state.bar.layout_preset.min(3)].to_string(),
    };
    let footer = format!("Workspace {ws} · {layout}");
    // Placeholder: no hay vista de ajustes de workspaces.
    let _ = panel_footer(ui, &footer, ic::GEAR, true, false);
}

/// Mini-vista previa de cada preset.
pub(super) fn paint_layout_preview(ui: &egui::Ui, rect: egui::Rect, preset: usize) {
    let dark = pal().dim;
    let light = pal().muted;
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
        pal().text_hi,
    );
    let (date, _) = ui.allocate_exact_size(egui::Vec2::new(width, 18.0), egui::Sense::hover());
    text_at(
        ui,
        date.center(),
        egui::Align2::CENTER_CENTER,
        format!("{}, {} de {month}", capitalize(weekday), now.day()),
        TEXT_CLOCK,
        pal().text,
    );
    let (zone, _) = ui.allocate_exact_size(egui::Vec2::new(width, 18.0), egui::Sense::hover());
    let offset_h = now.offset().local_minus_utc() as f32 / 3600.0;
    text_at(
        ui,
        zone.center(),
        egui::Align2::CENTER_CENTER,
        format!("{} · UTC{}", timezone_name(), format_offset(offset_h)),
        TEXT_BODY,
        pal().muted,
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
        pal().text,
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
            pal().text_hi
        } else {
            pal().muted
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
            pal().muted,
        );
    }
    for day in 1..=days {
        let rect = cell(7 + lead + day as usize - 1);
        let is_today = today == Some(day);
        if is_today {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(6), pal().text_hi);
        }
        let color = if is_today { pal().solid() } else { pal().text };
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
            ("Activa", pal().ok)
        } else {
            ("Inactiva", pal().muted)
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
            pal().text_hi,
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
            pal().text_hi,
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

fn format_minutes(minutes: u32) -> String {
    if minutes >= 60 {
        let h = minutes / 60;
        let m = minutes % 60;
        if m == 0 {
            format!("{h} h")
        } else {
            format!("{h} h {m} min")
        }
    } else {
        format!("{minutes} min")
    }
}

pub(super) fn panel_system(ui: &mut egui::Ui, state: &mut UiState) {
    let width = ui.available_width();
    let battery = &state.shared.battery;

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
        .rect_filled(icon_rect, CornerRadius::same(10), pal().badge);
    icon_at(ui, icon_rect.center(), icon, ICON_LG, pal().text_hi);
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
        pal().text_hi,
    );
    text_at(
        ui,
        egui::Pos2::new(x, icon_rect.center().y + 9.0),
        egui::Align2::LEFT_CENTER,
        status,
        TEXT_CAPTION,
        pal().muted,
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
    bar_at(ui, bar, battery.percent as f32 / 100.0, pal().track, tone);
    ui.add_space(15.0);

    // Los mismos controles que el overlay del topbar, en el ancho del panel.
    section_heading(ui, "QUICK SETTINGS", "Sistema");
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(width, 210.0), egui::Sense::hover());
    super::quick_settings::draw(ui, rect, state, pal());
}

// ── Alertas (centro de notificaciones, cola de la Fase B) ───────────────

#[cfg(feature = "notifications")]
pub(super) fn panel_notifications(ui: &mut egui::Ui, state: &mut UiState) {
    let width = ui.available_width();

    // DND: tarjeta con interruptor (suprime la isla vía `notifications.rs`).
    let (card, _) = ui.allocate_exact_size(egui::Vec2::new(width, 50.0), egui::Sense::hover());
    card_at(ui, card, 10);
    let icon_rect = egui::Rect::from_center_size(
        egui::Pos2::new(card.min.x + 12.0 + 14.0, card.center().y),
        egui::Vec2::splat(28.0),
    );
    icon_box_at(ui, icon_rect, ic::MOON, pal().border);
    let x = icon_rect.max.x + 9.0;
    text_at(
        ui,
        egui::Pos2::new(x, card.center().y - 6.0),
        egui::Align2::LEFT_CENTER,
        "Do Not Disturb",
        TEXT_BODY,
        pal().text_hi,
    );
    text_at(
        ui,
        egui::Pos2::new(x, card.center().y + 8.0),
        egui::Align2::LEFT_CENTER,
        "Silence incoming notifications",
        TEXT_CAPTION,
        pal().muted,
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
        (pal().muted, pal().solid(), toggle.max.x - 9.5)
    } else {
        (pal().faint, pal().text_hi, toggle.min.x + 9.5)
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
        pal().muted,
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
        pal().dim,
    );
    ui.add_space(SP_L + 1.0);

    if state.notifications.notification_list.is_empty() {
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(width, 90.0), egui::Sense::hover());
        icon_at(
            ui,
            rect.center() - egui::Vec2::new(0.0, 10.0),
            ic::MOON,
            ICON_XL,
            pal().muted,
        );
        text_at(
            ui,
            rect.center() + egui::Vec2::new(0.0, 16.0),
            egui::Align2::CENTER_CENTER,
            "You're all caught up.",
            TEXT_BODY,
            pal().muted,
        );
    }

    let mut dismiss = None;
    let mut open = None;
    for notification in &state.notifications.notification_list {
        Frame::new()
            .fill(pal().solid())
            .stroke(Stroke::new(1.0, pal().border))
            .corner_radius(11.0)
            .inner_margin(Margin::symmetric(11, 7))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing = egui::Vec2::new(SP_L, 0.0);
                ui.horizontal(|ui| {
                    let (icon_rect, _) =
                        ui.allocate_exact_size(egui::Vec2::splat(28.0), egui::Sense::hover());
                    icon_box_at(ui, icon_rect, ic::BELL, pal().border);
                    ui.label(
                        RichText::new(truncate(&notification.app, 28))
                            .size(TEXT_BODY)
                            .color(pal().text_hi),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(&notification.time)
                                .size(TEXT_CAPTION)
                                .color(pal().dim),
                        );
                    });
                });
                ui.label(
                    RichText::new(truncate(&notification.summary, 60))
                        .size(TEXT_TITLE)
                        .color(pal().text_hi),
                );
                if !notification.body.is_empty() {
                    ui.label(
                        RichText::new(truncate(&notification.body, 140))
                            .size(TEXT_BODY)
                            .color(pal().muted),
                    );
                }
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(ui.available_width(), 16.0),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if text_link(ui, "Abrir aplicación ↗", pal().text_hi).clicked() {
                            open = Some(notification.app.clone());
                        }
                        if text_link(ui, "Descartar", pal().muted).clicked() {
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
            .shared
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

/// Tarjeta del panel de control, centrada bajo la barra.
pub(super) fn draw_panel_card(
    ui: &mut egui::Ui,
    state: &mut UiState,
    palette: Palette,
    t: f32,
    panel_t: f32,
) {
    let screen = ui.max_rect();
    if panel_t > 0.01 {
        // Tarjeta independiente, centrada bajo la píldora y tan ancha como el modo.
        let card_w = panel_width(t).round();
        let card_h = metrics().panel_h as f32 * panel_t;
        let card = egui::Rect::from_min_size(
            egui::Pos2::new(
                ((screen.width() - card_w) / 2.0).round(),
                metrics().bar_h as f32 + PANEL_GAP,
            ),
            egui::Vec2::new(card_w, card_h),
        );
        let card_fill = pal().card_fill;
        let card_border = pal().border;
        ui.painter().rect(
            card,
            CornerRadius::same(18),
            card_fill,
            Stroke::new(1.0, card_border),
            StrokeKind::Middle,
        );
        let inner_min = card.min + egui::Vec2::new(PANEL_PAD_X, PANEL_PAD_Y);
        // Al abrir, la tarjeta mide menos que el padding: sin rect invertido.
        let panel_inner = egui::Rect::from_min_max(
            inner_min,
            (card.max - egui::Vec2::new(PANEL_PAD_X, PANEL_PAD_Y)).max(inner_min),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(panel_inner)
                .id_salt("panel-layer"),
        );
        // Durante la animación de apertura el contenido no sale de la tarjeta.
        child.set_clip_rect(card.intersect(ui.clip_rect()));
        child.set_opacity(panel_t);
        draw_panel(&mut child, state, palette);
    }
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
