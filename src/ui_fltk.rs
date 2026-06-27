use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    thread,
};

use arboard::Clipboard;
use fltk::{
    app,
    button::Button,
    draw,
    enums::{Align, Color, Event, Font, FrameType, Shortcut},
    frame::Frame,
    group::{Flex, FlexType, Scroll, ScrollType},
    prelude::*,
    text::{TextBuffer, TextEditor},
    window::Window,
};

use crate::{
    app_state::{AppState, STATUS_CLEARED},
    config::{
        AppConfig, ConfigLoadStatus, MIN_HEIGHT, MIN_WIDTH, Theme, config_path,
        load_config_from_path, save_config_to_path,
    },
    diff_core::{DiffOptions, render_unified_diff},
};

#[derive(Debug, Clone, Copy)]
struct Palette {
    surface: Color,
    pane: Color,
    text: Color,
    muted: Color,
    border: Color,
    primary: Color,
    primary_text: Color,
    insert_text: Color,
    delete_text: Color,
    replace_text: Color,
    status_bg: Color,
    secondary_button: Color,
    insert_bg: Color,
    delete_bg: Color,
    replace_bg: Color,
    inline_insert_bg: Color,
    inline_delete_bg: Color,
    header_bg: Color,
}

const ACTION_BAR_HEIGHT: i32 = 34;
const DIFF_TOOLBAR_HEIGHT: i32 = 32;
const OVERVIEW_RAIL_WIDTH: i32 = 14;
const STATUS_BAR_HEIGHT: i32 = 26;
const ROOT_MARGIN: i32 = 8;
const ROOT_PAD: i32 = 8;
const PANE_GAP: i32 = 8;
const STACK_INPUT_WIDTH: i32 = 760;
const INPUT_GUTTER_WIDTH: i32 = 48;
const INPUT_LINE_HEIGHT: i32 = 18;
const DIFF_OLD_GUTTER_WIDTH: i32 = 52;
const DIFF_NEW_GUTTER_WIDTH: i32 = 52;
const DIFF_MARKER_WIDTH: i32 = 26;
const DIFF_HEADER_HEIGHT: i32 = 28;
const DIFF_ROW_HEIGHT: i32 = 22;
const DIFF_CANVAS_MIN_WIDTH: i32 = 760;
const DIFF_TEXT_LEFT_PAD: i32 = 10;

fn diff_options_from_config(overrides: &crate::config::DiffOverrides) -> DiffOptions {
    let mut o = DiffOptions::default();
    if let Some(v) = overrides.debounce_ms {
        o.debounce_ms = v;
    }
    if let Some(v) = overrides.auto_diff_max_bytes {
        o.auto_diff_max_bytes = v;
    }
    if let Some(v) = overrides.auto_diff_max_lines {
        o.auto_diff_max_lines = v;
    }
    if let Some(v) = overrides.unified_context_radius {
        o.unified_context_radius = v;
    }
    if let Some(v) = overrides.inline_max_changed_ratio {
        o.inline_max_changed_ratio = v;
    }
    if let Some(v) = overrides.display_full_context_max_lines {
        o.display_full_context_max_lines = v;
    }
    if let Some(v) = overrides.similarity_pairing_max_lines {
        o.similarity_pairing_max_lines = v;
    }
    if let Some(v) = overrides.alignment_band {
        o.alignment_band = v;
    }
    o
}

struct UiHandles {
    left_editor: TextEditor,
    right_editor: TextEditor,
    left_buffer: TextBuffer,
    right_buffer: TextBuffer,
    left_gutter: Frame,
    right_gutter: Frame,
    left_gutter_top_line: Rc<Cell<i32>>,
    right_gutter_top_line: Rc<Cell<i32>>,
    diff_scroll: Scroll,
    diff_canvas: Frame,
    diff_view: Rc<RefCell<crate::diff_view::RenderedDiffView>>,
    stale_diff_notice: Rc<Cell<bool>>,
    diff_summary: Frame,
    overview_rail: Frame,
    status: Frame,
    copy_diff: Button,
    pin: Button,
}

#[derive(Debug, Clone)]
enum UiMessage {
    DiffReady(crate::app_state::DiffResult),
}

pub fn run() -> Result<(), FltkError> {
    let app = app::App::default().with_scheme(app::Scheme::Gtk);

    let config_file = config_path().ok();
    let config = config_file
        .as_ref()
        .map(load_config_from_path)
        .unwrap_or_else(|| crate::config::ConfigLoadResult {
            config: AppConfig::default(),
            status: ConfigLoadStatus::Missing,
        });
    let options = diff_options_from_config(&config.config.diff);
    let state = Rc::new(RefCell::new(AppState::new(options)));
    if config.status == ConfigLoadStatus::Invalid {
        state
            .borrow_mut()
            .set_status("Config invalid; using defaults.");
    }
    let palette = palette_for(config.config.theme);
    let (bg_r, bg_g, bg_b) = rgb(palette.surface);
    let (fg_r, fg_g, fg_b) = rgb(palette.text);
    app::background(bg_r, bg_g, bg_b);
    app::foreground(fg_r, fg_g, fg_b);
    let debounce_generation = Rc::new(Cell::new(0_u64));
    let suppress_buffer_events = Rc::new(Cell::new(false));
    let pinned = Rc::new(Cell::new(false));
    let (sender, receiver) = app::channel::<UiMessage>();

    let mut window = Window::default()
        .with_size(config.config.width, config.config.height)
        .with_label("Slippy");
    window.make_resizable(true);
    window.size_range(MIN_WIDTH, MIN_HEIGHT, 0, 0);
    window.set_color(palette.surface);

    let mut root = Flex::default_fill().column();
    root.set_margin(ROOT_MARGIN);
    root.set_pad(ROOT_PAD);

    let mut input_row = Flex::default();
    input_row.set_type(input_flex_type(config.config.width));
    input_row.set_pad(PANE_GAP);

    let (mut left_editor, left_buffer, left_gutter, left_gutter_top_line) =
        make_editor_pane("Left input", palette);
    let (right_editor, right_buffer, right_gutter, right_gutter_top_line) =
        make_editor_pane("Right input", palette);
    input_row.end();

    let mut actions = Flex::default().row();
    actions.set_pad(6);
    let mut paste_left = make_button("Paste Left", false, palette);
    let mut paste_right = make_button("Paste Right", false, palette);
    let mut compare = make_button("Compare", true, palette);
    let mut swap = make_button("Swap", false, palette);
    let mut clear = make_button("Clear", false, palette);
    let mut copy_diff = make_button("Copy Diff", false, palette);
    paste_left.set_shortcut(Shortcut::Command | 'l');
    paste_right.set_shortcut(Shortcut::Command | 'r');
    compare.set_shortcut(Shortcut::Command | fltk::enums::Key::Enter);
    swap.set_shortcut(Shortcut::Command | Shortcut::Shift | 's');
    copy_diff.set_shortcut(Shortcut::Command | Shortcut::Shift | 'c');
    actions.end();

    let mut diff_container = Flex::default().column();
    let mut diff_toolbar = Flex::default().row();
    diff_toolbar.set_pad(6);
    let mut diff_mode = Frame::default().with_label("Unified Review");
    diff_mode.set_frame(FrameType::FlatBox);
    diff_mode.set_color(palette.header_bg);
    diff_mode.set_label_color(palette.text);
    diff_mode.set_label_size(13);
    let mut prev_change = make_button("Prev", false, palette);
    prev_change.deactivate();
    let mut next_change = make_button("Next", false, palette);
    next_change.deactivate();
    let mut pin = make_button(pin_button_label(false), false, palette);
    pin.set_shortcut(Shortcut::Command | Shortcut::Shift | 'p');
    pin.set_tooltip("Keep the Slippy window above other windows");
    let mut diff_summary = Frame::default().with_label("0 removed  0 added  0 edited");
    diff_summary.set_frame(FrameType::FlatBox);
    diff_summary.set_color(palette.header_bg);
    diff_summary.set_label_color(palette.muted);
    diff_summary.set_label_size(13);
    diff_summary.set_align(fltk::enums::Align::Right | fltk::enums::Align::Inside);
    diff_toolbar.fixed(&diff_mode, 120);
    diff_toolbar.fixed(&prev_change, 58);
    diff_toolbar.fixed(&next_change, 58);
    diff_toolbar.fixed(&pin, 66);
    diff_toolbar.end();

    let mut diff_body = Flex::default().row();
    let initial_diff_view = Rc::new(RefCell::new(crate::diff_view::build_diff_view(
        state.borrow().diff(),
        state.borrow().options(),
    )));
    let stale_diff_notice = Rc::new(Cell::new(false));
    let (diff_scroll, diff_canvas) = make_diff_canvas(
        palette,
        initial_diff_view.clone(),
        stale_diff_notice.clone(),
    );
    let mut overview_rail = Frame::default();
    overview_rail.set_frame(FrameType::FlatBox);
    overview_rail.set_color(palette.header_bg);
    overview_rail.set_label_font(Font::Courier);
    overview_rail.set_label_size(11);
    overview_rail.set_label_color(palette.muted);
    overview_rail.set_align(fltk::enums::Align::Top | fltk::enums::Align::Inside);
    diff_body.fixed(&overview_rail, OVERVIEW_RAIL_WIDTH);
    diff_body.end();

    diff_container.fixed(&diff_toolbar, DIFF_TOOLBAR_HEIGHT);
    diff_container.end();

    let mut status = Frame::default().with_label(state.borrow().status());
    status.set_frame(FrameType::FlatBox);
    status.set_color(palette.status_bg);
    status.set_label_color(palette.muted);
    status.set_label_size(13);
    status.set_align(fltk::enums::Align::Left | fltk::enums::Align::Inside);

    root.fixed(
        &input_row,
        input_height_for(config.config.height, config.config.vertical_split),
    );
    root.fixed(&actions, ACTION_BAR_HEIGHT);
    root.fixed(&status, STATUS_BAR_HEIGHT);
    root.end();

    {
        let mut responsive_inputs = input_row.clone();
        let mut responsive_diff_scroll = diff_scroll.clone();
        let mut responsive_diff_canvas = diff_canvas.clone();
        window.handle(move |win, event| {
            if event == Event::Resize {
                responsive_inputs.set_type(input_flex_type(win.w()));
                responsive_inputs.layout();
                resize_diff_canvas(&mut responsive_diff_scroll, &mut responsive_diff_canvas);
            }
            false
        });
    }

    window.end();
    window.show();

    left_editor.take_focus().ok();

    let handles = Rc::new(RefCell::new(UiHandles {
        left_editor,
        right_editor,
        left_buffer,
        right_buffer,
        left_gutter,
        right_gutter,
        left_gutter_top_line,
        right_gutter_top_line,
        diff_scroll,
        diff_canvas,
        diff_view: initial_diff_view,
        stale_diff_notice,
        diff_summary,
        overview_rail,
        status,
        copy_diff: copy_diff.clone(),
        pin: pin.clone(),
    }));
    render_state(&state, &handles);

    {
        let state = state.clone();
        let handles = handles.clone();
        let debounce_generation = debounce_generation.clone();
        let suppress_buffer_events = suppress_buffer_events.clone();
        paste_left.set_callback(move |_| {
            suppress_buffer_events.set(true);
            if paste_into_left(&state, &handles) {
                suppress_buffer_events.set(false);
                schedule_auto_compare(&state, &handles, sender, &debounce_generation);
            } else {
                suppress_buffer_events.set(false);
            }
        });
    }

    {
        let state = state.clone();
        let handles = handles.clone();
        let debounce_generation = debounce_generation.clone();
        let suppress_buffer_events = suppress_buffer_events.clone();
        let mut left_buffer = handles.borrow().left_buffer.clone();
        left_buffer.add_modify_callback2(move |_, _, _, _, _, _| {
            if !suppress_buffer_events.get() {
                sync_state_from_buffers(&state, &handles);
                render_state(&state, &handles);
                schedule_auto_compare(&state, &handles, sender, &debounce_generation);
            }
        });
    }

    {
        let state = state.clone();
        let handles = handles.clone();
        let debounce_generation = debounce_generation.clone();
        let suppress_buffer_events = suppress_buffer_events.clone();
        paste_right.set_callback(move |_| {
            suppress_buffer_events.set(true);
            if paste_into_right(&state, &handles) {
                suppress_buffer_events.set(false);
                schedule_auto_compare(&state, &handles, sender, &debounce_generation);
            } else {
                suppress_buffer_events.set(false);
            }
        });
    }

    {
        let state = state.clone();
        let handles = handles.clone();
        let debounce_generation = debounce_generation.clone();
        let suppress_buffer_events = suppress_buffer_events.clone();
        let mut right_buffer = handles.borrow().right_buffer.clone();
        right_buffer.add_modify_callback2(move |_, _, _, _, _, _| {
            if !suppress_buffer_events.get() {
                sync_state_from_buffers(&state, &handles);
                render_state(&state, &handles);
                schedule_auto_compare(&state, &handles, sender, &debounce_generation);
            }
        });
    }

    {
        let state = state.clone();
        let handles = handles.clone();
        let debounce_generation = debounce_generation.clone();
        let suppress_buffer_events = suppress_buffer_events.clone();
        swap.set_callback(move |_| {
            suppress_buffer_events.set(true);
            if swap_inputs(&state, &handles) {
                suppress_buffer_events.set(false);
                schedule_auto_compare(&state, &handles, sender, &debounce_generation);
            } else {
                suppress_buffer_events.set(false);
            }
        });
    }

    {
        let state = state.clone();
        let handles = handles.clone();
        copy_diff.set_callback(move |_| copy_current_diff(&state, &handles));
    }

    {
        let state = state.clone();
        let handles = handles.clone();
        let pinned = pinned.clone();
        let mut window = window.clone();
        pin.set_callback(move |_| {
            let next = !pinned.get();
            pinned.set(next);
            apply_pin_state(&state, &handles, &mut window, next);
        });
    }

    {
        let state = state.clone();
        let handles = handles.clone();
        let debounce_generation = debounce_generation.clone();
        let suppress_buffer_events = suppress_buffer_events.clone();
        clear.set_callback(move |_| {
            suppress_buffer_events.set(true);
            clear_all(&state, &handles);
            suppress_buffer_events.set(false);
            debounce_generation.set(debounce_generation.get().saturating_add(1));
        });
    }

    {
        let state = state.clone();
        let handles = handles.clone();
        let debounce_generation = debounce_generation.clone();
        compare.set_callback(move |_| {
            debounce_generation.set(debounce_generation.get().saturating_add(1));
            compare_now(&state, &handles, sender);
        });
    }

    attach_editor_gutter_refresh(&handles, true);
    attach_editor_gutter_refresh(&handles, false);

    while app.wait() {
        if let Some(message) = receiver.recv() {
            match message {
                UiMessage::DiffReady(result) => {
                    state.borrow_mut().apply_result(result);
                    render_state(&state, &handles);
                }
            }
        }
    }

    if let Some(path) = config_file {
        let mut next_config = config.config;
        next_config.width = window.width();
        next_config.height = window.height();
        if save_config_to_path(path, &next_config).is_err() {
            state
                .borrow_mut()
                .set_status("Could not save layout config.");
        }
    }

    Ok(())
}

fn make_editor_pane(
    label: &str,
    palette: Palette,
) -> (TextEditor, TextBuffer, Frame, Rc<Cell<i32>>) {
    let mut pane = Flex::default().row();
    pane.set_pad(0);

    let (gutter, buffer_for_gutter, top_line_for_gutter) = make_input_gutter(palette);
    let mut editor = TextEditor::default();
    let buffer = TextBuffer::default();
    editor.set_buffer(buffer.clone());
    editor.set_linenumber_width(0);
    editor.maintain_absolute_top_line_number(true);
    editor.set_text_font(Font::Courier);
    editor.set_text_size(14);
    editor.set_color(palette.pane);
    editor.set_text_color(palette.text);
    editor.set_frame(FrameType::FlatBox);
    editor.set_tooltip(label);

    pane.fixed(&gutter, INPUT_GUTTER_WIDTH);
    pane.end();

    *buffer_for_gutter.borrow_mut() = buffer.clone();
    (editor, buffer, gutter, top_line_for_gutter)
}

fn make_input_gutter(palette: Palette) -> (Frame, Rc<RefCell<TextBuffer>>, Rc<Cell<i32>>) {
    let mut gutter = Frame::default();
    gutter.set_frame(FrameType::FlatBox);
    gutter.set_color(palette.header_bg);
    let buffer = Rc::new(RefCell::new(TextBuffer::default()));
    let top_line = Rc::new(Cell::new(1));
    gutter.draw({
        let buffer = buffer.clone();
        let top_line = top_line.clone();
        move |frame| draw_input_gutter(frame, &buffer.borrow(), top_line.get(), palette)
    });
    (gutter, buffer, top_line)
}

fn make_diff_canvas(
    palette: Palette,
    view: Rc<RefCell<crate::diff_view::RenderedDiffView>>,
    stale_notice: Rc<Cell<bool>>,
) -> (Scroll, Frame) {
    let mut scroll = Scroll::default();
    scroll.set_type(ScrollType::Both);
    scroll.set_frame(FrameType::FlatBox);
    scroll.set_color(palette.pane);
    scroll.set_scrollbar_size(14);

    let mut canvas = Frame::default().with_size(
        DIFF_CANVAS_MIN_WIDTH,
        diff_canvas_height(view.borrow().rows.len()),
    );
    canvas.set_frame(FrameType::FlatBox);
    canvas.set_color(palette.pane);
    canvas.draw({
        let view = view.clone();
        let stale_notice = stale_notice.clone();
        move |frame| draw_diff_canvas(frame, &view.borrow(), stale_notice.get(), palette)
    });
    scroll.end();
    (scroll, canvas)
}

fn make_button(label: &str, primary: bool, palette: Palette) -> Button {
    let mut button = Button::default().with_label(label);
    button.set_frame(FrameType::RFlatBox);
    button.set_label_size(13);
    if primary {
        button.set_color(palette.primary);
        button.set_label_color(palette.primary_text);
    } else {
        button.set_color(palette.secondary_button);
        button.set_label_color(palette.text);
        button.set_selection_color(palette.border);
    }
    button
}

fn compare_now(
    state: &Rc<RefCell<AppState>>,
    handles: &Rc<RefCell<UiHandles>>,
    sender: app::Sender<UiMessage>,
) {
    sync_state_from_buffers(state, handles);
    let request = state.borrow_mut().create_manual_request();
    render_state(state, handles);
    spawn_diff_worker(request, sender);
}

fn paste_into_left(state: &Rc<RefCell<AppState>>, handles: &Rc<RefCell<UiHandles>>) -> bool {
    match Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
        Ok(text) => {
            let mut left_buffer = handles.borrow().left_buffer.clone();
            left_buffer.set_text(&text);
            let mut left_editor = handles.borrow().left_editor.clone();
            left_editor.take_focus().ok();
            state.borrow_mut().set_left(text);
            render_state(state, handles);
            true
        }
        Err(_) => {
            state
                .borrow_mut()
                .set_status("Paste failed: clipboard text unavailable.");
            render_state(state, handles);
            false
        }
    }
}

fn paste_into_right(state: &Rc<RefCell<AppState>>, handles: &Rc<RefCell<UiHandles>>) -> bool {
    match Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
        Ok(text) => {
            let mut right_buffer = handles.borrow().right_buffer.clone();
            right_buffer.set_text(&text);
            let mut right_editor = handles.borrow().right_editor.clone();
            right_editor.take_focus().ok();
            state.borrow_mut().set_right(text);
            render_state(state, handles);
            true
        }
        Err(_) => {
            state
                .borrow_mut()
                .set_status("Paste failed: clipboard text unavailable.");
            render_state(state, handles);
            false
        }
    }
}

fn swap_inputs(state: &Rc<RefCell<AppState>>, handles: &Rc<RefCell<UiHandles>>) -> bool {
    sync_state_from_buffers(state, handles);
    let should_schedule = state.borrow_mut().swap();
    {
        let handles = handles.borrow();
        let state = state.borrow();
        let mut left_buffer = handles.left_buffer.clone();
        let mut right_buffer = handles.right_buffer.clone();
        left_buffer.set_text(state.left());
        right_buffer.set_text(state.right());
    }
    render_state(state, handles);
    should_schedule
}

fn schedule_auto_compare(
    state: &Rc<RefCell<AppState>>,
    handles: &Rc<RefCell<UiHandles>>,
    sender: app::Sender<UiMessage>,
    debounce_generation: &Rc<Cell<u64>>,
) {
    let (should, debounce_ms) = {
        let s = state.borrow();
        (s.should_auto_diff(), s.options().debounce_ms)
    };
    if !should {
        render_state(state, handles);
        return;
    }

    let generation = debounce_generation.get().saturating_add(1);
    debounce_generation.set(generation);
    let state = state.clone();
    let handles = handles.clone();
    let debounce_generation = debounce_generation.clone();
    app::add_timeout3(debounce_ms as f64 / 1000.0, move |_| {
        if debounce_generation.get() == generation {
            sync_state_from_buffers(&state, &handles);
            let Some(request) = state.borrow_mut().create_auto_request() else {
                render_state(&state, &handles);
                return;
            };
            render_state(&state, &handles);
            spawn_diff_worker(request, sender);
        }
    });
}

fn spawn_diff_worker(request: crate::app_state::DiffRequest, sender: app::Sender<UiMessage>) {
    thread::spawn(move || {
        sender.send(UiMessage::DiffReady(request.compute()));
        app::awake();
    });
}

fn clear_all(state: &Rc<RefCell<AppState>>, handles: &Rc<RefCell<UiHandles>>) {
    state.borrow_mut().clear();
    {
        let handles = handles.borrow();
        let mut left_buffer = handles.left_buffer.clone();
        let mut right_buffer = handles.right_buffer.clone();
        left_buffer.set_text("");
        right_buffer.set_text("");
    }
    state.borrow_mut().set_status(STATUS_CLEARED);
    render_state(state, handles);
}

fn copy_current_diff(state: &Rc<RefCell<AppState>>, handles: &Rc<RefCell<UiHandles>>) {
    let state_snapshot = state.borrow();
    if !state_snapshot.has_current_diff() {
        drop(state_snapshot);
        state
            .borrow_mut()
            .set_status("Copy Diff failed: no current diff.");
        render_state(state, handles);
        return;
    }
    let diff = render_unified_diff(state_snapshot.diff());
    drop(state_snapshot);

    match Clipboard::new().and_then(|mut clipboard| clipboard.set_text(diff)) {
        Ok(()) => state.borrow_mut().set_status("Diff copied."),
        Err(_) => state
            .borrow_mut()
            .set_status("Copy Diff failed: clipboard unavailable."),
    }
    render_state(state, handles);
}

fn apply_pin_state(
    state: &Rc<RefCell<AppState>>,
    handles: &Rc<RefCell<UiHandles>>,
    window: &mut Window,
    pinned: bool,
) {
    {
        let handles = handles.borrow();
        let mut pin = handles.pin.clone();
        pin.set_label(pin_button_label(pinned));
    }

    if pinned {
        window.set_on_top();
        state.borrow_mut().set_status("Pinned above other windows.");
    } else {
        state
            .borrow_mut()
            .set_status("Pin cleared. Some window managers keep native topmost until refocus.");
    }
    render_state(state, handles);
}

fn sync_state_from_buffers(state: &Rc<RefCell<AppState>>, handles: &Rc<RefCell<UiHandles>>) {
    let handles = handles.borrow();
    let left = handles.left_buffer.text();
    let right = handles.right_buffer.text();
    let mut state = state.borrow_mut();
    state.set_left(left);
    state.set_right(right);
}

fn render_state(state: &Rc<RefCell<AppState>>, handles: &Rc<RefCell<UiHandles>>) {
    let state = state.borrow();
    let handles = handles.borrow();
    let mut diff_scroll = handles.diff_scroll.clone();
    let mut diff_canvas = handles.diff_canvas.clone();
    let mut diff_summary = handles.diff_summary.clone();
    let mut overview_rail = handles.overview_rail.clone();
    let mut status = handles.status.clone();
    let mut copy_diff = handles.copy_diff.clone();

    let view = crate::diff_view::build_diff_view(state.diff(), state.options());
    *handles.diff_view.borrow_mut() = view.clone();
    handles.stale_diff_notice.set(state.has_stale_diff());
    resize_diff_canvas_for_view(&mut diff_scroll, &mut diff_canvas, &view);
    diff_canvas.redraw();
    diff_summary.set_label(&diff_summary_label(&view.summary));
    overview_rail.set_label(&overview_rail_label(&view));
    overview_rail.set_label_color(handles.status.label_color());
    overview_rail.redraw();
    redraw_input_gutters(&handles);
    status.set_label(state.status());
    if state.has_current_diff() {
        copy_diff.activate();
    } else {
        copy_diff.deactivate();
    }
}

fn diff_summary_label(summary: &crate::diff_view::ChangeSummary) -> String {
    format!(
        "{} removed  {} added  {} edited",
        summary.removed, summary.added, summary.edited
    )
}

fn text_line_count(text: &str) -> usize {
    text.lines().count().max(1) + usize::from(text.ends_with('\n'))
}

fn visible_input_line_numbers(top_line: i32, visible_rows: i32, text: &str) -> Vec<usize> {
    let first = top_line.max(1) as usize;
    let total = text_line_count(text);
    let visible = visible_rows.max(1) as usize;
    (first..=total).take(visible).collect()
}

fn pin_button_label(pinned: bool) -> &'static str {
    if pinned { "Pinned" } else { "Pin" }
}

fn diff_canvas_height(row_count: usize) -> i32 {
    DIFF_HEADER_HEIGHT + DIFF_ROW_HEIGHT * row_count.max(1) as i32
}

fn attach_editor_gutter_refresh(handles: &Rc<RefCell<UiHandles>>, left_side: bool) {
    let (mut editor, mut gutter, top_line) = {
        let handles = handles.borrow();
        if left_side {
            (
                handles.left_editor.clone(),
                handles.left_gutter.clone(),
                handles.left_gutter_top_line.clone(),
            )
        } else {
            (
                handles.right_editor.clone(),
                handles.right_gutter.clone(),
                handles.right_gutter_top_line.clone(),
            )
        }
    };

    editor.handle(move |editor, event| {
        let handled = false;
        match event {
            Event::Push
            | Event::Drag
            | Event::Released
            | Event::MouseWheel
            | Event::KeyDown
            | Event::KeyUp
            | Event::Resize
            | Event::Move => {
                top_line.set(editor.get_absolute_top_line_number().max(1));
                gutter.redraw();
            }
            _ => {}
        }
        handled
    });
}

fn redraw_input_gutters(handles: &UiHandles) {
    handles
        .left_gutter_top_line
        .set(handles.left_editor.get_absolute_top_line_number().max(1));
    handles
        .right_gutter_top_line
        .set(handles.right_editor.get_absolute_top_line_number().max(1));
    let mut left_gutter = handles.left_gutter.clone();
    let mut right_gutter = handles.right_gutter.clone();
    left_gutter.redraw();
    right_gutter.redraw();
}

fn resize_diff_canvas_for_view(
    scroll: &mut Scroll,
    canvas: &mut Frame,
    view: &crate::diff_view::RenderedDiffView,
) {
    let width = (scroll.w() - scroll.scrollbar_size()).max(DIFF_CANVAS_MIN_WIDTH);
    let height = diff_canvas_height(view.rows.len());
    canvas.resize(scroll.x(), scroll.y(), width, height);
    scroll.redraw();
}

fn resize_diff_canvas(scroll: &mut Scroll, canvas: &mut Frame) {
    let width = (scroll.w() - scroll.scrollbar_size()).max(DIFF_CANVAS_MIN_WIDTH);
    canvas.resize(canvas.x(), canvas.y(), width, canvas.h());
    scroll.redraw();
}

fn draw_input_gutter(frame: &Frame, buffer: &TextBuffer, top_line: i32, palette: Palette) {
    draw::set_draw_color(palette.header_bg);
    draw::draw_rectf(frame.x(), frame.y(), frame.w(), frame.h());

    draw::set_draw_color(palette.border);
    draw::draw_line(
        frame.x() + frame.w() - 1,
        frame.y(),
        frame.x() + frame.w() - 1,
        frame.y() + frame.h(),
    );

    draw::set_font(Font::Courier, 13);
    draw::set_draw_color(palette.muted);
    let visible_rows = ((frame.h() - 8).max(INPUT_LINE_HEIGHT) / INPUT_LINE_HEIGHT) + 1;
    let numbers = visible_input_line_numbers(top_line, visible_rows, &buffer.text());

    for (idx, line_no) in numbers.into_iter().enumerate() {
        let y = frame.y() + 4 + idx as i32 * INPUT_LINE_HEIGHT;
        draw::draw_text2(
            &line_no.to_string(),
            frame.x(),
            y,
            frame.w() - 8,
            INPUT_LINE_HEIGHT,
            Align::Right | Align::Inside,
        );
    }
}

fn draw_diff_canvas(
    frame: &Frame,
    view: &crate::diff_view::RenderedDiffView,
    stale_notice: bool,
    palette: Palette,
) {
    draw::set_draw_color(palette.pane);
    draw::draw_rectf(frame.x(), frame.y(), frame.w(), frame.h());

    draw_diff_header(frame, stale_notice, palette);

    let mut y = frame.y() + DIFF_HEADER_HEIGHT;
    if view.rows.is_empty() {
        draw_empty_diff_row(frame, y, palette);
        return;
    }

    for row in &view.rows {
        draw_diff_row(frame, y, row, palette);
        y += DIFF_ROW_HEIGHT;
    }
}

fn draw_diff_header(frame: &Frame, stale_notice: bool, palette: Palette) {
    draw::set_draw_color(palette.header_bg);
    draw::draw_rectf(frame.x(), frame.y(), frame.w(), DIFF_HEADER_HEIGHT);
    draw::set_draw_color(palette.border);
    draw::draw_line(
        frame.x(),
        frame.y() + DIFF_HEADER_HEIGHT - 1,
        frame.x() + frame.w(),
        frame.y() + DIFF_HEADER_HEIGHT - 1,
    );

    draw::set_font(Font::Courier, 13);
    draw::set_draw_color(if stale_notice {
        palette.delete_text
    } else {
        palette.muted
    });

    if stale_notice {
        draw::draw_text2(
            "Previous diff is stale. Press Compare to update.",
            frame.x() + 10,
            frame.y(),
            frame.w() - 20,
            DIFF_HEADER_HEIGHT,
            Align::Left | Align::Inside,
        );
        return;
    }

    let old_x = frame.x();
    let new_x = old_x + DIFF_OLD_GUTTER_WIDTH;
    let marker_x = new_x + DIFF_NEW_GUTTER_WIDTH;
    let text_x = marker_x + DIFF_MARKER_WIDTH;
    draw::draw_text2(
        "OLD",
        old_x,
        frame.y(),
        DIFF_OLD_GUTTER_WIDTH - 8,
        DIFF_HEADER_HEIGHT,
        Align::Right | Align::Inside,
    );
    draw::draw_text2(
        "NEW",
        new_x,
        frame.y(),
        DIFF_NEW_GUTTER_WIDTH - 8,
        DIFF_HEADER_HEIGHT,
        Align::Right | Align::Inside,
    );
    draw::draw_text2(
        "K",
        marker_x,
        frame.y(),
        DIFF_MARKER_WIDTH,
        DIFF_HEADER_HEIGHT,
        Align::Center | Align::Inside,
    );
    draw::draw_text2(
        "Text",
        text_x + DIFF_TEXT_LEFT_PAD,
        frame.y(),
        frame.w() - text_x,
        DIFF_HEADER_HEIGHT,
        Align::Left | Align::Inside,
    );
}

fn draw_empty_diff_row(frame: &Frame, y: i32, palette: Palette) {
    draw::set_font(Font::Courier, 14);
    draw::set_draw_color(palette.muted);
    draw::draw_text2(
        "No differences",
        frame.x() + DIFF_OLD_GUTTER_WIDTH + DIFF_NEW_GUTTER_WIDTH + DIFF_MARKER_WIDTH + 10,
        y,
        frame.w(),
        DIFF_ROW_HEIGHT,
        Align::Left | Align::Inside,
    );
}

fn draw_diff_row(frame: &Frame, y: i32, row: &crate::diff_view::DiffViewRow, palette: Palette) {
    let row_bg = diff_row_bg(row.kind, palette);
    draw::set_draw_color(row_bg);
    draw::draw_rectf(frame.x(), y, frame.w(), DIFF_ROW_HEIGHT);

    draw::set_draw_color(palette.header_bg);
    draw::draw_rectf(
        frame.x(),
        y,
        DIFF_OLD_GUTTER_WIDTH + DIFF_NEW_GUTTER_WIDTH + DIFF_MARKER_WIDTH,
        DIFF_ROW_HEIGHT,
    );

    draw::set_draw_color(palette.border);
    let old_right = frame.x() + DIFF_OLD_GUTTER_WIDTH;
    let new_right = old_right + DIFF_NEW_GUTTER_WIDTH;
    let marker_right = new_right + DIFF_MARKER_WIDTH;
    draw::draw_line(old_right, y, old_right, y + DIFF_ROW_HEIGHT);
    draw::draw_line(new_right, y, new_right, y + DIFF_ROW_HEIGHT);
    draw::draw_line(marker_right, y, marker_right, y + DIFF_ROW_HEIGHT);
    draw::draw_line(
        frame.x(),
        y + DIFF_ROW_HEIGHT - 1,
        frame.x() + frame.w(),
        y + DIFF_ROW_HEIGHT - 1,
    );

    draw::set_font(Font::Courier, 13);
    draw::set_draw_color(palette.muted);
    draw_line_number(row.old_line, frame.x(), y, DIFF_OLD_GUTTER_WIDTH - 8);
    draw_line_number(row.new_line, old_right, y, DIFF_NEW_GUTTER_WIDTH - 8);

    let marker_color = match row.kind {
        crate::diff_view::DiffViewRowKind::Delete
        | crate::diff_view::DiffViewRowKind::ReplaceOld => palette.delete_text,
        crate::diff_view::DiffViewRowKind::Insert
        | crate::diff_view::DiffViewRowKind::ReplaceNew => palette.insert_text,
        _ => palette.muted,
    };
    draw::set_draw_color(marker_color);
    draw::draw_text2(
        row.marker,
        new_right,
        y,
        DIFF_MARKER_WIDTH,
        DIFF_ROW_HEIGHT,
        Align::Center | Align::Inside,
    );

    draw::set_font(Font::Courier, 14);
    let mut x = marker_right + DIFF_TEXT_LEFT_PAD;
    for segment in &row.segments {
        let color = diff_segment_text_color(segment.kind, row.kind, palette);
        let bg = diff_segment_bg(segment.kind, row.kind, palette);
        x = draw_diff_segment(&segment.text, x, y, color, bg);
    }
}

fn draw_line_number(line: Option<usize>, x: i32, y: i32, width: i32) {
    if let Some(line) = line {
        draw::draw_text2(
            &line.to_string(),
            x,
            y,
            width,
            DIFF_ROW_HEIGHT,
            Align::Right | Align::Inside,
        );
    }
}

fn draw_diff_segment(text: &str, x: i32, y: i32, color: Color, bg: Color) -> i32 {
    let (width, _) = draw::measure(text, false);
    if width > 0 {
        draw::set_draw_color(bg);
        draw::draw_rectf(x - 1, y + 3, width + 2, DIFF_ROW_HEIGHT - 6);
    }
    draw::set_draw_color(color);
    draw::draw_text2(
        text,
        x,
        y,
        width.max(1),
        DIFF_ROW_HEIGHT,
        Align::Left | Align::Inside,
    );
    x + width
}

fn diff_row_bg(kind: crate::diff_view::DiffViewRowKind, palette: Palette) -> Color {
    match kind {
        crate::diff_view::DiffViewRowKind::Delete => palette.delete_bg,
        crate::diff_view::DiffViewRowKind::Insert => palette.insert_bg,
        crate::diff_view::DiffViewRowKind::ReplaceOld
        | crate::diff_view::DiffViewRowKind::ReplaceNew => palette.replace_bg,
        crate::diff_view::DiffViewRowKind::Fold
        | crate::diff_view::DiffViewRowKind::Notice
        | crate::diff_view::DiffViewRowKind::Context => palette.pane,
    }
}

fn diff_segment_text_color(
    segment: crate::diff_view::DiffViewSegmentKind,
    row: crate::diff_view::DiffViewRowKind,
    palette: Palette,
) -> Color {
    match segment {
        crate::diff_view::DiffViewSegmentKind::DeleteToken => palette.delete_text,
        crate::diff_view::DiffViewSegmentKind::InsertToken => palette.insert_text,
        crate::diff_view::DiffViewSegmentKind::Normal => match row {
            crate::diff_view::DiffViewRowKind::Delete => palette.delete_text,
            crate::diff_view::DiffViewRowKind::Insert => palette.insert_text,
            crate::diff_view::DiffViewRowKind::ReplaceOld
            | crate::diff_view::DiffViewRowKind::ReplaceNew => palette.replace_text,
            crate::diff_view::DiffViewRowKind::Fold | crate::diff_view::DiffViewRowKind::Notice => {
                palette.muted
            }
            crate::diff_view::DiffViewRowKind::Context => palette.text,
        },
    }
}

fn diff_segment_bg(
    segment: crate::diff_view::DiffViewSegmentKind,
    row: crate::diff_view::DiffViewRowKind,
    palette: Palette,
) -> Color {
    match segment {
        crate::diff_view::DiffViewSegmentKind::DeleteToken => palette.inline_delete_bg,
        crate::diff_view::DiffViewSegmentKind::InsertToken => palette.inline_insert_bg,
        crate::diff_view::DiffViewSegmentKind::Normal => diff_row_bg(row, palette),
    }
}

fn overview_rail_label(view: &crate::diff_view::RenderedDiffView) -> String {
    use crate::diff_view::ChangeMarkKind;

    if view.rows.is_empty() {
        return String::new();
    }

    let height = 12usize;
    let mut slots = vec![' '; height];
    let last_row = view.rows.len().saturating_sub(1).max(1);
    for mark in &view.marks {
        let slot = (mark.row_index * (height - 1)) / last_row;
        slots[slot] = match mark.kind {
            ChangeMarkKind::Delete => '-',
            ChangeMarkKind::Insert => '+',
            ChangeMarkKind::Replace => '~',
        };
    }

    slots
        .into_iter()
        .map(|slot| slot.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn input_flex_type(width: i32) -> FlexType {
    if width < STACK_INPUT_WIDTH {
        FlexType::Column
    } else {
        FlexType::Row
    }
}

fn input_height_for(window_height: i32, vertical_split: f32) -> i32 {
    let available =
        window_height - (ROOT_MARGIN * 2) - (ROOT_PAD * 3) - ACTION_BAR_HEIGHT - STATUS_BAR_HEIGHT;
    ((available.max(1) as f32) * vertical_split).round() as i32
}

fn palette_for(theme: Theme) -> Palette {
    match theme {
        Theme::System | Theme::Light => Palette {
            surface: Color::from_rgb(247, 245, 240),
            pane: Color::from_rgb(255, 254, 250),
            text: Color::from_rgb(37, 35, 31),
            muted: Color::from_rgb(110, 103, 94),
            border: Color::from_rgb(216, 210, 199),
            primary: Color::from_rgb(47, 111, 115),
            primary_text: Color::White,
            insert_text: Color::from_rgb(31, 107, 58),
            delete_text: Color::from_rgb(154, 58, 37),
            replace_text: Color::from_rgb(68, 62, 48),
            status_bg: Color::from_rgb(239, 236, 228),
            secondary_button: Color::from_rgb(240, 238, 232),
            insert_bg: Color::from_rgb(232, 244, 234), // #E8F4EA
            delete_bg: Color::from_rgb(248, 231, 225), // #F8E7E1
            replace_bg: Color::from_rgb(245, 241, 224),
            inline_insert_bg: Color::from_rgb(196, 231, 207),
            inline_delete_bg: Color::from_rgb(239, 199, 187),
            header_bg: Color::from_rgb(240, 238, 232), // #F0EEE8
        },
        Theme::Dark => Palette {
            surface: Color::from_rgb(31, 33, 30),
            pane: Color::from_rgb(37, 40, 34),
            text: Color::from_rgb(236, 232, 221),
            muted: Color::from_rgb(166, 159, 145),
            border: Color::from_rgb(58, 62, 53),
            primary: Color::from_rgb(111, 168, 173),
            primary_text: Color::from_rgb(16, 32, 34),
            insert_text: Color::from_rgb(168, 216, 178),
            delete_text: Color::from_rgb(240, 160, 138),
            replace_text: Color::from_rgb(230, 221, 197),
            status_bg: Color::from_rgb(37, 40, 32),
            secondary_button: Color::from_rgb(46, 49, 42),
            insert_bg: Color::from_rgb(31, 58, 41), // #1F3A29
            delete_bg: Color::from_rgb(68, 37, 31), // #44251F
            replace_bg: Color::from_rgb(55, 52, 38),
            inline_insert_bg: Color::from_rgb(49, 87, 59),
            inline_delete_bg: Color::from_rgb(91, 48, 38),
            header_bg: Color::from_rgb(46, 49, 42), // #2E312A
        },
    }
}

fn rgb(color: Color) -> (u8, u8, u8) {
    let value = color.bits();
    (
        ((value >> 24) & 0xff) as u8,
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_summary_label_formats_counts() {
        let summary = crate::diff_view::ChangeSummary {
            removed: 2,
            added: 3,
            edited: 1,
        };

        assert_eq!(diff_summary_label(&summary), "2 removed  3 added  1 edited");
    }

    #[test]
    fn text_line_count_keeps_single_empty_line_and_trailing_newline() {
        assert_eq!(text_line_count(""), 1);
        assert_eq!(text_line_count("one"), 1);
        assert_eq!(text_line_count("one\n"), 2);
        assert_eq!(text_line_count("one\ntwo"), 2);
    }

    #[test]
    fn visible_input_line_numbers_start_at_absolute_top_line() {
        assert_eq!(
            visible_input_line_numbers(3, 4, "a\nb\nc\nd\ne\n"),
            vec![3, 4, 5, 6]
        );
    }

    #[test]
    fn visible_input_line_numbers_never_exceed_buffer_line_count() {
        assert_eq!(visible_input_line_numbers(2, 8, "a\nb\n"), vec![2, 3]);
    }

    #[test]
    fn pin_button_label_reflects_app_level_state() {
        assert_eq!(pin_button_label(false), "Pin");
        assert_eq!(pin_button_label(true), "Pinned");
    }

    #[test]
    fn diff_canvas_height_includes_header_and_all_rows() {
        assert_eq!(diff_canvas_height(0), DIFF_HEADER_HEIGHT + DIFF_ROW_HEIGHT);
        assert_eq!(
            diff_canvas_height(3),
            DIFF_HEADER_HEIGHT + (DIFF_ROW_HEIGHT * 3)
        );
    }

    #[test]
    fn overview_rail_label_places_change_markers() {
        use crate::{
            diff_core::{DiffOptions, build_display_diff},
            diff_view::build_diff_view,
        };

        let diff = build_display_diff(
            "i wanna eatt banana\ni wanna eatt banana",
            "i wanna eat bananas\ni wanna eatt banana\ni，",
            &DiffOptions::default(),
        );
        let view = build_diff_view(&diff, &DiffOptions::default());
        let label = overview_rail_label(&view);

        assert!(label.contains('~'));
        assert!(label.contains('+'));
    }

    #[test]
    fn overview_rail_label_keeps_blank_slots_without_marks() {
        use crate::{
            diff_core::{DiffOptions, build_display_diff},
            diff_view::build_diff_view,
        };

        let diff = build_display_diff("same\n", "same\n", &DiffOptions::default());
        let view = build_diff_view(&diff, &DiffOptions::default());
        let label = overview_rail_label(&view);

        assert_eq!(label, vec![" "; 12].join("\n"));
        assert!(!label.contains('-'));
        assert!(!label.contains('+'));
        assert!(!label.contains('~'));
    }
}
