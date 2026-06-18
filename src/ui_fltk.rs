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
    text::{StyleTableEntry, TextBuffer, TextDisplay, TextEditor},
    window::Window,
};

use crate::{
    app_state::{AppState, STATUS_CLEARED},
    config::{
        AppConfig, ConfigLoadStatus, MIN_HEIGHT, MIN_WIDTH, Theme, config_path,
        load_config_from_path, save_config_to_path,
    },
    diff_core::{
        DEBOUNCE_MS, DiffLineKind, InlineDiffSegmentKind, classify_diff_line, inline_diff_match,
        should_auto_diff,
    },
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
    hunk_text: Color,
    header_text: Color,
    status_bg: Color,
    secondary_button: Color,
}

const ACTION_BAR_HEIGHT: i32 = 34;
const STATUS_BAR_HEIGHT: i32 = 26;
const ROOT_MARGIN: i32 = 8;
const ROOT_PAD: i32 = 8;
const PANE_GAP: i32 = 8;
const LINE_NUMBER_WIDTH: i32 = 44;
const STACK_INPUT_WIDTH: i32 = 760;

struct UiHandles {
    left_editor: TextEditor,
    right_editor: TextEditor,
    left_buffer: TextBuffer,
    right_buffer: TextBuffer,
    diff_buffer: TextBuffer,
    diff_style_buffer: TextBuffer,
    status: Frame,
    copy_diff: Button,
}

#[derive(Debug, Clone)]
enum UiMessage {
    DiffReady(crate::app_state::DiffResult),
}

pub fn run() -> Result<(), FltkError> {
    let app = app::App::default().with_scheme(app::Scheme::Gtk);

    let state = Rc::new(RefCell::new(AppState::default()));
    let config_file = config_path().ok();
    let config = config_file
        .as_ref()
        .map(load_config_from_path)
        .unwrap_or_else(|| crate::config::ConfigLoadResult {
            config: AppConfig::default(),
            status: ConfigLoadStatus::Missing,
        });
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

    let (mut diff_display, diff_buffer, diff_style_buffer) = make_diff_display(palette);
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
    configure_line_numbers(&mut display, palette);
    display.set_text_font(Font::Courier);
    display.set_text_size(14);
    display.set_color(palette.pane);
    display.set_frame(FrameType::BorderBox);
    display.set_highlight_data(style_buffer.clone(), style_table(palette));
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
    if !should_auto_diff(state.borrow().left(), state.borrow().right()) {
        render_state(state, handles);
        return;
    }

    let generation = debounce_generation.get().saturating_add(1);
    debounce_generation.set(generation);
    let state = state.clone();
    let handles = handles.clone();
    let debounce_generation = debounce_generation.clone();
    app::add_timeout3(DEBOUNCE_MS as f64 / 1000.0, move |_| {
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
    let diff = state_snapshot.diff().to_string();
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
    let mut status = handles.status.clone();
    let mut copy_diff = handles.copy_diff.clone();
    let source_diff = rendered_diff_text(&state);
    let rendered_diff = render_diff_display(&source_diff);
    diff_buffer.set_text(&rendered_diff.text);
    diff_style_buffer.set_text(&rendered_diff.styles);
    status.set_label(state.status());
    if state.has_current_diff() {
        copy_diff.activate();
    } else {
        copy_diff.deactivate();
    }
}

fn rendered_diff_text(state: &AppState) -> String {
    if state.has_stale_diff() {
        format!(
            "Previous diff is stale. Press Compare to update.\n\n{}",
            state.diff()
        )
    } else {
        state.diff().to_string()
    }
}

fn style_table(palette: Palette) -> Vec<StyleTableEntry> {
    vec![
        StyleTableEntry {
            color: palette.text,
            font: Font::Courier,
            size: 14,
        },
        StyleTableEntry {
            color: palette.header_text,
            font: Font::Courier,
            size: 14,
        },
        StyleTableEntry {
            color: palette.hunk_text,
            font: Font::Courier,
            size: 14,
        },
        StyleTableEntry {
            color: palette.insert_text,
            font: Font::Courier,
            size: 14,
        },
        StyleTableEntry {
            color: palette.delete_text,
            font: Font::Courier,
            size: 14,
        },
        StyleTableEntry {
            color: palette.insert_text,
            font: Font::CourierBold,
            size: 14,
        },
        StyleTableEntry {
            color: palette.delete_text,
            font: Font::CourierBold,
            size: 14,
        },
    ]
}

struct RenderedDiff {
    text: String,
    styles: String,
}

fn render_diff_display(diff: &str) -> RenderedDiff {
    let mut text = String::with_capacity(diff.len());
    let mut styles = String::with_capacity(diff.len());
    let lines = diff.split_inclusive('\n').collect::<Vec<_>>();
    let mut index = 0;
    while index < lines.len() {
        if is_change_block_start(&lines, index) {
            let end = change_block_end(&lines, index);
            push_change_block(&mut text, &mut styles, &lines[index..end]);
            index = end;
            continue;
        }

        let line = lines[index];
        text.push_str(line);
        styles.push_str(&plain_style_line(line));
        index += 1;
    }

    RenderedDiff { text, styles }
}

fn is_change_block_start(lines: &[&str], index: usize) -> bool {
    let kind = classify_diff_line(lines[index].trim_end_matches('\n'));
    if kind != DiffLineKind::Delete && kind != DiffLineKind::Insert {
        return false;
    }

    let end = change_block_end(lines, index);
    let has_delete = lines[index..end]
        .iter()
        .any(|line| classify_diff_line(line.trim_end_matches('\n')) == DiffLineKind::Delete);
    let has_insert = lines[index..end]
        .iter()
        .any(|line| classify_diff_line(line.trim_end_matches('\n')) == DiffLineKind::Insert);
    has_delete && has_insert
}

fn change_block_end(lines: &[&str], start: usize) -> usize {
    let mut end = start;
    while end < lines.len() {
        let kind = classify_diff_line(lines[end].trim_end_matches('\n'));
        if kind != DiffLineKind::Delete && kind != DiffLineKind::Insert {
            break;
        }
        end += 1;
    }
    end
}

fn push_change_block(text: &mut String, styles: &mut String, block: &[&str]) {
    let deletes = block
        .iter()
        .filter(|line| classify_diff_line(line.trim_end_matches('\n')) == DiffLineKind::Delete)
        .copied()
        .collect::<Vec<_>>();
    let inserts = block
        .iter()
        .filter(|line| classify_diff_line(line.trim_end_matches('\n')) == DiffLineKind::Insert)
        .copied()
        .collect::<Vec<_>>();
    let pairs = best_inline_pairs(&deletes, &inserts);

    for (delete_index, delete_line) in deletes.iter().enumerate() {
        if let Some(inline) = pairs
            .iter()
            .find(|pair| pair.delete_index == delete_index)
            .map(|pair| &pair.inline)
        {
            push_inline_replacement_line(text, styles, &inline.segments);
        } else {
            push_diff_line(text, styles, delete_line);
        }
    }

    for (insert_index, insert_line) in inserts.iter().enumerate() {
        if pairs.iter().any(|pair| pair.insert_index == insert_index) {
            continue;
        }
        push_diff_line(text, styles, insert_line);
    }
}

#[derive(Debug)]
struct InlinePair {
    delete_index: usize,
    insert_index: usize,
    inline: crate::diff_core::InlineDiffMatch,
}

fn best_inline_pairs(deletes: &[&str], inserts: &[&str]) -> Vec<InlinePair> {
    let mut candidates = Vec::new();
    for (delete_index, delete_line) in deletes.iter().enumerate() {
        for (insert_index, insert_line) in inserts.iter().enumerate() {
            let delete_trimmed = delete_line.trim_end_matches('\n');
            let insert_trimmed = insert_line.trim_end_matches('\n');
            if let Some(inline) = inline_diff_match(delete_trimmed, insert_trimmed) {
                candidates.push(InlinePair {
                    delete_index,
                    insert_index,
                    inline,
                });
            }
        }
    }

    candidates.sort_by(|left, right| {
        left.inline
            .changed_ratio
            .total_cmp(&right.inline.changed_ratio)
    });

    let mut pairs = Vec::new();
    for candidate in candidates {
        if pairs
            .iter()
            .any(|pair: &InlinePair| pair.delete_index == candidate.delete_index)
        {
            continue;
        }
        if pairs
            .iter()
            .any(|pair: &InlinePair| pair.insert_index == candidate.insert_index)
        {
            continue;
        }
        pairs.push(candidate);
    }

    pairs.sort_by_key(|pair| pair.delete_index);
    pairs
}

fn push_inline_replacement_line(
    text: &mut String,
    styles: &mut String,
    segments: &[crate::diff_core::InlineDiffSegment],
) {
    push_styled_text(text, styles, "~ ", 'C');
    for segment in segments {
        match segment.kind {
            InlineDiffSegmentKind::Equal => push_styled_text(text, styles, &segment.text, 'A'),
            InlineDiffSegmentKind::Delete => {
                push_styled_text(text, styles, "[-", 'E');
                push_styled_text(text, styles, &segment.text, 'G');
                push_styled_text(text, styles, "]", 'E');
            }
            InlineDiffSegmentKind::Insert => {
                push_styled_text(text, styles, "[+", 'D');
                push_styled_text(text, styles, &segment.text, 'F');
                push_styled_text(text, styles, "]", 'D');
            }
        }
    }
    push_styled_text(text, styles, "\n", 'A');
}

fn push_diff_line(text: &mut String, styles: &mut String, line: &str) {
    text.push_str(line);
    styles.push_str(&plain_style_line(line));
}

fn push_styled_text(text: &mut String, styles: &mut String, value: &str, style: char) {
    text.push_str(value);
    styles.extend(std::iter::repeat_n(style, value.len()));
}

fn plain_style_line(line: &str) -> String {
    let style = match classify_diff_line(line.trim_end_matches('\n')) {
        DiffLineKind::Context => 'A',
        DiffLineKind::Header => 'B',
        DiffLineKind::Hunk => 'C',
        DiffLineKind::Insert => 'D',
        DiffLineKind::Delete => 'E',
    };
    std::iter::repeat_n(style, line.len()).collect()
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
            hunk_text: Color::from_rgb(66, 82, 107),
            header_text: Color::from_rgb(110, 103, 94),
            status_bg: Color::from_rgb(239, 236, 228),
            secondary_button: Color::from_rgb(240, 238, 232),
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
            hunk_text: Color::from_rgb(183, 198, 230),
            header_text: Color::from_rgb(166, 159, 145),
            status_bg: Color::from_rgb(37, 40, 32),
            secondary_button: Color::from_rgb(46, 49, 42),
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
    fn render_diff_display_compacts_reliable_single_line_replacements() {
        let rendered = render_diff_display(
            "--- left\n+++ right\n@@ -1 +1 @@\n-i wanna eat bananas\n+i wanna eat banana\n",
        );

        assert_eq!(
            rendered.text,
            "--- left\n+++ right\n@@ -1 +1 @@\n~ i wanna eat banana[-s]\n"
        );
        assert_eq!(rendered.text.len(), rendered.styles.len());
        assert!(rendered.styles.contains('G'));
    }

    #[test]
    fn render_diff_display_pairs_replacements_inside_multi_line_change_blocks() {
        let rendered = render_diff_display(
            "--- left\n+++ right\n@@ -1,2 +1 @@\n-i wanna eat bananas\n-1\n+i wanna eaate banana\n",
        );

        assert_eq!(
            rendered.text,
            "--- left\n+++ right\n@@ -1,2 +1 @@\n~ i wanna ea[+a]t[+e] banana[-s]\n-1\n"
        );
        assert_eq!(rendered.text.len(), rendered.styles.len());
        assert!(rendered.styles.contains('F'));
        assert!(rendered.styles.contains('G'));
    }

    #[test]
    fn render_diff_display_pairs_mid_sized_replacements_as_inline_lines() {
        let rendered = render_diff_display("--- left\n+++ right\n@@ -1 +1 @@\n-fuck\n+fk\n");

        assert!(rendered.text.contains("~ f[-uc]k\n"));
        assert_eq!(rendered.text.len(), rendered.styles.len());
        assert!(rendered.styles.contains('G'));
    }

    #[test]
    fn render_diff_display_keeps_large_replacements_line_level() {
        let rendered = render_diff_display("--- left\n+++ right\n@@ -1 +1 @@\n-abcdef\n+uvwxyz\n");

        assert!(rendered.text.contains("-abcdef\n+uvwxyz\n"));
        assert_eq!(rendered.text.len(), rendered.styles.len());
        assert!(!rendered.styles.contains('F'));
        assert!(!rendered.styles.contains('G'));
    }
}
