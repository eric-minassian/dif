#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    Context,
    Added,
    Removed,
    Meta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffCell {
    pub line_no: Option<usize>,
    pub text: String,
    pub kind: CellKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffRow {
    pub old: Option<DiffCell>,
    pub new: Option<DiffCell>,
}

pub fn parse_unified_diff(diff_text: &str) -> Vec<DiffRow> {
    let mut rows = Vec::new();

    let mut in_hunk = false;
    let mut old_line = 0usize;
    let mut new_line = 0usize;

    let mut removed_run = Vec::new();
    let mut added_run = Vec::new();

    for raw_line in diff_text.lines() {
        if raw_line.starts_with("@@") {
            flush_change_run(&mut rows, &mut removed_run, &mut added_run);
            if let Some((next_old, next_new)) = parse_hunk_header(raw_line) {
                old_line = next_old;
                new_line = next_new;
                in_hunk = true;
            }
            continue;
        }

        if !in_hunk {
            continue;
        }

        if raw_line.starts_with('\\') {
            continue;
        }

        let mut chars = raw_line.chars();
        let marker = chars.next();
        let content = chars.as_str().to_string();

        match marker {
            Some(' ') => {
                flush_change_run(&mut rows, &mut removed_run, &mut added_run);

                let old_cell = DiffCell {
                    line_no: normalize_line_no(old_line),
                    text: content.clone(),
                    kind: CellKind::Context,
                };
                let new_cell = DiffCell {
                    line_no: normalize_line_no(new_line),
                    text: content,
                    kind: CellKind::Context,
                };
                rows.push(DiffRow {
                    old: Some(old_cell),
                    new: Some(new_cell),
                });

                old_line += 1;
                new_line += 1;
            }
            Some('-') => {
                removed_run.push(DiffCell {
                    line_no: normalize_line_no(old_line),
                    text: content,
                    kind: CellKind::Removed,
                });
                old_line += 1;
            }
            Some('+') => {
                added_run.push(DiffCell {
                    line_no: normalize_line_no(new_line),
                    text: content,
                    kind: CellKind::Added,
                });
                new_line += 1;
            }
            _ => {}
        }
    }

    flush_change_run(&mut rows, &mut removed_run, &mut added_run);

    if rows.is_empty() {
        for line in diff_text.lines() {
            let meta = DiffCell {
                line_no: None,
                text: line.to_owned(),
                kind: CellKind::Meta,
            };
            rows.push(DiffRow {
                old: Some(meta.clone()),
                new: Some(meta),
            });
        }
    }

    rows
}

fn flush_change_run(
    rows: &mut Vec<DiffRow>,
    removed_run: &mut Vec<DiffCell>,
    added_run: &mut Vec<DiffCell>,
) {
    let row_count = removed_run.len().max(added_run.len());
    for idx in 0..row_count {
        rows.push(DiffRow {
            old: removed_run.get(idx).cloned(),
            new: added_run.get(idx).cloned(),
        });
    }

    removed_run.clear();
    added_run.clear();
}

fn parse_hunk_header(header: &str) -> Option<(usize, usize)> {
    let closing_at = header[2..].find("@@")? + 2;
    let body = header[2..closing_at].trim();

    let mut old_start = None;
    let mut new_start = None;

    for segment in body.split_whitespace() {
        if let Some(rest) = segment.strip_prefix('-') {
            old_start = parse_range_start(rest);
        } else if let Some(rest) = segment.strip_prefix('+') {
            new_start = parse_range_start(rest);
        }
    }

    Some((old_start?, new_start?))
}

fn parse_range_start(range: &str) -> Option<usize> {
    range.split(',').next()?.parse::<usize>().ok()
}

fn normalize_line_no(raw: usize) -> Option<usize> {
    if raw == 0 { None } else { Some(raw) }
}

#[cfg(test)]
mod tests {
    use super::{CellKind, parse_unified_diff};

    #[test]
    fn aligns_replaced_line_blocks() {
        let input = "@@ -1,3 +1,3 @@\n-old_a\n-old_b\n+new_a\n+new_b\n keep";
        let rows = parse_unified_diff(input);

        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[0].old.as_ref().map(|c| c.kind),
            Some(CellKind::Removed)
        );
        assert_eq!(rows[0].new.as_ref().map(|c| c.kind), Some(CellKind::Added));
        assert_eq!(
            rows[1].old.as_ref().map(|c| c.kind),
            Some(CellKind::Removed)
        );
        assert_eq!(rows[1].new.as_ref().map(|c| c.kind), Some(CellKind::Added));
        assert_eq!(
            rows[2].old.as_ref().map(|c| c.kind),
            Some(CellKind::Context)
        );
        assert_eq!(
            rows[2].new.as_ref().map(|c| c.kind),
            Some(CellKind::Context)
        );
    }

    #[test]
    fn keeps_unmatched_added_lines_on_new_side() {
        let input = "@@ -2,1 +2,3 @@\n same\n+plus_a\n+plus_b";
        let rows = parse_unified_diff(input);

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1].old, None);
        assert_eq!(rows[1].new.as_ref().map(|c| c.kind), Some(CellKind::Added));
        assert_eq!(rows[2].old, None);
        assert_eq!(rows[2].new.as_ref().map(|c| c.kind), Some(CellKind::Added));
    }
}
