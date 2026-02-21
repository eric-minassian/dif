use ratatui::layout::Rect;

pub(crate) fn contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x
        && x < rect.x.saturating_add(rect.width)
        && y >= rect.y
        && y < rect.y.saturating_add(rect.height)
}

pub(crate) fn ensure_visible(
    selected: Option<usize>,
    len: usize,
    visible: usize,
    scroll: &mut usize,
) {
    if len == 0 || visible == 0 {
        *scroll = 0;
        return;
    }

    *scroll = (*scroll).min(len.saturating_sub(visible));

    let Some(selected) = selected else {
        return;
    };

    if selected < *scroll {
        *scroll = selected;
        return;
    }

    let bottom = *scroll + visible;
    if selected >= bottom {
        *scroll = selected + 1 - visible;
    }
}

pub(crate) fn shift_and_clamp_u16(value: u16, delta: isize, step: u16, min: u16, max: u16) -> u16 {
    let candidate = value as i32 + (delta as i32 * step as i32);
    candidate.clamp(min as i32, max as i32) as u16
}

pub(crate) fn order_positions(
    a: (usize, usize),
    b: (usize, usize),
) -> ((usize, usize), (usize, usize)) {
    if a.0 < b.0 || (a.0 == b.0 && a.1 <= b.1) {
        (a, b)
    } else {
        (b, a)
    }
}

pub(crate) fn find_query_in_line(line: &str, query: &str, start_col: usize) -> Option<usize> {
    if query.is_empty() {
        return None;
    }

    let line_chars = line.chars().collect::<Vec<_>>();
    let query_chars = query.chars().collect::<Vec<_>>();

    if query_chars.len() > line_chars.len() {
        return None;
    }

    let from = start_col.min(line_chars.len());
    for idx in from..=line_chars.len().saturating_sub(query_chars.len()) {
        if line_chars[idx..idx + query_chars.len()] == query_chars[..] {
            return Some(idx);
        }
    }

    None
}

pub(crate) fn clamp_text_cursor(text: &str, cursor: usize) -> usize {
    let capped = cursor.min(text.len());
    if text.is_char_boundary(capped) {
        return capped;
    }

    text.char_indices()
        .map(|(idx, _)| idx)
        .take_while(|idx| *idx < capped)
        .last()
        .unwrap_or(0)
}

pub(crate) fn prev_text_cursor(text: &str, cursor: usize) -> usize {
    let cursor = clamp_text_cursor(text, cursor);
    if cursor == 0 {
        return 0;
    }

    text[..cursor]
        .char_indices()
        .last()
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

pub(crate) fn next_text_cursor(text: &str, cursor: usize) -> usize {
    let cursor = clamp_text_cursor(text, cursor);
    if cursor >= text.len() {
        return text.len();
    }

    cursor
        + text[cursor..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(0)
}

pub(crate) fn move_text_cursor_home(text: &str, cursor: usize) -> usize {
    current_line_start(text, cursor)
}

pub(crate) fn move_text_cursor_end(text: &str, cursor: usize) -> usize {
    current_line_end(text, cursor)
}

pub(crate) fn move_text_cursor_up(text: &str, cursor: usize) -> usize {
    let cursor = clamp_text_cursor(text, cursor);
    let start = current_line_start(text, cursor);
    if start == 0 {
        return cursor;
    }

    let col = text[start..cursor].chars().count();
    let prev_end = start.saturating_sub(1);
    let prev_start = text[..prev_end].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    let prev_line = &text[prev_start..prev_end];

    let target_col = col.min(prev_line.chars().count());
    prev_start + byte_index_for_char_offset(prev_line, target_col)
}

pub(crate) fn move_text_cursor_down(text: &str, cursor: usize) -> usize {
    let cursor = clamp_text_cursor(text, cursor);
    let start = current_line_start(text, cursor);
    let col = text[start..cursor].chars().count();
    let end = current_line_end(text, cursor);

    if end == text.len() {
        return cursor;
    }

    let next_start = end + 1;
    let next_end = text[next_start..]
        .find('\n')
        .map(|rel| next_start + rel)
        .unwrap_or(text.len());
    let next_line = &text[next_start..next_end];

    let target_col = col.min(next_line.chars().count());
    next_start + byte_index_for_char_offset(next_line, target_col)
}

fn current_line_start(text: &str, cursor: usize) -> usize {
    let cursor = clamp_text_cursor(text, cursor);
    text[..cursor].rfind('\n').map(|idx| idx + 1).unwrap_or(0)
}

fn current_line_end(text: &str, cursor: usize) -> usize {
    let cursor = clamp_text_cursor(text, cursor);
    text[cursor..]
        .find('\n')
        .map(|rel| cursor + rel)
        .unwrap_or(text.len())
}

fn byte_index_for_char_offset(text: &str, char_offset: usize) -> usize {
    text.char_indices()
        .nth(char_offset)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len())
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;

    use super::{
        clamp_text_cursor, contains, ensure_visible, find_query_in_line, move_text_cursor_down,
        move_text_cursor_end, move_text_cursor_home, move_text_cursor_up, next_text_cursor,
        prev_text_cursor, shift_and_clamp_u16,
    };

    #[test]
    fn contains_checks_inside_and_outside_bounds() {
        let rect = Rect::new(2, 3, 4, 2);

        assert!(contains(rect, 2, 3));
        assert!(contains(rect, 5, 4));
        assert!(!contains(rect, 6, 4));
        assert!(!contains(rect, 5, 5));
    }

    #[test]
    fn ensure_visible_adjusts_scroll_for_selection() {
        let mut scroll = 0;
        ensure_visible(Some(5), 10, 3, &mut scroll);
        assert_eq!(scroll, 3);

        ensure_visible(Some(2), 10, 3, &mut scroll);
        assert_eq!(scroll, 2);
    }

    #[test]
    fn shift_and_clamp_handles_bounds() {
        assert_eq!(shift_and_clamp_u16(10, 2, 4, 8, 20), 18);
        assert_eq!(shift_and_clamp_u16(10, 5, 4, 8, 20), 20);
        assert_eq!(shift_and_clamp_u16(10, -5, 4, 8, 20), 8);
    }

    #[test]
    fn finds_query_from_start_column() {
        assert_eq!(find_query_in_line("abcdef", "cd", 0), Some(2));
        assert_eq!(find_query_in_line("abcdef", "cd", 3), None);
        assert_eq!(find_query_in_line("abcdef", "", 0), None);
    }

    #[test]
    fn text_cursor_navigation_respects_char_boundaries() {
        let text = "aé🙂b";

        assert_eq!(clamp_text_cursor(text, 2), 1);
        assert_eq!(next_text_cursor(text, 0), 1);
        assert_eq!(next_text_cursor(text, 1), 3);
        assert_eq!(next_text_cursor(text, 3), 7);
        assert_eq!(prev_text_cursor(text, 7), 3);
        assert_eq!(prev_text_cursor(text, 3), 1);
    }

    #[test]
    fn text_cursor_moves_between_lines_with_column_preference() {
        let text = "ab\n12345\nxy";
        let line_two_col_four = 3 + "1234".len();

        assert_eq!(move_text_cursor_up(text, line_two_col_four), 2);
        assert_eq!(move_text_cursor_down(text, 2), 5);
        assert_eq!(move_text_cursor_down(text, text.len()), text.len());
    }

    #[test]
    fn text_cursor_home_and_end_are_line_local() {
        let text = "first\nsecond\nthird";
        let in_second = 6 + 3;

        assert_eq!(move_text_cursor_home(text, in_second), 6);
        assert_eq!(move_text_cursor_end(text, in_second), 12);
    }
}
