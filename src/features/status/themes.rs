//! Theme system — ANSI color definitions for the statusline

use std::collections::HashMap;
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// Low-level ANSI building
// ---------------------------------------------------------------------------

pub type RGB = (u8, u8, u8);

/// 24-bit true color foreground
pub fn fg(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[38;2;{r};{g};{b}m")
}

/// 256-color foreground
pub fn fg256(n: u8) -> String {
    format!("\x1b[38;5;{n}m")
}

pub const RESET: &str = "\x1b[0m";
pub const DIM: &str = "\x1b[38;5;240m";

// ---------------------------------------------------------------------------
// Theme structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ModelColors {
    pub anchor: RGB,
    pub warm_shift: RGB,
    pub cool_shift: RGB,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,

    // Decorative slots
    pub border: String,
    pub border_off: String,
    pub pwd: String,
    pub branch: String,
    pub commit: String,
    pub session: String,
    pub skills: String,
    pub time: String,
    pub tok: String,
    pub tok_dim: String,
    pub tok_day: String,
    pub tok_day_dim: String,
    pub cost: String,
    pub bar_fill: String,
    pub bar_empty: String,
    pub dim_green: String,
    pub label: String,
    pub ctx: String,
    pub ctx_dim: String,
    pub white_brt: String,
    pub arrow: String,
    pub dirty: String,
    pub icon_path: String,
    pub tok_icon: String,
    pub model: String,

    // Ladder
    pub safe: String,
    pub warn: String,
    pub alert: String,
    pub yellow: String,
    pub tok_arrow: String,

    // Per-model pill identity
    pub models: HashMap<String, ModelColors>,

    // Pill foreground
    pub pill_fg_dark: RGB,
    pub pill_fg_light: RGB,

    // Gradients
    pub grad_stops: Vec<(f64, RGB)>,
    pub grey_rgb: RGB,
    pub spark_stops: Vec<(f64, RGB)>,
    pub spec_gradients: Vec<(RGB, RGB, RGB)>,
    pub spec_empty_ansi: String,
    pub bar_empty_rgb: RGB,
}

// ---------------------------------------------------------------------------
// Theme builders (runtime)
// ---------------------------------------------------------------------------

fn make_model_colors(
    anchor: (u8, u8, u8),
    warm: (u8, u8, u8),
    cool: (u8, u8, u8),
    label: String,
) -> ModelColors {
    ModelColors {
        anchor,
        warm_shift: warm,
        cool_shift: cool,
        label,
    }
}

fn make_theme(name: &str) -> Theme {
    let models = HashMap::from([
        (
            "opus".to_string(),
            make_model_colors((255, 255, 0), (255, 165, 0), (180, 230, 60), fg256(226)),
        ),
        (
            "sonnet".to_string(),
            make_model_colors((135, 215, 135), (44, 208, 168), (44, 140, 80), fg256(114)),
        ),
        (
            "haiku".to_string(),
            make_model_colors((95, 175, 255), (123, 230, 255), (74, 110, 224), fg256(75)),
        ),
        (
            "minimax".to_string(),
            make_model_colors((180, 100, 255), (220, 140, 255), (130, 60, 200), fg256(183)),
        ),
        (
            "qwopus".to_string(),
            make_model_colors((255, 200, 80), (255, 160, 50), (200, 150, 40), fg256(214)),
        ),
        (
            "other".to_string(),
            make_model_colors(
                (215, 175, 255),
                (240, 165, 224),
                (138, 111, 214),
                fg256(183),
            ),
        ),
    ]);

    Theme {
        name: name.to_string(),
        border: fg256(244),
        border_off: fg256(242),
        pwd: fg256(75),
        branch: fg256(114),
        commit: fg256(244),
        session: fg256(244),
        skills: fg256(222),
        time: fg256(244),
        tok: fg256(116),
        tok_dim: fg256(244),
        tok_day: fg256(109),
        tok_day_dim: fg256(240),
        cost: fg256(210),
        bar_fill: fg256(114),
        bar_empty: fg256(238),
        dim_green: fg256(77),
        label: fg256(244),
        ctx: fg256(216),
        ctx_dim: fg256(248),
        white_brt: fg256(15),
        arrow: fg256(46),
        dirty: fg256(214),
        icon_path: fg256(117),
        tok_icon: fg256(11),
        model: fg256(183),
        safe: fg256(114),
        warn: fg256(214),
        alert: fg256(167),
        yellow: fg256(226),
        tok_arrow: fg256(226),
        models,
        pill_fg_dark: (15, 15, 15),
        pill_fg_light: (235, 235, 235),
        grad_stops: vec![
            (0.00, (40, 210, 80)),
            (0.25, (240, 230, 20)),
            (0.50, (255, 140, 20)),
            (0.75, (220, 40, 50)),
            (1.00, (170, 60, 210)),
        ],
        grey_rgb: (108, 108, 108),
        spark_stops: vec![
            (0.00, (179, 46, 32)),
            (0.50, (200, 55, 40)),
            (1.00, (204, 65, 51)),
        ],
        spec_gradients: vec![
            ((20, 60, 200), (20, 180, 240), (100, 240, 255)),
            ((200, 80, 10), (245, 30, 100), (255, 160, 80)),
            ((10, 120, 40), (80, 210, 20), (200, 255, 60)),
            ((80, 20, 200), (160, 60, 255), (220, 160, 255)),
            ((160, 20, 10), (240, 120, 10), (255, 220, 30)),
            ((20, 80, 160), (60, 180, 240), (210, 240, 255)),
            ((120, 50, 10), (200, 120, 20), (255, 200, 80)),
            ((160, 10, 50), (240, 60, 130), (255, 180, 210)),
            ((10, 110, 90), (20, 210, 150), (120, 255, 200)),
            ((50, 10, 160), (180, 20, 220), (255, 100, 240)),
            ((140, 10, 180), (40, 100, 255), (20, 220, 200)),
            ((200, 160, 10), (240, 80, 20), (180, 20, 80)),
        ],
        spec_empty_ansi: fg256(233),
        bar_empty_rgb: (68, 68, 68),
    }
}

// ---------------------------------------------------------------------------
// Bundled themes
// ---------------------------------------------------------------------------

static CLAUDE_DARK: LazyLock<Theme> = LazyLock::new(|| make_theme("claude-dark"));
static CLAUDE_LIGHT: LazyLock<Theme> = LazyLock::new(|| make_claude_light());
static CATPPUCCIN_LATTE: LazyLock<Theme> = LazyLock::new(|| make_catppuccin_latte());
static CATPPUCCIN_MOCHA: LazyLock<Theme> = LazyLock::new(|| make_catppuccin_mocha());

fn make_claude_light() -> Theme {
    let models = HashMap::from([
        (
            "opus".to_string(),
            make_model_colors(
                (212, 160, 23),
                (200, 120, 20),
                (170, 175, 40),
                fg(150, 110, 20),
            ),
        ),
        (
            "sonnet".to_string(),
            make_model_colors((110, 175, 110), (60, 170, 130), (50, 130, 80), fg256(28)),
        ),
        (
            "haiku".to_string(),
            make_model_colors(
                (80, 145, 210),
                (100, 175, 215),
                (60, 95, 180),
                fg(0, 95, 175),
            ),
        ),
        (
            "minimax".to_string(),
            make_model_colors((150, 80, 210), (190, 120, 230), (110, 50, 170), fg256(183)),
        ),
        (
            "qwopus".to_string(),
            make_model_colors((200, 160, 60), (220, 140, 40), (160, 120, 30), fg256(214)),
        ),
        (
            "other".to_string(),
            make_model_colors((170, 130, 195), (190, 130, 180), (115, 90, 170), fg256(96)),
        ),
    ]);

    Theme {
        name: "claude-light".to_string(),
        border: fg256(244),
        border_off: fg256(246),
        pwd: fg(0, 95, 175),
        branch: fg256(28),
        commit: fg256(243),
        session: fg256(243),
        skills: fg(160, 110, 30),
        time: fg256(243),
        tok: fg(40, 110, 150),
        tok_dim: fg256(245),
        tok_day: fg(70, 120, 130),
        tok_day_dim: fg256(247),
        cost: fg(175, 80, 80),
        bar_fill: fg256(28),
        bar_empty: fg256(252),
        dim_green: fg(60, 130, 70),
        label: fg256(243),
        ctx: fg(180, 100, 50),
        ctx_dim: fg256(245),
        white_brt: fg256(232),
        arrow: fg(0, 135, 0),
        dirty: fg(180, 110, 20),
        icon_path: fg(40, 110, 160),
        tok_icon: fg(160, 130, 20),
        model: fg256(96),
        safe: fg256(28),
        warn: fg(180, 110, 20),
        alert: fg(170, 50, 50),
        yellow: fg(160, 130, 20),
        tok_arrow: fg(0, 0, 0),
        models,
        pill_fg_dark: (10, 10, 10),
        pill_fg_light: (250, 250, 250),
        grad_stops: vec![
            (0.00, (30, 158, 60)),
            (0.25, (180, 172, 15)),
            (0.50, (191, 105, 15)),
            (0.75, (165, 30, 38)),
            (1.00, (128, 45, 158)),
        ],
        grey_rgb: (160, 160, 160),
        spark_stops: vec![
            (0.00, (145, 35, 25)),
            (0.50, (165, 45, 32)),
            (1.00, (175, 55, 42)),
        ],
        spec_gradients: vec![
            ((15, 45, 150), (15, 135, 180), (75, 180, 191)),
            ((150, 60, 8), (184, 22, 75), (191, 120, 60)),
            ((8, 90, 30), (60, 158, 15), (150, 191, 45)),
            ((60, 15, 150), (120, 45, 191), (165, 120, 191)),
            ((120, 15, 8), (180, 90, 8), (191, 165, 23)),
            ((15, 60, 120), (45, 135, 180), (158, 180, 191)),
            ((90, 38, 8), (150, 90, 15), (191, 150, 60)),
            ((120, 8, 38), (180, 45, 98), (191, 135, 158)),
            ((8, 82, 68), (15, 158, 112), (90, 191, 150)),
            ((38, 8, 120), (135, 15, 165), (191, 75, 180)),
            ((105, 8, 135), (30, 75, 191), (15, 165, 150)),
            ((150, 120, 8), (180, 60, 15), (135, 15, 60)),
        ],
        spec_empty_ansi: fg256(254),
        bar_empty_rgb: (208, 208, 208),
    }
}

fn make_catppuccin_latte() -> Theme {
    let models = HashMap::from([
        (
            "opus".to_string(),
            make_model_colors(
                (223, 142, 29),
                (254, 100, 11),
                (64, 160, 43),
                fg(223, 142, 29),
            ),
        ),
        (
            "sonnet".to_string(),
            make_model_colors(
                (64, 160, 43),
                (23, 146, 153),
                (30, 102, 245),
                fg(64, 160, 43),
            ),
        ),
        (
            "haiku".to_string(),
            make_model_colors(
                (32, 159, 181),
                (4, 165, 229),
                (30, 102, 245),
                fg(30, 102, 245),
            ),
        ),
        (
            "minimax".to_string(),
            make_model_colors(
                (160, 80, 220),
                (200, 120, 255),
                (120, 50, 180),
                fg(136, 57, 239),
            ),
        ),
        (
            "qwopus".to_string(),
            make_model_colors(
                (220, 170, 50),
                (250, 140, 30),
                (180, 130, 30),
                fg(220, 170, 50),
            ),
        ),
        (
            "other".to_string(),
            make_model_colors(
                (234, 118, 203),
                (136, 57, 239),
                (114, 135, 253),
                fg(136, 57, 239),
            ),
        ),
    ]);

    Theme {
        name: "catppuccin-latte".to_string(),
        border: fg(140, 143, 161),
        border_off: fg(156, 160, 176),
        pwd: fg(30, 102, 245),
        branch: fg(64, 160, 43),
        commit: fg(108, 111, 133),
        session: fg(108, 111, 133),
        skills: fg(223, 142, 29),
        time: fg(108, 111, 133),
        tok: fg(23, 146, 153),
        tok_dim: fg(140, 143, 161),
        tok_day: fg(32, 159, 181),
        tok_day_dim: fg(156, 160, 176),
        cost: fg(230, 69, 83),
        bar_fill: fg(64, 160, 43),
        bar_empty: fg(188, 192, 204),
        dim_green: fg(64, 160, 43),
        label: fg(140, 143, 161),
        ctx: fg(254, 100, 11),
        ctx_dim: fg(124, 127, 147),
        white_brt: fg(76, 79, 105),
        arrow: fg(64, 160, 43),
        dirty: fg(254, 100, 11),
        icon_path: fg(32, 159, 181),
        tok_icon: fg(223, 142, 29),
        model: fg(136, 57, 239),
        safe: fg(64, 160, 43),
        warn: fg(254, 100, 11),
        alert: fg(210, 15, 57),
        yellow: fg(223, 142, 29),
        tok_arrow: fg(223, 142, 29),
        models,
        pill_fg_dark: (30, 30, 46),
        pill_fg_light: (239, 241, 245),
        grad_stops: vec![
            (0.00, (64, 160, 43)),
            (0.25, (223, 142, 29)),
            (0.50, (254, 100, 11)),
            (0.75, (210, 15, 57)),
            (1.00, (136, 57, 239)),
        ],
        grey_rgb: (156, 160, 176),
        spark_stops: vec![
            (0.00, (230, 69, 83)),
            (0.50, (210, 15, 57)),
            (1.00, (254, 100, 11)),
        ],
        spec_gradients: vec![
            ((32, 159, 181), (30, 102, 245), (4, 165, 229)),
            ((254, 100, 11), (230, 69, 83), (223, 142, 29)),
            ((64, 160, 43), (23, 146, 153), (223, 142, 29)),
            ((136, 57, 239), (114, 135, 253), (234, 118, 203)),
            ((210, 15, 57), (254, 100, 11), (223, 142, 29)),
            ((32, 159, 181), (4, 165, 229), (188, 192, 204)),
            ((254, 100, 11), (223, 142, 29), (230, 69, 83)),
            ((234, 118, 203), (220, 138, 120), (221, 120, 120)),
            ((23, 146, 153), (64, 160, 43), (4, 165, 229)),
            ((136, 57, 239), (234, 118, 203), (114, 135, 253)),
            ((23, 146, 153), (32, 159, 181), (136, 57, 239)),
            ((210, 15, 57), (230, 69, 83), (254, 100, 11)),
        ],
        spec_empty_ansi: fg256(254),
        bar_empty_rgb: (188, 192, 204),
    }
}

fn make_catppuccin_mocha() -> Theme {
    let models = HashMap::from([
        (
            "opus".to_string(),
            make_model_colors(
                (249, 226, 175),
                (250, 179, 135),
                (166, 227, 161),
                fg(249, 226, 175),
            ),
        ),
        (
            "sonnet".to_string(),
            make_model_colors(
                (166, 227, 161),
                (148, 226, 213),
                (137, 220, 235),
                fg(166, 227, 161),
            ),
        ),
        (
            "haiku".to_string(),
            make_model_colors(
                (137, 180, 250),
                (116, 199, 236),
                (180, 190, 254),
                fg(137, 180, 250),
            ),
        ),
        (
            "minimax".to_string(),
            make_model_colors(
                (180, 130, 250),
                (210, 160, 255),
                (140, 90, 200),
                fg(203, 166, 247),
            ),
        ),
        (
            "qwopus".to_string(),
            make_model_colors(
                (245, 200, 100),
                (250, 170, 60),
                (200, 160, 50),
                fg(249, 226, 175),
            ),
        ),
        (
            "other".to_string(),
            make_model_colors(
                (203, 166, 247),
                (245, 194, 231),
                (180, 190, 254),
                fg(203, 166, 247),
            ),
        ),
    ]);

    Theme {
        name: "catppuccin-mocha".to_string(),
        border: fg(127, 132, 156),
        border_off: fg(108, 112, 134),
        pwd: fg(137, 180, 250),
        branch: fg(166, 227, 161),
        commit: fg(166, 173, 200),
        session: fg(166, 173, 200),
        skills: fg(249, 226, 175),
        time: fg(166, 173, 200),
        tok: fg(148, 226, 213),
        tok_dim: fg(127, 132, 156),
        tok_day: fg(116, 199, 236),
        tok_day_dim: fg(108, 112, 134),
        cost: fg(235, 160, 172),
        bar_fill: fg(166, 227, 161),
        bar_empty: fg(69, 71, 90),
        dim_green: fg(166, 227, 161),
        label: fg(127, 132, 156),
        ctx: fg(250, 179, 135),
        ctx_dim: fg(166, 173, 200),
        white_brt: fg(205, 214, 244),
        arrow: fg(166, 227, 161),
        dirty: fg(250, 179, 135),
        icon_path: fg(116, 199, 236),
        tok_icon: fg(249, 226, 175),
        model: fg(203, 166, 247),
        safe: fg(166, 227, 161),
        warn: fg(250, 179, 135),
        alert: fg(243, 139, 168),
        yellow: fg(249, 226, 175),
        tok_arrow: fg(249, 226, 175),
        models,
        pill_fg_dark: (17, 17, 27),
        pill_fg_light: (205, 214, 244),
        grad_stops: vec![
            (0.00, (166, 227, 161)),
            (0.25, (249, 226, 175)),
            (0.50, (250, 179, 135)),
            (0.75, (243, 139, 168)),
            (1.00, (203, 166, 247)),
        ],
        grey_rgb: (108, 112, 134),
        spark_stops: vec![
            (0.00, (235, 160, 172)),
            (0.50, (243, 139, 168)),
            (1.00, (250, 179, 135)),
        ],
        spec_gradients: vec![
            ((116, 199, 236), (137, 180, 250), (137, 220, 235)),
            ((250, 179, 135), (235, 160, 172), (249, 226, 175)),
            ((166, 227, 161), (148, 226, 213), (249, 226, 175)),
            ((203, 166, 247), (180, 190, 254), (245, 194, 231)),
            ((243, 139, 168), (250, 179, 135), (249, 226, 175)),
            ((116, 199, 236), (137, 220, 235), (180, 190, 254)),
            ((250, 179, 135), (249, 226, 175), (235, 160, 172)),
            ((245, 194, 231), (245, 224, 220), (242, 205, 205)),
            ((148, 226, 213), (166, 227, 161), (137, 220, 235)),
            ((203, 166, 247), (245, 194, 231), (180, 190, 254)),
            ((148, 226, 213), (116, 199, 236), (203, 166, 247)),
            ((243, 139, 168), (235, 160, 172), (250, 179, 135)),
        ],
        spec_empty_ansi: fg256(233),
        bar_empty_rgb: (69, 71, 90),
    }
}

// ---------------------------------------------------------------------------
// Theme resolution
// ---------------------------------------------------------------------------

pub fn resolve_theme(name: &str) -> Theme {
    match name {
        "claude-light" => CLAUDE_LIGHT.clone(),
        "catppuccin-latte" => CATPPUCCIN_LATTE.clone(),
        "catppuccin-mocha" => CATPPUCCIN_MOCHA.clone(),
        _ => CLAUDE_DARK.clone(),
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

/// Interpolate a color along the gradient stops
pub fn gradient_color(theme: &Theme, ratio: f64) -> RGB {
    let stops = &theme.grad_stops;
    if stops.is_empty() {
        return theme.grey_rgb;
    }

    let ratio = ratio.max(0.0).min(1.0);

    for i in 0..stops.len() - 1 {
        if ratio >= stops[i].0 && ratio <= stops[i + 1].0 {
            let t = (ratio - stops[i].0) / (stops[i + 1].0 - stops[i].0);
            let lower = stops[i].1;
            let upper = stops[i + 1].1;
            return (
                (lower.0 as f64 + t * (upper.0 as f64 - lower.0 as f64)) as u8,
                (lower.1 as f64 + t * (upper.1 as f64 - lower.1 as f64)) as u8,
                (lower.2 as f64 + t * (upper.2 as f64 - lower.2 as f64)) as u8,
            );
        }
    }

    stops[stops.len() - 1].1
}

/// Decide pill foreground based on background luminance
pub fn pill_fg(bg: RGB, theme: &Theme) -> String {
    let luminance = (bg.0 as f64 * 0.299 + bg.1 as f64 * 0.587 + bg.2 as f64 * 0.114) / 255.0;
    let (r, g, b) = if luminance > 0.5 {
        theme.pill_fg_dark
    } else {
        theme.pill_fg_light
    };
    fg(r, g, b)
}

/// Interpolate between two RGB colors
pub fn lerp_rgb(a: RGB, b: RGB, t: f64) -> RGB {
    (
        (a.0 as f64 + t * (b.0 as f64 - a.0 as f64)) as u8,
        (a.1 as f64 + t * (b.1 as f64 - a.1 as f64)) as u8,
        (a.2 as f64 + t * (b.2 as f64 - a.2 as f64)) as u8,
    )
}
