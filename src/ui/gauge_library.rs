//! Gauge library: templates and draggable gauge presets with rendered preview icons.

use std::collections::HashMap;
use std::sync::OnceLock;

use egui::{Color32, RichText, Sense, TextureHandle, TextureOptions, vec2};

use super::theme;
use crate::app::application::GaugeApp;
use crate::gauges::RenderCtx;
use crate::gauges::library::{PresetId, Template, categories, make_preset};
use crate::gauges::model::GaugeId;
use crate::render::compositor::rasterize_gauge;
use crate::telemetry::{SyncSettings, Track, UnitSystem};

const ICON_W: f32 = 64.0;
const ICON_H: f32 = 40.0;

fn demo_track() -> &'static Option<Track> {
    static T: OnceLock<Option<Track>> = OnceLock::new();
    T.get_or_init(|| {
        crate::telemetry::fit::parse(
            &crate::sample::synthetic_ride_fit(1_778_923_800, 600),
            "demo",
        )
        .ok()
    })
}

fn icon(
    ctx: &egui::Context,
    icons: &mut HashMap<PresetId, TextureHandle>,
    p: PresetId,
) -> Option<TextureHandle> {
    if let Some(t) = icons.get(&p) {
        return Some(t.clone());
    }
    let track = demo_track().as_ref();
    let mut g = make_preset(p, GaugeId(0), (1920.0, 1080.0), track, UnitSystem::Metric);
    g.style.shadow = false;
    let sync = SyncSettings::default();
    let rctx = RenderCtx {
        track,
        sync: &sync,
        video_t: 300.0,
        units: UnitSystem::Metric,
    };
    let ppp = ctx.pixels_per_point();
    let scale = (ICON_W / g.placement.w).min(ICON_H / g.placement.h) * ppp;
    let r = rasterize_gauge(&g, &rctx, scale)?;
    let tex = ctx.load_texture(format!("preset-{p:?}"), r.image, TextureOptions::LINEAR);
    icons.insert(p, tex.clone());
    Some(tex)
}

/// Hover text for "Save Current Template", showing the name it will get.
fn save_hint(app: &GaugeApp) -> String {
    if app.state.project.gauges.is_empty() {
        return "Add gauges first, then save the layout as a reusable template".into();
    }
    let name = crate::project::templates::auto_name(
        app.state.project.template.as_deref(),
        &app.templates.templates,
    );
    format!("Save the current gauge layout and settings as \"{name}\"")
}

/// Template menu entries (toolbar combo and Project menu): built-in templates, saved user
/// templates, Save Current Template, and the replace option.
pub fn template_menu(app: &mut GaugeApp, ui: &mut egui::Ui) {
    for t in Template::ALL {
        if ui.button(t.name()).on_hover_text(t.description()).clicked() {
            app.apply_template(t);
            ui.close();
        }
    }
    if !app.templates.templates.is_empty() {
        ui.separator();
        ui.label(RichText::new("My Templates").small().weak());
        let entries: Vec<(String, String)> = app
            .templates
            .templates
            .iter()
            .map(|t| (t.name.clone(), t.description()))
            .collect();
        for (name, desc) in entries {
            if ui.button(&name).on_hover_text(desc).clicked() {
                app.apply_user_template(&name);
                ui.close();
            }
        }
    }
    ui.separator();
    let can_save = !app.state.project.gauges.is_empty();
    let hint = save_hint(app);
    if ui
        .add_enabled(can_save, egui::Button::new("💾  Save Current Template"))
        .on_hover_text(&hint)
        .on_disabled_hover_text(&hint)
        .clicked()
    {
        app.save_current_template();
        ui.close();
    }
    ui.checkbox(&mut app.ui.template_replace, "Replace existing gauges");
}

/// Saved user templates in the library panel: click to apply, right-click to rename or delete.
fn user_templates(app: &mut GaugeApp, ui: &mut egui::Ui) {
    let entries: Vec<(String, String)> = app
        .templates
        .templates
        .iter()
        .map(|t| (t.name.clone(), t.description()))
        .collect();
    if entries.is_empty() {
        return;
    }
    ui.label(RichText::new("My Templates").small().weak());
    ui.horizontal_wrapped(|ui| {
        for (name, desc) in entries {
            let resp = ui
                .button(RichText::new(&name).color(if ui.visuals().dark_mode {
                    theme::ACCENT_2
                } else {
                    egui::Color32::from_rgb(20, 100, 170)
                }))
                .on_hover_text(format!("{desc}\nRight-click to rename or delete"));
            if resp.clicked() {
                app.apply_user_template(&name);
            }
            resp.context_menu(|ui| {
                if app
                    .ui
                    .template_rename
                    .as_ref()
                    .is_none_or(|(o, _)| *o != name)
                {
                    app.ui.template_rename = Some((name.clone(), name.clone()));
                }
                ui.label(RichText::new("Name").small().weak());
                let mut submit = false;
                if let Some((_, edited)) = &mut app.ui.template_rename {
                    let r = ui.add(egui::TextEdit::singleline(edited).desired_width(180.0));
                    submit = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                }
                if (ui.button("Rename").clicked() || submit)
                    && let Some((old, new)) = app.ui.template_rename.clone()
                    && app.rename_user_template(&old, &new)
                {
                    app.ui.template_rename = None;
                    ui.close();
                }
                ui.separator();
                ui.menu_button("Delete", |ui| {
                    if ui
                        .button(RichText::new(format!("Delete \"{name}\"")).color(theme::ERROR))
                        .clicked()
                    {
                        app.delete_user_template(&name);
                        app.ui.template_rename = None;
                        ui.close();
                    }
                });
            });
        }
    });
}

pub fn show(app: &mut GaugeApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            super::widgets::section(ui, "Templates");
            ui.horizontal_wrapped(|ui| {
                for t in Template::ALL {
                    if ui.button(t.name()).on_hover_text(t.description()).clicked() {
                        app.apply_template(t);
                    }
                }
            });
            user_templates(app, ui);
            ui.horizontal_wrapped(|ui| {
                let can_save = !app.state.project.gauges.is_empty();
                let hint = save_hint(app);
                if ui
                    .add_enabled(can_save, egui::Button::new("💾  Save Current Template"))
                    .on_hover_text(&hint)
                    .on_disabled_hover_text(&hint)
                    .clicked()
                {
                    app.save_current_template();
                }
                if ui
                    .add_enabled(can_save, egui::Button::new("🗑  Clear All Gauges"))
                    .on_hover_text("Remove every gauge (Undo restores them)")
                    .clicked()
                {
                    app.clear_gauges();
                }
            });
            ui.checkbox(&mut app.ui.template_replace, "Replace existing gauges");
            ui.add_space(4.0);
            ui.label(
                RichText::new("Drag a gauge onto the video, or click to add it.")
                    .small()
                    .weak(),
            );

            let ctx = ui.ctx().clone();
            for (cat, presets) in categories() {
                egui::CollapsingHeader::new(RichText::new(cat).strong())
                    .default_open(matches!(cat, "Speed" | "Heart Rate" | "Route"))
                    .show(ui, |ui| {
                        for p in presets {
                            let missing = match (p.metric(), &app.state.track) {
                                (Some(crate::telemetry::Metric::Latitude), Some(t)) => {
                                    t.route.is_none()
                                }
                                (Some(m), Some(t)) => !t.has(m),
                                _ => false,
                            };
                            let tex = icon(&ctx, &mut app.ui_icons, p);
                            let id = egui::Id::new(("preset", format!("{p:?}")));
                            let inner = ui.dnd_drag_source(id, p, |ui| {
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui
                                        .allocate_exact_size(vec2(ICON_W, ICON_H), Sense::hover());
                                    ui.painter().rect_filled(
                                        rect,
                                        4.0,
                                        Color32::from_rgb(58, 72, 88),
                                    );
                                    if let Some(t) = &tex {
                                        let sz = t.size_vec2() / ctx.pixels_per_point();
                                        let k = (rect.width() / sz.x)
                                            .min(rect.height() / sz.y)
                                            .min(1.0);
                                        let r = egui::Rect::from_center_size(rect.center(), sz * k);
                                        ui.painter().image(
                                            t.id(),
                                            r,
                                            egui::Rect::from_min_max(
                                                egui::pos2(0.0, 0.0),
                                                egui::pos2(1.0, 1.0),
                                            ),
                                            Color32::WHITE,
                                        );
                                    }
                                    ui.vertical(|ui| {
                                        ui.label(p.name());
                                        if missing {
                                            ui.label(
                                                RichText::new("no data in this recording")
                                                    .small()
                                                    .color(theme::WARN),
                                            );
                                        }
                                    });
                                });
                            });
                            let resp = inner.response.interact(Sense::click());
                            if resp.clicked() {
                                app.add_preset(p, None);
                            }
                            resp.on_hover_text("Click to add · drag onto the video to place");
                        }
                    });
            }
        });
}
