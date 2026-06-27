use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    thread,
};

use arboard::Clipboard;
use fltk::{
    app,
    button::Button,
    enums::{Color, Event, Font, FrameType, Shortcut},
    frame::Frame,
    group::{Flex, FlexType},
    prelude::*,
    text::{StyleTableEntryExt, TextAttr, TextBuffer, TextDisplay, TextEditor},
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
    status_bg: Color,
    secondary_button: Color,
    insert_bg: Color,
    delete_bg: Color,
    header_bg: Color,
}

const ACTION_BAR_HEIGHT: i32 = 34;
const DIFF_TOOLBAR_HEIGHT: i32 = 32;
const OVERVIEW_RAIL_WIDTH: i32 = 14;
const STATUS_BAR_HEIGHT: i32 = 26;
const ROOT_MARGIN: i32 = 8;
const ROOT_PAD: i32 = 8;
const PANE_GAP: i32 = 8;
const LINE_NUMBER_WIDTH: i32 = 44;
const STACK_INPUT_WIDTH: i32 = 760;

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
    diff_buffer: TextBuffer,
    diff_style_buffer: TextBuffer,
    diff_summary: Frame,
    overview_rail: Frame,
    status: Frame,
    copy_diff: Button,
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

    let (mut left_editor, left_buffer) = make_editor("Left input", palette);
    let (right_editor, right_buffer) = make_editor("Right input", palette);
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
    let mut next_change = make_button("Next", false, palette);
    let mut diff_summary = Frame::default().with_label("0 removed  0 added  0 edited");
    diff_summary.set_frame(FrameType::FlatBox);
    diff_summary.set_color(palette.header_bg);
    diff_summary.set_label_color(palette.muted);
    diff_summary.set_label_size(13);
    diff_summary.set_align(fltk::enums::Align::Right | fltk::enums::Align::Inside);
    diff_toolbar.fixed(&diff_mode, 120);
    diff_toolbar.fixed(&prev_change, 58);
    diff_toolbar.fixed(&next_change, 58);
    diff_toolbar.end();

    let mut diff_body = Flex::default().row();
    let (mut diff_display, diff_buffer, diff_style_buffer) = make_diff_display(palette);
    let mut overview_rail = Frame::default();
    overview_rail.set_frame(FrameType::FlatBox);
    overview_rail.set_color(palette.header_bg);
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
        window.handle(move |win, event| {
            if event == Event::Resize {
                responsive_inputs.set_type(input_flex_type(win.w()));
                responsive_inputs.layout();
            }
            false
        });
    }

    window.end();
    window.show();

    diff_display.set_text_color(palette.text);
    left_editor.take_focus().ok();

    let handles = Rc::new(RefCell::new(UiHandles {
        left_editor,
        right_editor,
        left_buffer,
        right_buffer,
        diff_buffer,
        diff_style_buffer,
        diff_summary,
        overview_rail,
        status,
        copy_diff: copy_diff.clone(),
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

fn make_editor(label: &str, palette: Palette) -> (TextEditor, TextBuffer) {
    let mut editor = TextEditor::default();
    let buffer = TextBuffer::default();
    editor.set_buffer(buffer.clone());
    configure_line_numbers(&mut editor, palette);
    editor.set_text_font(Font::Courier);
    editor.set_text_size(14);
    editor.set_color(palette.pane);
    editor.set_text_color(palette.text);
    editor.set_frame(FrameType::BorderBox);
    editor.set_tooltip(label);
    (editor, buffer)
}

fn make_diff_display(palette: Palette) -> (TextDisplay, TextBuffer, TextBuffer) {
    let mut display = TextDisplay::default();
    let buffer = TextBuffer::default();
    let style_buffer = TextBuffer::default();
    display.set_buffer(buffer.clone());
    display.set_linenumber_width(0);
    display.set_text_font(Font::Courier);
    display.set_text_size(14);
    display.set_color(palette.pane);
    display.set_frame(FrameType::BorderBox);
    display.set_highlight_data_ext(style_buffer.clone(), style_table_ext(palette));
    (display, buffer, style_buffer)
}

fn configure_line_numbers<T: fltk::prelude::DisplayExt>(display: &mut T, palette: Palette) {
    display.set_linenumber_width(LINE_NUMBER_WIDTH);
    display.set_linenumber_font(Font::Courier);
    display.set_linenumber_size(13);
    display.set_linenumber_fgcolor(palette.muted);
    display.set_linenumber_bgcolor(palette.pane);
    display.set_linenumber_align(fltk::enums::Align::Right);
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
        let mut diff_buffer = handles.diff_buffer.clone();
        let mut diff_style_buffer = handles.diff_style_buffer.clone();
        left_buffer.set_text("");
        right_buffer.set_text("");
        diff_buffer.set_text("");
        diff_style_buffer.set_text("");
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
    let mut diff_buffer = handles.diff_buffer.clone();
    let mut diff_style_buffer = handles.diff_style_buffer.clone();
    let mut diff_summary = handles.diff_summary.clone();
    let mut overview_rail = handles.overview_rail.clone();
    let mut status = handles.status.clone();
    let mut copy_diff = handles.copy_diff.clone();

    let view = crate::diff_view::build_diff_view(state.diff(), state.options());
    let rendered = if state.has_stale_diff() {
        with_leading_notice(
            render_diff_view_text(&view),
            "Previous diff is stale. Press Compare to update.\n\n",
            'G',
        )
    } else {
        render_diff_view_text(&view)
    };
    diff_buffer.set_text(&rendered.text);
    diff_style_buffer.set_text(&rendered.styles);
    diff_summary.set_label(&diff_summary_label(&view.summary));
    overview_rail.redraw();
    status.set_label(state.status());
    if state.has_current_diff() {
        copy_diff.activate();
    } else {
        copy_diff.deactivate();
    }
}

fn style_table_ext(palette: Palette) -> Vec<StyleTableEntryExt> {
    vec![
        // 'A' normal / context
        StyleTableEntryExt {
            color: palette.text,
            font: Font::Courier,
            size: 14,
            attr: TextAttr::None,
            bgcolor: palette.pane,
        },
        // 'B' generic header
        StyleTableEntryExt {
            color: palette.muted,
            font: Font::Courier,
            size: 14,
            attr: TextAttr::None,
            bgcolor: palette.header_bg,
        },
        // 'C' insert line
        StyleTableEntryExt {
            color: palette.insert_text,
            font: Font::Courier,
            size: 14,
            attr: TextAttr::None,
            bgcolor: palette.insert_bg,
        },
        // 'D' delete line
        StyleTableEntryExt {
            color: palette.delete_text,
            font: Font::Courier,
            size: 14,
            attr: TextAttr::None,
            bgcolor: palette.delete_bg,
        },
        // 'E' inline delete fragment
        StyleTableEntryExt {
            color: palette.delete_text,
            font: Font::CourierBold,
            size: 14,
            attr: TextAttr::None,
            bgcolor: palette.delete_bg,
        },
        // 'F' inline insert fragment
        StyleTableEntryExt {
            color: palette.insert_text,
            font: Font::CourierBold,
            size: 14,
            attr: TextAttr::None,
            bgcolor: palette.insert_bg,
        },
        // 'G' fold / notice / stale notice
        StyleTableEntryExt {
            color: palette.muted,
            font: Font::Courier,
            size: 14,
            attr: TextAttr::None,
            bgcolor: palette.pane,
        },
        // 'H' neutral replacement line
        StyleTableEntryExt {
            color: palette.text,
            font: Font::Courier,
            size: 14,
            attr: TextAttr::None,
            bgcolor: palette.header_bg,
        },
        // 'I' semantic gutter / marker
        StyleTableEntryExt {
            color: palette.muted,
            font: Font::Courier,
            size: 14,
            attr: TextAttr::None,
            bgcolor: palette.header_bg,
        },
    ]
}

struct RenderedDiff {
    text: String,
    styles: String,
}

/// Prepend a `notice` into both the text and style buffers of a `RenderedDiff`,
/// applying the same `style` char to every byte of the notice. This preserves the
/// FLTK ext-highlight invariant `text.len() == styles.len()` (style chars are
/// resolved by byte index) when a leading banner is added — e.g. the stale-diff
/// notice in `render_state`.
fn with_leading_notice(mut rendered: RenderedDiff, notice: &str, style: char) -> RenderedDiff {
    let mut text = String::with_capacity(notice.len() + rendered.text.len());
    let mut styles = String::with_capacity(notice.len() + rendered.styles.len());
    push_styled(&mut text, &mut styles, notice, style);
    text.push_str(&rendered.text);
    styles.push_str(&rendered.styles);
    rendered.text = text;
    rendered.styles = styles;
    rendered
}

fn diff_summary_label(summary: &crate::diff_view::ChangeSummary) -> String {
    format!(
        "{} removed  {} added  {} edited",
        summary.removed, summary.added, summary.edited
    )
}

fn render_diff_view_text(view: &crate::diff_view::RenderedDiffView) -> RenderedDiff {
    let mut text = String::new();
    let mut styles = String::new();

    push_styled(&mut text, &mut styles, "OLD  NEW  K | Text\n", 'B');
    push_styled(&mut text, &mut styles, "---------------\n", 'B');

    for row in &view.rows {
        let row_style = match row.kind {
            crate::diff_view::DiffViewRowKind::Context => 'A',
            crate::diff_view::DiffViewRowKind::Delete => 'D',
            crate::diff_view::DiffViewRowKind::Insert => 'C',
            crate::diff_view::DiffViewRowKind::ReplaceOld
            | crate::diff_view::DiffViewRowKind::ReplaceNew => 'H',
            crate::diff_view::DiffViewRowKind::Fold | crate::diff_view::DiffViewRowKind::Notice => {
                'G'
            }
        };

        let old_no = format_line_no(row.old_line);
        let new_no = format_line_no(row.new_line);
        push_styled(&mut text, &mut styles, &old_no, 'I');
        push_styled(&mut text, &mut styles, " | ", 'I');
        push_styled(&mut text, &mut styles, &new_no, 'I');
        push_styled(&mut text, &mut styles, " | ", 'I');
        push_styled(
            &mut text,
            &mut styles,
            &format!("{:<1}", row.marker),
            'I',
        );
        push_styled(&mut text, &mut styles, " | ", 'I');

        for segment in &row.segments {
            let segment_style = match segment.kind {
                crate::diff_view::DiffViewSegmentKind::Normal => row_style,
                crate::diff_view::DiffViewSegmentKind::DeleteToken => 'E',
                crate::diff_view::DiffViewSegmentKind::InsertToken => 'F',
            };
            push_styled(&mut text, &mut styles, &segment.text, segment_style);
        }
        push_styled(&mut text, &mut styles, "\n", row_style);
    }

    RenderedDiff { text, styles }
}

fn format_line_no(line: Option<usize>) -> String {
    match line {
        Some(value) => format!("{value:<3}"),
        None => "   ".to_string(),
    }
}

fn push_styled(text: &mut String, styles: &mut String, value: &str, style: char) {
    text.push_str(value);
    styles.extend(std::iter::repeat_n(style, value.len()));
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
            status_bg: Color::from_rgb(239, 236, 228),
            secondary_button: Color::from_rgb(240, 238, 232),
            insert_bg: Color::from_rgb(232, 244, 234), // #E8F4EA
            delete_bg: Color::from_rgb(248, 231, 225), // #F8E7E1
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
            status_bg: Color::from_rgb(37, 40, 32),
            secondary_button: Color::from_rgb(46, 49, 42),
            insert_bg: Color::from_rgb(31, 58, 41), // #1F3A29
            delete_bg: Color::from_rgb(68, 37, 31), // #44251F
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
    fn render_diff_view_text_shows_semantic_old_new_gutters() {
        use crate::{
            diff_core::{DiffOptions, build_display_diff},
            diff_view::build_diff_view,
        };

        let diff = build_display_diff("a\nc\n", "a\nb\nc\n", &DiffOptions::default());
        let view = build_diff_view(&diff, &DiffOptions::default());
        let rendered = render_diff_view_text(&view);

        assert!(rendered.text.contains("OLD  NEW  K | Text\n"));
        assert!(rendered.text.contains("    | 2   | + | b"));
        assert!(rendered.text.contains("2   | 3   |   | c"));
        assert_eq!(rendered.text.len(), rendered.styles.len());
    }

    #[test]
    fn render_diff_view_text_marks_replacement_rows_neutral_with_token_styles() {
        use crate::{
            diff_core::{DiffOptions, build_display_diff},
            diff_view::build_diff_view,
        };

        let diff = build_display_diff(
            "let mode = \"old\";\n",
            "let mode = \"new\";\n",
            &DiffOptions::default(),
        );
        let view = build_diff_view(&diff, &DiffOptions::default());
        let rendered = render_diff_view_text(&view);

        assert!(rendered.text.contains("~ | let mode"));
        assert!(rendered.styles.contains('H'), "replacement block style required");
        assert!(rendered.styles.contains('E'), "delete token style required");
        assert!(rendered.styles.contains('F'), "insert token style required");
        assert_eq!(rendered.text.len(), rendered.styles.len());
    }

    /// Regression for the stale-diff path (Task 7 review, Fix 1):
    /// prepending the stale-diff banner must keep `text.len() == styles.len()`,
    /// otherwise FLTK's byte-indexed ext highlight resolves every style char
    /// against the wrong offset.
    #[test]
    fn with_leading_notice_keeps_text_and_styles_aligned() {
        use crate::{
            diff_core::{DiffOptions, build_display_diff},
            diff_view::build_diff_view,
        };
        let diff = build_display_diff(
            "i wanna eatt banana",
            "i wanna eat bananas",
            &DiffOptions::default(),
        );
        let view = build_diff_view(&diff, &DiffOptions::default());
        let rendered = render_diff_view_text(&view);
        // sanity: the base render is already aligned
        assert_eq!(rendered.text.len(), rendered.styles.len());

        let noticed = with_leading_notice(
            rendered,
            "Previous diff is stale. Press Compare to update.\n\n",
            'G',
        );
        assert_eq!(noticed.text.len(), noticed.styles.len());
        assert!(
            noticed
                .text
                .starts_with("Previous diff is stale. Press Compare to update.\n\n"),
            "notice must lead the text"
        );
        // every byte of the notice carries the chosen style char
        let notice_len = "Previous diff is stale. Press Compare to update.\n\n".len();
        let (head, tail) = noticed.styles.split_at(notice_len);
        assert!(
            head.chars().all(|c| c == 'G'),
            "notice region must be uniformly styled 'G'"
        );
        assert!(!tail.is_empty(), "body styles must follow the notice");
        assert!(
            tail.contains('F'),
            "body inline-insert style must survive the prepend"
        );
    }

    #[test]
    fn diff_summary_label_formats_counts() {
        let summary = crate::diff_view::ChangeSummary {
            removed: 2,
            added: 3,
            edited: 1,
        };

        assert_eq!(
            diff_summary_label(&summary),
            "2 removed  3 added  1 edited"
        );
    }
}
