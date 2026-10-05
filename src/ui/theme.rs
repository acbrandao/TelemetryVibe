//! Visual themes: Dark (default), Light, Nord, Dracula, Pale Yellow, Solarized Dark, Gruvbox.

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke, Visuals};

use crate::project::autosave::Theme;

pub const ACCENT: Color32 = Color32::from_rgb(255, 106, 61);
pub const ACCENT_2: Color32 = Color32::from_rgb(80, 200, 255);
pub const GPS_COLOR: Color32 = Color32::from_rgb(64, 190, 160);
pub const VIDEO_COLOR: Color32 = Color32::from_rgb(90, 120, 200);
pub const GAUGE_COLOR: Color32 = Color32::from_rgb(200, 130, 60);
pub const WARN: Color32 = Color32::from_rgb(255, 196, 60);
pub const ERROR: Color32 = Color32::from_rgb(255, 90, 90);
pub const OK: Color32 = Color32::from_rgb(110, 210, 120);
/// Timeline trim (render range) handles and band.
pub const TRIM: Color32 = Color32::from_rgb(250, 204, 21);

static INTER: &[u8] = include_bytes!("../../assets/fonts/Inter.ttf");

/// Installs the Inter UI font (keeping egui's fonts as fallbacks for symbols).
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "Inter".into(),
        std::sync::Arc::new(FontData::from_static(INTER)),
    );
    if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
        list.insert(0, "Inter".into());
    }
    ctx.set_fonts(fonts);
}

/// Colors of one UI theme.
struct Palette {
    dark: bool,
    /// Behind the video.
    canvas: Color32,
    panel: Color32,
    window: Color32,
    /// Text fields, timeline tracks.
    extreme: Color32,
    faint: Color32,
    widget: Color32,
    hover: Color32,
    active: Color32,
    border: Color32,
    text: Color32,
    selection: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

fn palette(theme: Theme) -> Palette {
    match theme {
        Theme::Dark => Palette {
            dark: true,
            canvas: rgb(14, 15, 17),
            panel: rgb(28, 29, 33),
            window: rgb(32, 33, 38),
            extreme: rgb(18, 19, 22),
            faint: rgb(36, 37, 42),
            widget: rgb(44, 46, 52),
            hover: rgb(58, 60, 68),
            active: rgb(70, 72, 82),
            border: rgb(48, 50, 56),
            text: rgb(220, 222, 228),
            selection: rgb(170, 70, 40),
        },
        Theme::Light => Palette {
            dark: false,
            canvas: rgb(200, 202, 206),
            panel: rgb(238, 239, 242),
            window: rgb(248, 248, 250),
            extreme: rgb(255, 255, 255),
            faint: rgb(230, 231, 235),
            widget: rgb(222, 224, 228),
            hover: rgb(208, 211, 216),
            active: rgb(195, 198, 204),
            border: rgb(200, 202, 208),
            text: rgb(30, 32, 36),
            selection: rgb(255, 170, 140),
        },
        // https://www.nordtheme.com — Polar Night backgrounds, Snow Storm text, Frost accents.
        Theme::Nord => Palette {
            dark: true,
            canvas: rgb(36, 41, 51),
            panel: rgb(46, 52, 64),
            window: rgb(59, 66, 82),
            extreme: rgb(39, 44, 54),
            faint: rgb(59, 66, 82),
            widget: rgb(67, 76, 94),
            hover: rgb(76, 86, 106),
            active: rgb(94, 129, 172),
            border: rgb(76, 86, 106),
            text: rgb(236, 239, 244),
            selection: rgb(94, 129, 172),
        },
        // https://draculatheme.com
        Theme::Dracula => Palette {
            dark: true,
            canvas: rgb(30, 31, 41),
            panel: rgb(40, 42, 54),
            window: rgb(46, 48, 62),
            extreme: rgb(33, 34, 44),
            faint: rgb(52, 55, 70),
            widget: rgb(68, 71, 90),
            hover: rgb(85, 89, 112),
            active: rgb(98, 114, 164),
            border: rgb(68, 71, 90),
            text: rgb(248, 248, 242),
            selection: rgb(130, 100, 190),
        },
        // Warm, low-glare light theme.
        Theme::PaleYellow => Palette {
            dark: false,
            canvas: rgb(214, 206, 170),
            panel: rgb(250, 245, 220),
            window: rgb(255, 251, 232),
            extreme: rgb(255, 253, 242),
            faint: rgb(243, 236, 205),
            widget: rgb(238, 229, 190),
            hover: rgb(228, 217, 170),
            active: rgb(215, 202, 150),
            border: rgb(220, 208, 160),
            text: rgb(60, 52, 30),
            selection: rgb(240, 200, 110),
        },
        // https://ethanschoonover.com/solarized
        Theme::SolarizedDark => Palette {
            dark: true,
            canvas: rgb(0, 30, 38),
            panel: rgb(0, 43, 54),
            window: rgb(7, 54, 66),
            extreme: rgb(0, 36, 46),
            faint: rgb(7, 54, 66),
            widget: rgb(20, 70, 84),
            hover: rgb(30, 86, 102),
            active: rgb(42, 104, 122),
            border: rgb(30, 80, 95),
            text: rgb(238, 232, 213),
            selection: rgb(38, 139, 210),
        },
        // https://github.com/morhetz/gruvbox
        Theme::Gruvbox => Palette {
            dark: true,
            canvas: rgb(29, 32, 33),
            panel: rgb(40, 40, 40),
            window: rgb(50, 48, 47),
            extreme: rgb(29, 32, 33),
            faint: rgb(60, 56, 54),
            widget: rgb(80, 73, 69),
            hover: rgb(102, 92, 84),
            active: rgb(124, 111, 100),
            border: rgb(80, 73, 69),
            text: rgb(235, 219, 178),
            selection: rgb(214, 93, 14),
        },
    }
}

pub fn canvas_bg(theme: Theme) -> Color32 {
    palette(theme).canvas
}

pub fn apply(ctx: &egui::Context, theme: Theme) {
    let p = palette(theme);
    let mut v = if p.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let r = CornerRadius::same(5);
    v.panel_fill = p.panel;
    v.window_fill = p.window;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.extreme_bg_color = p.extreme;
    v.code_bg_color = p.extreme;
    v.faint_bg_color = p.faint;
    v.widgets.noninteractive.bg_fill = p.panel;
    v.widgets.noninteractive.weak_bg_fill = p.panel;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text.gamma_multiply(0.9));
    for (w, bg) in [
        (&mut v.widgets.inactive, p.widget),
        (&mut v.widgets.hovered, p.hover),
        (&mut v.widgets.active, p.active),
        (&mut v.widgets.open, p.widget),
    ] {
        w.bg_fill = bg;
        w.weak_bg_fill = bg;
        w.fg_stroke = Stroke::new(1.0, p.text);
    }
    v.selection.bg_fill = p.selection;
    v.selection.stroke = Stroke::new(1.0, if p.dark { Color32::WHITE } else { p.text });
    v.hyperlink_color = ACCENT_2;
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = r;
    }
    v.window_corner_radius = CornerRadius::same(8);
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(8.0, 4.0);
        s.spacing.interact_size.y = 22.0;
    });
}
