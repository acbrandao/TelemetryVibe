//! Right-side properties panel for the selected gauge(s).

use egui::{CollapsingHeader, DragValue, Grid, RichText, Slider};

use super::widgets::{color_edit, prop_label};
use crate::app::application::GaugeApp;
use crate::app::commands::Command;
use crate::gauges::library::{default_range, hr_zones, power_zones, speed_zones};
use crate::gauges::model::*;
use crate::telemetry::units::{to_display, to_si, unit_label};
use crate::telemetry::{Metric, UnitPref, UnitSystem};

pub fn group_selection(app: &mut GaugeApp) {
    if app.state.selection.len() < 2 {
        return;
    }
    let gid = app.state.project.next_group_id;
    app.state.project.next_group_id += 1;
    let gs: Vec<Gauge> = app
        .state
        .selected_gauges()
        .into_iter()
        .cloned()
        .map(|mut g| {
            g.group = Some(gid);
            g
        })
        .collect();
    app.state.execute(Command::UpdateGauges {
        gauges: gs,
        merge: "group",
    });
}

pub fn ungroup_selection(app: &mut GaugeApp) {
    let gs: Vec<Gauge> = app
        .state
        .selected_gauges()
        .into_iter()
        .cloned()
        .map(|mut g| {
            g.group = None;
            g
        })
        .collect();
    if !gs.is_empty() {
        app.state.execute(Command::UpdateGauges {
            gauges: gs,
            merge: "group",
        });
    }
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    HCenter,
    Right,
    Top,
    VCenter,
    Bottom,
}

fn align(app: &mut GaugeApp, a: Align) {
    let sel: Vec<Gauge> = app
        .state
        .selected_gauges()
        .into_iter()
        .filter(|g| !g.locked)
        .cloned()
        .collect();
    if sel.is_empty() {
        return;
    }
    // Single gauge aligns to the video frame; several align to their common bounds.
    let (bx0, by0, bx1, by1) = if sel.len() == 1 {
        let (w, h) = app.state.video_size();
        let m = h * 0.035;
        (m, m, w - m, h - m)
    } else {
        sel.iter()
            .fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |acc, g| {
                let (x, y, w, h) = g.placement.bounds();
                (
                    acc.0.min(x),
                    acc.1.min(y),
                    acc.2.max(x + w),
                    acc.3.max(y + h),
                )
            })
    };
    let moved: Vec<Gauge> = sel
        .into_iter()
        .map(|mut g| {
            let (x, y, w, h) = g.placement.bounds();
            let (dx, dy) = match a {
                Align::Left => (bx0 - x, 0.0),
                Align::HCenter => ((bx0 + bx1) / 2.0 - (x + w / 2.0), 0.0),
                Align::Right => (bx1 - (x + w), 0.0),
                Align::Top => (0.0, by0 - y),
                Align::VCenter => (0.0, (by0 + by1) / 2.0 - (y + h / 2.0)),
                Align::Bottom => (0.0, by1 - (y + h)),
            };
            g.placement.x = (g.placement.x + dx).round();
            g.placement.y = (g.placement.y + dy).round();
            g
        })
        .collect();
    app.state.undo.break_merge();
    app.state.execute(Command::UpdateGauges {
        gauges: moved,
        merge: "align",
    });
}

fn align_bar(app: &mut GaugeApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        for (label, a, tip) in [
            ("⇤", Align::Left, "Align left"),
            ("↔", Align::HCenter, "Align centers horizontally"),
            ("⇥", Align::Right, "Align right"),
            ("⤒", Align::Top, "Align top"),
            ("↕", Align::VCenter, "Align centers vertically"),
            ("⤓", Align::Bottom, "Align bottom"),
        ] {
            if ui.button(label).on_hover_text(tip).clicked() {
                align(app, a);
            }
        }
    });
}

pub fn show(app: &mut GaugeApp, ui: &mut egui::Ui) {
    let sel = app.state.selection.clone();
    match sel.len() {
        0 => project_settings(app, ui),
        1 => {
            let Some(g) = app.state.project.gauge(sel[0]).cloned() else {
                return;
            };
            let mut edited = g.clone();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    gauge_editor(app, ui, &mut edited);
                });
            if edited != g {
                app.state.execute(Command::UpdateGauge(edited));
            }
        }
        n => {
            ui.label(RichText::new(format!("{n} gauges selected")).strong());
            ui.add_space(4.0);
            prop_label(ui, if n > 1 { "Align selection" } else { "Align" });
            align_bar(app, ui);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Group").clicked() {
                    group_selection(app);
                }
                if ui.button("Ungroup").clicked() {
                    ungroup_selection(app);
                }
                if ui.button("Duplicate").clicked() {
                    app.state.duplicate_selection();
                }
                if ui.button("Delete").clicked() {
                    app.state.delete_selection();
                }
            });
            ui.add_space(6.0);
            let mut opacity = app
                .state
                .selected_gauges()
                .first()
                .map(|g| g.opacity)
                .unwrap_or(1.0);
            ui.horizontal(|ui| {
                prop_label(ui, "Opacity");
                if ui
                    .add(
                        Slider::new(&mut opacity, 0.0..=1.0)
                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                    )
                    .changed()
                {
                    let gs: Vec<Gauge> = app
                        .state
                        .selected_gauges()
                        .into_iter()
                        .cloned()
                        .map(|mut g| {
                            g.opacity = opacity;
                            g
                        })
                        .collect();
                    app.state.execute(Command::UpdateGauges {
                        gauges: gs,
                        merge: "multi-opacity",
                    });
                }
            });
        }
    }
}

fn project_settings(app: &mut GaugeApp, ui: &mut egui::Ui) {
    ui.label(RichText::new("No gauge selected").strong());
    ui.label(RichText::new("Select a gauge on the video or in the Layers list to edit it.").weak());
    ui.add_space(10.0);
    super::widgets::section(ui, "Project");
    Grid::new("project_grid")
        .num_columns(2)
        .spacing([10.0, 6.0])
        .show(ui, |ui| {
            prop_label(ui, "Name");
            ui.text_edit_singleline(&mut app.state.project.name);
            ui.end_row();
            prop_label(ui, "Units");
            let mut u = app.state.project.units;
            ui.horizontal(|ui| {
                ui.selectable_value(&mut u, UnitSystem::Metric, "Metric");
                ui.selectable_value(&mut u, UnitSystem::Imperial, "Imperial");
            });
            if u != app.state.project.units {
                app.state.execute(Command::SetUnits(u));
            }
            ui.end_row();
            prop_label(ui, "GPS offset");
            ui.label(crate::utils::timecode::format_offset(
                app.state.project.sync.offset,
            ));
            ui.end_row();
        });
    ui.add_space(6.0);
    if ui.button("Synchronize GPS…").clicked() {
        app.ui.show_sync = true;
    }
}

fn metric_combo(ui: &mut egui::Ui, g: &mut Gauge, track: Option<&crate::telemetry::Track>) {
    let label = if g.metric == Metric::Custom {
        format!("Custom: {}", g.custom_key)
    } else {
        g.metric.label().to_string()
    };
    egui::ComboBox::from_id_salt("metric")
        .selected_text(label)
        .width(170.0)
        .show_ui(ui, |ui| {
            for m in Metric::BUILTIN {
                let has = track.is_none_or(|t| t.has(m));
                let text = if has {
                    RichText::new(m.label())
                } else {
                    RichText::new(format!("{} (no data)", m.label())).weak()
                };
                ui.selectable_value(&mut g.metric, m, text);
            }
            if let Some(t) = track
                && !t.extra.is_empty()
            {
                ui.separator();
                ui.label(RichText::new("Additional fields").small().weak());
                for key in t.extra.keys() {
                    if ui
                        .selectable_label(g.metric == Metric::Custom && &g.custom_key == key, key)
                        .clicked()
                    {
                        g.metric = Metric::Custom;
                        g.custom_key = key.clone();
                    }
                }
            }
        });
}

fn gauge_editor(app: &mut GaugeApp, ui: &mut egui::Ui, g: &mut Gauge) {
    let units = g.units.resolve(app.state.project.units);
    let track = app.state.track.clone();
    let track = track.as_deref();

    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut g.name)
                .desired_width(170.0)
                .font(egui::TextStyle::Heading),
        );
        ui.label(RichText::new(g.kind.type_label()).weak());
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut g.visible, "Visible");
        ui.checkbox(&mut g.locked, "Locked");
        if ui.button("Duplicate").clicked() {
            // Applied after this frame's edit to avoid clobbering.
            app.state.selection = vec![g.id];
            app.state.duplicate_selection();
        }
        if ui.button("Delete").clicked() {
            app.state.execute(Command::DeleteGauges(vec![g.id]));
        }
    });
    align_bar(app, ui);
    ui.horizontal(|ui| {
        if ui.small_button("Bring forward").clicked() {
            app.state.execute(Command::Reorder { id: g.id, delta: 1 });
        }
        if ui.small_button("Send backward").clicked() {
            app.state.execute(Command::Reorder {
                id: g.id,
                delta: -1,
            });
        }
    });

    if g.kind.uses_metric() {
        CollapsingHeader::new("Data")
            .default_open(true)
            .show(ui, |ui| {
                Grid::new("data_grid")
                    .num_columns(2)
                    .spacing([10.0, 6.0])
                    .show(ui, |ui| {
                        prop_label(ui, "Source");
                        let before = g.metric;
                        metric_combo(ui, g, track);
                        if g.metric != before && g.metric != Metric::Custom {
                            let (lo, hi) = default_range(g.metric, track, units);
                            g.min = lo;
                            g.max = hi;
                            g.style.decimals = g.metric.default_decimals();
                            g.zones.clear();
                        }
                        ui.end_row();
                        prop_label(ui, "Units");
                        egui::ComboBox::from_id_salt("units")
                            .selected_text(match g.units {
                                UnitPref::Inherit => "Project",
                                UnitPref::Metric => "Metric",
                                UnitPref::Imperial => "Imperial",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut g.units, UnitPref::Inherit, "Project");
                                ui.selectable_value(&mut g.units, UnitPref::Metric, "Metric");
                                ui.selectable_value(&mut g.units, UnitPref::Imperial, "Imperial");
                            });
                        ui.end_row();
                        prop_label(ui, "Decimals");
                        ui.add(DragValue::new(&mut g.style.decimals).range(0..=4));
                        ui.end_row();
                        let unit = if g.metric == Metric::Custom {
                            ""
                        } else {
                            unit_label(g.metric, units)
                        };
                        let custom = g.metric == Metric::Custom;
                        let conv = |v: f64| {
                            if custom {
                                v
                            } else {
                                to_display(g.metric, v, units)
                            }
                        };
                        let back = |v: f64| if custom { v } else { to_si(g.metric, v, units) };
                        prop_label(ui, "Range");
                        ui.horizontal(|ui| {
                            let mut lo = conv(g.min);
                            let mut hi = conv(g.max);
                            let speed = ((hi - lo).abs() / 200.0).max(0.01);
                            let r1 = ui.add(DragValue::new(&mut lo).speed(speed).max_decimals(2));
                            ui.label("–");
                            let r2 = ui.add(DragValue::new(&mut hi).speed(speed).max_decimals(2));
                            ui.label(unit);
                            if r1.changed() || r2.changed() {
                                g.min = back(lo);
                                g.max = back(hi.max(lo + 1e-6));
                            }
                        });
                        ui.end_row();
                        prop_label(ui, "");
                        if ui.small_button("Auto from recording").clicked() {
                            let (lo, hi) = default_range(g.metric, track, units);
                            g.min = lo;
                            g.max = hi;
                        }
                        ui.end_row();
                    });
            });
    }

    CollapsingHeader::new("Position & Size")
        .default_open(true)
        .show(ui, |ui| {
            Grid::new("pos_grid")
                .num_columns(4)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    let p = &mut g.placement;
                    prop_label(ui, "X");
                    ui.add(DragValue::new(&mut p.x).speed(1.0).max_decimals(0));
                    prop_label(ui, "Y");
                    ui.add(DragValue::new(&mut p.y).speed(1.0).max_decimals(0));
                    ui.end_row();
                    prop_label(ui, "Width");
                    ui.add(
                        DragValue::new(&mut p.w)
                            .speed(1.0)
                            .range(8.0..=10000.0)
                            .max_decimals(0),
                    );
                    prop_label(ui, "Height");
                    ui.add(
                        DragValue::new(&mut p.h)
                            .speed(1.0)
                            .range(8.0..=10000.0)
                            .max_decimals(0),
                    );
                    ui.end_row();
                    prop_label(ui, "Rotation");
                    ui.add(
                        DragValue::new(&mut p.rotation)
                            .speed(0.5)
                            .range(-180.0..=180.0)
                            .suffix("°")
                            .max_decimals(1),
                    );
                    ui.end_row();
                });
        });

    CollapsingHeader::new("Appearance")
        .default_open(true)
        .show(ui, |ui| {
            let st = &mut g.style;
            Grid::new("look_grid")
                .num_columns(2)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    prop_label(ui, "Opacity");
                    ui.add(
                        Slider::new(&mut g.opacity, 0.0..=1.0)
                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                    );
                    ui.end_row();
                    prop_label(ui, "Primary");
                    color_edit(ui, &mut st.primary);
                    ui.end_row();
                    prop_label(ui, "Secondary");
                    color_edit(ui, &mut st.secondary);
                    ui.end_row();
                    prop_label(ui, "Accent");
                    color_edit(ui, &mut st.accent);
                    ui.end_row();
                    prop_label(ui, "Text");
                    color_edit(ui, &mut st.text);
                    ui.end_row();
                    prop_label(ui, "Background");
                    ui.horizontal(|ui| {
                        color_edit(ui, &mut st.background);
                        ui.add(
                            Slider::new(&mut st.background_opacity, 0.0..=1.0)
                                .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                        );
                    });
                    ui.end_row();
                    prop_label(ui, "Border");
                    ui.horizontal(|ui| {
                        color_edit(ui, &mut st.border);
                        ui.add(
                            DragValue::new(&mut st.border_width)
                                .range(0.0..=20.0)
                                .speed(0.1)
                                .suffix(" px"),
                        );
                    });
                    ui.end_row();
                    prop_label(ui, "Corner radius");
                    ui.add(Slider::new(&mut st.corner_radius, 0.0..=0.5).show_value(false));
                    ui.end_row();
                    prop_label(ui, "Font");
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("weight")
                            .selected_text(st.font_weight.label())
                            .width(80.0)
                            .show_ui(ui, |ui| {
                                for w in FontWeight::ALL {
                                    ui.selectable_value(&mut st.font_weight, w, w.label());
                                }
                            });
                        ui.add(
                            DragValue::new(&mut st.font_scale)
                                .range(0.3..=3.0)
                                .speed(0.01)
                                .prefix("× "),
                        );
                    });
                    ui.end_row();
                    prop_label(ui, "Shadow");
                    ui.checkbox(&mut st.shadow, "");
                    ui.end_row();
                    prop_label(ui, "Glow");
                    ui.add(Slider::new(&mut st.glow, 0.0..=1.0).show_value(false));
                    ui.end_row();
                    prop_label(ui, "Label");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut st.show_label, "");
                        ui.add(
                            egui::TextEdit::singleline(&mut st.label)
                                .hint_text("auto")
                                .desired_width(120.0),
                        );
                    });
                    ui.end_row();
                    if g.kind.uses_metric() {
                        prop_label(ui, "Show units");
                        ui.checkbox(&mut st.show_units, "");
                        ui.end_row();
                        prop_label(ui, "Prefix / suffix");
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut st.prefix).desired_width(50.0));
                            ui.add(egui::TextEdit::singleline(&mut st.suffix).desired_width(50.0));
                        });
                        ui.end_row();
                        prop_label(ui, "Tick marks");
                        ui.checkbox(&mut st.show_ticks, "");
                        ui.end_row();
                    }
                });
        });

    kind_options(ui, g, app);

    if g.kind.uses_metric()
        || matches!(
            g.kind,
            GaugeKind::Map {
                speed_colors: true,
                ..
            }
        )
    {
        if matches!(g.kind, GaugeKind::Map { .. }) {
            // Map speed colors are speed zones; seed defaults when switched on without any.
            g.metric = Metric::Speed;
            if g.zones.is_empty() {
                let max = track
                    .and_then(|t| t.stats.get(Metric::Speed))
                    .map(|s| s.max)
                    .filter(|m| *m > 1.0)
                    .unwrap_or(15.0);
                g.zones = crate::gauges::library::speed_zones(max);
            }
        }
        zones_editor(ui, g, units, track);
        CollapsingHeader::new("Animation")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    prop_label(ui, "Smoothing");
                    ui.add(
                        Slider::new(&mut g.smoothing, 0.0..=1.0)
                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                    );
                });
                ui.label(
                    RichText::new("0% shows raw data; higher values give smoother motion.")
                        .small()
                        .weak(),
                );
            });
    }
}

fn kind_options(ui: &mut egui::Ui, g: &mut Gauge, app: &mut GaugeApp) {
    CollapsingHeader::new(format!("{} options", g.kind.type_label()))
        .default_open(true)
        .show(ui, |ui| {
            Grid::new("kind_grid")
                .num_columns(2)
                .spacing([10.0, 6.0])
                .show(ui, |ui| match &mut g.kind {
                    GaugeKind::Analog {
                        dial,
                        sweep,
                        major_ticks,
                        minor_ticks,
                        show_readout,
                    } => {
                        prop_label(ui, "Style");
                        egui::ComboBox::from_id_salt("dial")
                            .selected_text(dial.label())
                            .show_ui(ui, |ui| {
                                for d in DialStyle::ALL {
                                    ui.selectable_value(dial, d, d.label());
                                }
                            });
                        ui.end_row();
                        prop_label(ui, "Sweep");
                        ui.add(Slider::new(sweep, 90.0..=340.0).suffix("°"));
                        ui.end_row();
                        prop_label(ui, "Major ticks");
                        ui.add(DragValue::new(major_ticks).range(0..=30))
                            .on_hover_text("0 = automatic");
                        ui.end_row();
                        prop_label(ui, "Minor ticks");
                        ui.add(DragValue::new(minor_ticks).range(0..=10));
                        ui.end_row();
                        prop_label(ui, "Digital readout");
                        ui.checkbox(show_readout, "");
                        ui.end_row();
                    }
                    GaugeKind::Digital { align, .. } | GaugeKind::Text { align, .. } => {
                        prop_label(ui, "Align");
                        ui.horizontal(|ui| {
                            ui.selectable_value(align, TextAlign::Left, "Left");
                            ui.selectable_value(align, TextAlign::Center, "Center");
                            ui.selectable_value(align, TextAlign::Right, "Right");
                        });
                        ui.end_row();
                        if let GaugeKind::Digital { icon, .. } = &mut g.kind {
                            prop_label(ui, "Icon");
                            egui::ComboBox::from_id_salt("digital_icon")
                                .selected_text(icon.label())
                                .show_ui(ui, |ui| {
                                    for i in DigitalIcon::ALL {
                                        ui.selectable_value(icon, i, i.label());
                                    }
                                })
                                .response
                                .on_hover_text(
                                    "Beating heart pulses at the heart-rate value; slope wedge tilts with the gradient",
                                );
                            ui.end_row();
                        }
                        if let GaugeKind::Text { text, .. } = &mut g.kind {
                            prop_label(ui, "Text");
                            ui.add(
                                egui::TextEdit::singleline(text).hint_text("Use {date} or {time}"),
                            );
                            ui.end_row();
                        }
                    }
                    GaugeKind::Tape {
                        orientation,
                        span,
                        major_step,
                        minor_ticks,
                    } => {
                        prop_label(ui, "Orientation");
                        ui.horizontal(|ui| {
                            ui.selectable_value(orientation, Orientation::Vertical, "Vertical");
                            ui.selectable_value(orientation, Orientation::Horizontal, "Horizontal");
                        });
                        ui.end_row();
                        prop_label(ui, "Visible span");
                        ui.add(DragValue::new(span).range(1.0..=100000.0).speed(1.0));
                        ui.end_row();
                        prop_label(ui, "Major step");
                        ui.add(DragValue::new(major_step).range(0.0..=10000.0).speed(0.5))
                            .on_hover_text("0 = automatic");
                        ui.end_row();
                        prop_label(ui, "Minor ticks");
                        ui.add(DragValue::new(minor_ticks).range(1..=10));
                        ui.end_row();
                    }
                    GaugeKind::Bar {
                        orientation,
                        segments,
                        rounded,
                        thickness,
                        show_value,
                    } => {
                        prop_label(ui, "Orientation");
                        ui.horizontal(|ui| {
                            ui.selectable_value(orientation, Orientation::Horizontal, "Horizontal");
                            ui.selectable_value(orientation, Orientation::Vertical, "Vertical");
                        });
                        ui.end_row();
                        prop_label(ui, "Segments");
                        ui.add(DragValue::new(segments).range(0..=60))
                            .on_hover_text("0 = continuous");
                        ui.end_row();
                        prop_label(ui, "Thickness");
                        ui.add(Slider::new(thickness, 0.05..=1.0).show_value(false));
                        ui.end_row();
                        prop_label(ui, "Rounded");
                        ui.checkbox(rounded, "");
                        ui.end_row();
                        prop_label(ui, "Show value");
                        ui.checkbox(show_value, "");
                        ui.end_row();
                    }
                    GaugeKind::Zone { show_value } => {
                        prop_label(ui, "Show value");
                        ui.checkbox(show_value, "");
                        ui.end_row();
                    }
                    GaugeKind::Graph {
                        window,
                        full_activity,
                        line_width,
                        fill,
                        grid,
                        auto_scale,
                        show_value,
                    } => {
                        prop_label(ui, "Full activity");
                        ui.checkbox(full_activity, "");
                        ui.end_row();
                        if !*full_activity {
                            prop_label(ui, "Time window");
                            ui.add(
                                Slider::new(window, 5.0..=600.0)
                                    .logarithmic(true)
                                    .suffix(" s"),
                            );
                            ui.end_row();
                        }
                        prop_label(ui, "Line width");
                        ui.add(Slider::new(line_width, 0.5..=8.0));
                        ui.end_row();
                        prop_label(ui, "Fill");
                        ui.checkbox(fill, "");
                        ui.end_row();
                        prop_label(ui, "Grid");
                        ui.checkbox(grid, "");
                        ui.end_row();
                        prop_label(ui, "Auto scale");
                        ui.checkbox(auto_scale, "")
                            .on_hover_text("Off: use the gauge range");
                        ui.end_row();
                        prop_label(ui, "Show value");
                        ui.checkbox(show_value, "");
                        ui.end_row();
                    }
                    GaugeKind::Map {
                        mode,
                        trail_seconds,
                        route_width,
                        marker_size,
                        show_arrow,
                        heading_up,
                        zoom_radius,
                        speed_colors,
                    } => {
                        prop_label(ui, "Route");
                        egui::ComboBox::from_id_salt("route_mode")
                            .selected_text(mode.label())
                            .show_ui(ui, |ui| {
                                for m in RouteMode::ALL {
                                    ui.selectable_value(mode, m, m.label());
                                }
                            });
                        ui.end_row();
                        if *mode == RouteMode::Trail {
                            prop_label(ui, "Trail length");
                            ui.add(
                                Slider::new(trail_seconds, 10.0..=1800.0)
                                    .logarithmic(true)
                                    .suffix(" s"),
                            );
                            ui.end_row();
                        }
                        if *mode == RouteMode::CloseUp {
                            prop_label(ui, "Zoom radius");
                            ui.add(
                                Slider::new(zoom_radius, 50.0..=5000.0)
                                    .logarithmic(true)
                                    .suffix(" m"),
                            )
                            .on_hover_text("Distance visible from the position to the map edge");
                            ui.end_row();
                        }
                        prop_label(ui, "Speed colors");
                        ui.checkbox(speed_colors, "")
                            .on_hover_text("Color the marker (and close-up trail) by speed using the color zones below");
                        ui.end_row();
                        prop_label(ui, "Route width");
                        ui.add(Slider::new(route_width, 0.5..=20.0));
                        ui.end_row();
                        prop_label(ui, "Marker size");
                        ui.add(Slider::new(marker_size, 2.0..=40.0));
                        ui.end_row();
                        prop_label(ui, "Direction arrow");
                        ui.checkbox(show_arrow, "");
                        ui.end_row();
                        prop_label(ui, "Heading up");
                        ui.checkbox(heading_up, "");
                        ui.end_row();
                    }
                    GaugeKind::Time {
                        format,
                        tz_offset_minutes,
                    } => {
                        prop_label(ui, "Format");
                        egui::ComboBox::from_id_salt("time_fmt")
                            .selected_text(format.label())
                            .show_ui(ui, |ui| {
                                for f in TimeFormat::ALL {
                                    ui.selectable_value(format, f, f.label());
                                }
                            });
                        ui.end_row();
                        if matches!(format, TimeFormat::Date | TimeFormat::TimeOfDay) {
                            prop_label(ui, "UTC offset");
                            let mut h = *tz_offset_minutes as f32 / 60.0;
                            if ui
                                .add(
                                    DragValue::new(&mut h)
                                        .range(-14.0..=14.0)
                                        .speed(0.25)
                                        .suffix(" h"),
                                )
                                .changed()
                            {
                                *tz_offset_minutes = (h * 60.0).round() as i32;
                            }
                            ui.end_row();
                        }
                    }
                    GaugeKind::Ekg {
                        beats,
                        show_value,
                        grid,
                        ..
                    } => {
                        prop_label(ui, "Beats shown");
                        ui.add(Slider::new(beats, 1.0..=10.0).step_by(0.5))
                            .on_hover_text("Heartbeats visible in the strip; the time span follows the heart rate");
                        ui.end_row();
                        prop_label(ui, "Show value");
                        ui.checkbox(show_value, "");
                        ui.end_row();
                        prop_label(ui, "Grid");
                        ui.checkbox(grid, "");
                        ui.end_row();
                    }
                    GaugeKind::Image { path } => {
                        prop_label(ui, "Image (PNG)");
                        ui.horizontal(|ui| {
                            let name = std::path::Path::new(path.as_str())
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "none".into());
                            ui.label(name);
                            if ui.button("Choose…").clicked() {
                                let mut d =
                                    rfd::FileDialog::new().add_filter("PNG image", &["png", "PNG"]);
                                if let Some(dir) = &app.settings.last_dir {
                                    d = d.set_directory(dir);
                                }
                                if let Some(p) = d.pick_file() {
                                    *path = p.to_string_lossy().to_string();
                                }
                            }
                        });
                        ui.end_row();
                    }
                });
        });
}

fn zones_editor(
    ui: &mut egui::Ui,
    g: &mut Gauge,
    units: UnitSystem,
    track: Option<&crate::telemetry::Track>,
) {
    CollapsingHeader::new("Color zones")
        .default_open(!g.zones.is_empty())
        .show(ui, |ui| {
            let custom = g.metric == Metric::Custom;
            let unit = if custom {
                ""
            } else {
                unit_label(g.metric, units)
            };
            let mut remove = None;
            for (i, z) in g.zones.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    color_edit(ui, &mut z.color);
                    let mut v = if custom {
                        z.from
                    } else {
                        to_display(g.metric, z.from, units)
                    };
                    ui.label("from");
                    if ui
                        .add(
                            DragValue::new(&mut v)
                                .speed(0.5)
                                .max_decimals(1)
                                .suffix(format!(" {unit}")),
                        )
                        .changed()
                    {
                        z.from = if custom { v } else { to_si(g.metric, v, units) };
                    }
                    ui.add(
                        egui::TextEdit::singleline(&mut z.label)
                            .desired_width(70.0)
                            .hint_text("label"),
                    );
                    if ui.small_button("×").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                g.zones.remove(i);
            }
            g.zones.sort_by(|a, b| a.from.total_cmp(&b.from));
            ui.horizontal_wrapped(|ui| {
                if ui.small_button("+ Add zone").clicked() {
                    let from = g
                        .zones
                        .last()
                        .map(|z| (z.from + g.max) / 2.0)
                        .unwrap_or(g.min);
                    g.zones.push(Zone {
                        from,
                        color: Rgba::rgb(255, 160, 40),
                        label: String::new(),
                    });
                }
                match g.metric {
                    Metric::HeartRate => {
                        if ui
                            .small_button("HR zones")
                            .on_hover_text("Five zones from your max HR (from the recording)")
                            .clicked()
                        {
                            let max = track
                                .and_then(|t| t.stats.get(Metric::HeartRate))
                                .map(|s| s.max.max(160.0))
                                .unwrap_or(190.0);
                            g.zones = hr_zones(max);
                        }
                    }
                    Metric::Power => {
                        if ui
                            .small_button("Power zones")
                            .on_hover_text("Coggan zones from an estimated FTP")
                            .clicked()
                        {
                            let ftp = track
                                .and_then(|t| t.stats.get(Metric::Power))
                                .map(|s| (s.avg * 1.25).clamp(150.0, 400.0))
                                .unwrap_or(250.0);
                            g.zones = power_zones(ftp);
                        }
                    }
                    _ => {
                        if ui.small_button("Speed-style zones").clicked() {
                            g.zones = speed_zones(g.max);
                        }
                    }
                }
                if !g.zones.is_empty() && ui.small_button("Clear").clicked() {
                    g.zones.clear();
                }
            });
            if !g.zones.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    prop_label(ui, "Zones affect:");
                    let t = &mut g.zone_targets;
                    ui.checkbox(&mut t.arc, "Arc");
                    ui.checkbox(&mut t.needle, "Needle");
                    ui.checkbox(&mut t.number, "Number");
                    ui.checkbox(&mut t.bar, "Bar");
                    ui.checkbox(&mut t.background, "Background");
                });
            }
        });
}
