use fltk::{
    app,
    button::Button,
    draw,
    enums::{Align, Color, FrameType, Mode},
    frame::Frame,
    group::Flex,
    prelude::*,
    window::{DoubleWindow, SingleWindow},
};

const BG_COLOR: Color = Color::from_rgb(24, 28, 30);
const FG_COLOR: Color = Color::from_rgb(230, 226, 216);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeWindowKind {
    Double,
    Single,
}

impl ProbeWindowKind {
    fn from_name(name: Option<&str>) -> Self {
        match name.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            Some("single") => Self::Single,
            Some("double") | None => Self::Double,
            Some(_) => Self::Double,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbePaintMode {
    Normal,
    OpaqueDraw,
}

impl ProbePaintMode {
    fn from_name(name: Option<&str>) -> Self {
        match name.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            Some("opaque-draw") => Self::OpaqueDraw,
            Some("normal") | None => Self::Normal,
            Some(_) => Self::Normal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeVisualMode {
    Default,
    Rgb8,
}

impl ProbeVisualMode {
    fn from_name(name: Option<&str>) -> Self {
        match name.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            Some("rgb8") => Self::Rgb8,
            Some("default") | None => Self::Default,
            Some(_) => Self::Default,
        }
    }

    fn apply(self) {
        if self == Self::Rgb8
            && let Err(err) = app::set_visual(Mode::Rgb8)
        {
            eprintln!("SLIPPY_PROBE_VISUAL=rgb8 failed: {err}");
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ProbeOptions {
    window_kind: ProbeWindowKind,
    paint_mode: ProbePaintMode,
    visual_mode: ProbeVisualMode,
}

impl ProbeOptions {
    fn from_env() -> Self {
        Self {
            window_kind: ProbeWindowKind::from_name(
                std::env::var("SLIPPY_PROBE_WINDOW").ok().as_deref(),
            ),
            paint_mode: ProbePaintMode::from_name(
                std::env::var("SLIPPY_PROBE_PAINT").ok().as_deref(),
            ),
            visual_mode: ProbeVisualMode::from_name(
                std::env::var("SLIPPY_PROBE_VISUAL").ok().as_deref(),
            ),
        }
    }
}

fn main() {
    let options = ProbeOptions::from_env();
    let app = app::App::default().with_scheme(app::Scheme::Gtk);
    options.visual_mode.apply();
    app::background(24, 28, 30);
    app::foreground(230, 226, 216);

    let mode = std::env::var("SLIPPY_PROBE_MODE").unwrap_or_else(|_| "flex".to_string());

    match options.window_kind {
        ProbeWindowKind::Double => run_probe::<DoubleWindow>(app, &mode, options),
        ProbeWindowKind::Single => run_probe::<SingleWindow>(app, &mode, options),
    }
}

fn run_probe<W>(app: app::App, mode: &str, options: ProbeOptions)
where
    W: Default + WidgetBase + WidgetExt + GroupExt + WindowExt + 'static,
{
    let mut window = W::default()
        .with_size(900, 620)
        .with_label("Slippy FLTK resize probe");
    window.size_range(320, 220, 0, 0);
    window.set_color(BG_COLOR);
    window.set_frame(FrameType::FlatBox);
    if options.paint_mode == ProbePaintMode::OpaqueDraw {
        install_opaque_draw(&mut window);
    }

    match mode {
        "empty" => {}
        "frame" => build_frame_surface(&mut window),
        _ => build_flex_surface(&mut window),
    }

    window.end();
    window.show();

    eprintln!("SLIPPY_PROBE_MODE={mode}");
    eprintln!(
        "FLTK_BACKEND={}",
        std::env::var("FLTK_BACKEND").unwrap_or_else(|_| "<unset>".to_string())
    );
    eprintln!("SLIPPY_PROBE_WINDOW={:?}", options.window_kind);
    eprintln!("SLIPPY_PROBE_PAINT={:?}", options.paint_mode);
    eprintln!("SLIPPY_PROBE_VISUAL={:?}", options.visual_mode);
    eprintln!("Try SLIPPY_PROBE_MODE: empty, frame, flex");
    eprintln!("Try SLIPPY_PROBE_WINDOW: double, single");
    eprintln!("Try SLIPPY_PROBE_PAINT: normal, opaque-draw");
    eprintln!("Try SLIPPY_PROBE_VISUAL: default, rgb8");
    eprintln!("Manually drag-resize this window continuously and compare flicker.");

    app.run().expect("FLTK event loop failed");
}

fn install_opaque_draw<W: WidgetBase + WidgetExt>(window: &mut W) {
    window.draw(|w| {
        draw::set_draw_color(BG_COLOR);
        draw::draw_rectf(0, 0, w.w(), w.h());
    });
}

fn build_frame_surface<W: GroupExt>(window: &mut W) {
    let mut surface = Frame::default_fill().with_label("single full-window Frame");
    surface.set_frame(FrameType::FlatBox);
    surface.set_color(Color::from_rgb(38, 44, 48));
    surface.set_label_color(FG_COLOR);
    surface.set_label_size(18);
    surface.set_align(Align::Center | Align::Inside);
    window.resizable(&surface);
}

fn build_flex_surface<W: GroupExt>(window: &mut W) {
    let mut root = Flex::default_fill().column();
    root.set_frame(FrameType::FlatBox);
    root.set_color(BG_COLOR);
    root.set_margin(18);
    root.set_pad(12);

    let mut header = Frame::default().with_label("Flex layout probe");
    header.set_frame(FrameType::FlatBox);
    header.set_color(Color::from_rgb(55, 68, 72));
    header.set_label_color(Color::from_rgb(238, 231, 214));
    header.set_label_size(18);
    header.set_align(Align::Center | Align::Inside);

    let mut row = Flex::default().row();
    row.set_frame(FrameType::FlatBox);
    row.set_color(Color::from_rgb(82, 93, 89));
    row.set_pad(10);

    let mut left = Frame::default().with_label("left panel");
    left.set_frame(FrameType::FlatBox);
    left.set_color(Color::from_rgb(35, 41, 45));
    left.set_label_color(FG_COLOR);
    left.set_align(Align::Center | Align::Inside);

    let mut right = Frame::default().with_label("right panel");
    right.set_frame(FrameType::FlatBox);
    right.set_color(Color::from_rgb(35, 41, 45));
    right.set_label_color(FG_COLOR);
    right.set_align(Align::Center | Align::Inside);

    row.end();

    let mut footer = Flex::default().row();
    footer.set_frame(FrameType::FlatBox);
    footer.set_color(BG_COLOR);
    footer.set_pad(8);

    let mut action = Button::default().with_label("button");
    action.set_frame(FrameType::FlatBox);
    action.set_color(Color::from_rgb(205, 132, 87));
    action.set_label_color(Color::from_rgb(30, 26, 22));

    let mut status = Frame::default().with_label("no custom resize callback");
    status.set_frame(FrameType::FlatBox);
    status.set_color(Color::from_rgb(38, 44, 48));
    status.set_label_color(Color::from_rgb(181, 174, 160));
    status.set_align(Align::Left | Align::Inside);

    footer.fixed(&action, 120);
    footer.end();

    root.fixed(&header, 54);
    root.fixed(&footer, 42);
    root.end();
    window.resizable(&root);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_window_kind_variants() {
        assert_eq!(ProbeWindowKind::from_name(None), ProbeWindowKind::Double);
        assert_eq!(
            ProbeWindowKind::from_name(Some("single")),
            ProbeWindowKind::Single
        );
        assert_eq!(
            ProbeWindowKind::from_name(Some("double")),
            ProbeWindowKind::Double
        );
    }

    #[test]
    fn parses_paint_mode_variants() {
        assert_eq!(ProbePaintMode::from_name(None), ProbePaintMode::Normal);
        assert_eq!(
            ProbePaintMode::from_name(Some("opaque-draw")),
            ProbePaintMode::OpaqueDraw
        );
    }

    #[test]
    fn parses_visual_mode_variants() {
        assert_eq!(ProbeVisualMode::from_name(None), ProbeVisualMode::Default);
        assert_eq!(
            ProbeVisualMode::from_name(Some("rgb8")),
            ProbeVisualMode::Rgb8
        );
    }
}
