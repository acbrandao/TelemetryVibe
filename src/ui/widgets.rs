//! Small reusable widgets.

use egui::{Color32, RichText, Ui};

use crate::gauges::model::Rgba;

pub fn to_color32(c: Rgba) -> Color32 {
    let [r, g, b, a] = c.0;
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

pub fn from_color32(c: Color32) -> Rgba {
    Rgba(c.to_srgba_unmultiplied())
}

/// Color button with alpha. Returns true when changed.
pub fn color_edit(ui: &mut Ui, c: &mut Rgba) -> bool {
    let mut c32 = to_color32(*c);
    let changed = egui::color_picker::color_edit_button_srgba(
        ui,
        &mut c32,
        egui::color_picker::Alpha::OnlyBlend,
    )
    .changed();
    if changed {
        *c = from_color32(c32);
    }
    changed
}

/// Two-column property grid row label.
pub fn prop_label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).color(ui.visuals().weak_text_color()));
}

/// Section header used in side panels.
pub fn section(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(
        RichText::new(text.to_uppercase())
            .small()
            .strong()
            .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(2.0);
}

/// Key/value row with a weak key.
pub fn kv(ui: &mut Ui, k: &str, v: impl Into<String>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(k).color(ui.visuals().weak_text_color()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(v.into());
        });
    });
}
