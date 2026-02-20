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

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;

    use super::{contains, ensure_visible, find_query_in_line, shift_and_clamp_u16};

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
}
