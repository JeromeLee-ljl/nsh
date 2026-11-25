//! Where soft-wrapped text may break, shared by the editor's rows and the
//! diff view's wrapped lines so both wrap alike: between two ASCII chars
//! after whitespace (code wraps between its tokens; an operator such as
//! `->` or a path stays whole), next to any other char where Unicode's line
//! breaking (UAX #14) allows, and never inside a grapheme.

use unicode_segmentation::GraphemeCursor;

/// How far before a row UAX #14 may look for context.
const BREAK_CONTEXT: usize = 64;

/// The last break opportunity of the row from `row_start` of `text` after
/// it and `lead` and at or before `at` (a grapheme's offset), by the offset
/// of the char after it. Between two ASCII chars it is after whitespace;
/// elsewhere it is UAX #14's, at a grapheme boundary. UAX #14 runs
/// backwards from `at` a span at a time, each from where its breaks no
/// longer depend on the text before it (`split_at_safe`): a few chars, not
/// the whole line. A span starts a char before that point (so it can break
/// there), but that char lacks its own context (a combining mark, its
/// base's class): a break before the point is the next span's to decide.
/// Nothing looks further back than [`BREAK_CONTEXT`] bytes before the row,
/// so a line wraps in time linear in its length; text without such a point
/// that long (a rare run, as of spaces with combining marks) takes its
/// breaks as if the line started there.
pub(crate) fn last_break(text: &str, row_start: usize, lead: usize, at: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let ascii_pair = |offset: usize| bytes[offset - 1].is_ascii() && bytes[offset].is_ascii();
    let floor = row_start.max(lead);
    let mut context = row_start.saturating_sub(BREAK_CONTEXT);
    while !text.is_char_boundary(context) {
        context -= 1;
    }
    // A row starts at a grapheme boundary: graphemes are found from there.
    let row = &text[row_start..];
    // Opportunities after `start`, up to `bound`, are looked for; `bound`
    // is a char's offset, and the span ends after that char, so it decides
    // the break before it.
    let mut bound = at;
    loop {
        let end = bound + text[bound..].chars().next().map_or(0, char::len_utf8);
        let (before, span) = unicode_linebreak::split_at_safe(&text[context..end]);
        let span_start = context + before.len();
        let start = if before.is_empty() {
            span_start
        } else {
            span_start + span.chars().next().map_or(0, char::len_utf8)
        };
        let mut found = None;
        for (offset, _) in unicode_linebreak::linebreaks(span) {
            let offset = span_start + offset;
            if offset > bound {
                break;
            }
            if offset > start
                && offset > floor
                && !ascii_pair(offset)
                && is_grapheme_boundary(row, offset - row_start)
            {
                found = Some(offset);
            }
        }
        // Between ASCII chars (always a grapheme boundary: no `\r\n` in a
        // line), after whitespace.
        if let Some(offset) = (start.max(floor) + 1..=bound)
            .rev()
            .find(|&offset| ascii_pair(offset) && char::from(bytes[offset - 1]).is_whitespace())
        {
            found = found.max(Some(offset));
        }
        if found.is_some() || start <= floor {
            return found;
        }
        bound = start;
    }
}

fn is_grapheme_boundary(text: &str, offset: usize) -> bool {
    GraphemeCursor::new(offset, text.len(), true)
        .is_boundary(text, 0)
        .unwrap_or(false)
}
