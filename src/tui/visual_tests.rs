//! Visual selection rendering: which cells get the visual style in
//! charwise, linewise and block visual mode.

use ratatui::text::Span;

use super::editor::Mode;
use super::harness::{Harness, Screen};
use super::style;

/// The x positions on row `y` painted with the visual background. A wide
/// character's second cell is not sent to the terminal, which paints it
/// with the first cell's style, so it counts with the first.
fn selected(screen: &Screen, y: u16) -> Vec<u16> {
    let area = screen.buffer.area;
    let mut out = Vec::new();
    let mut x = area.left();
    while x < area.right() {
        let cell = &screen.buffer[(x, y)];
        let w = u16::try_from(Span::raw(cell.symbol()).width().max(1)).unwrap_or(1);
        if cell.bg == style::VISUAL_BG {
            out.extend(x..(x + w).min(area.right()));
        }
        x += w;
    }
    out
}

fn span(from: u16, to: u16) -> Vec<u16> {
    (from..=to).collect()
}

// The gutter is 5 cells for fewer than 100 lines, and the input column
// ends at 48 on the 80-column harness screen.

#[test]
fn charwise_covers_anchor_to_cursor_inclusive() {
    let mut h = Harness::at(&["abcdef"], 0, 1);
    h.keys("vll");
    let screen = h.render();
    assert_eq!(selected(&screen, 0), span(6, 8));
    for x in 6..=8 {
        assert_eq!(screen.buffer[(x, 0)].fg, style::VISUAL_FG, "cell {x}");
    }
    assert!(screen.rows[0].starts_with(" 1 │ abcdef "));
}

#[test]
fn charwise_across_lines() {
    let mut h = Harness::at(&["abcd", "efgh", "ijkl"], 0, 2);
    h.keys("vj");
    let screen = h.render();
    assert_eq!(selected(&screen, 0), span(7, 8));
    assert_eq!(selected(&screen, 1), span(5, 7));
    assert!(selected(&screen, 2).is_empty());
}

#[test]
fn charwise_whole_middle_lines() {
    let mut h = Harness::at(&["abcd", "ef", "ijkl"], 0, 3);
    h.keys("vjj");
    assert_eq!(h.cursor(), (2, 3));
    let screen = h.render();
    assert_eq!(selected(&screen, 0), vec![8]);
    assert_eq!(selected(&screen, 1), span(5, 6));
    assert_eq!(selected(&screen, 2), span(5, 8));
}

#[test]
fn linewise_covers_the_full_input_column() {
    let mut h = Harness::new(&["12", "345", "6"]);
    h.keys("Vj");
    assert_eq!(h.mode(), Mode::VisualLine);
    let screen = h.render();
    // Text, padding after it, nothing in the gutter or the results column.
    assert_eq!(selected(&screen, 0), span(5, 47));
    assert_eq!(selected(&screen, 1), span(5, 47));
    assert!(selected(&screen, 2).is_empty());
    assert!(screen.rows[1].starts_with(" 2 │ 345 "));
    assert!(screen.rows[1].trim_end().ends_with("345"));
}

#[test]
fn backward_on_one_line() {
    let mut h = Harness::new(&["abcdef"]);
    h.keys("$vhh");
    assert_eq!(h.cursor(), (0, 3));
    let screen = h.render();
    assert_eq!(selected(&screen, 0), span(8, 10));
}

#[test]
fn backward_across_lines() {
    let mut h = Harness::at(&["abcd", "efgh"], 1, 1);
    h.keys("vk");
    assert_eq!(h.cursor(), (0, 1));
    let screen = h.render();
    assert_eq!(selected(&screen, 0), span(6, 8));
    assert_eq!(selected(&screen, 1), span(5, 6));
}

#[test]
fn wide_characters_line_up() {
    // a=5, 日=6-7, b=8, 😀=9-10, c=11.
    let mut h = Harness::new(&["a日b😀c"]);
    h.keys("vlll");
    let screen = h.render();
    assert_eq!(selected(&screen, 0), span(5, 10));
    assert_eq!(screen.buffer[(6, 0)].symbol(), "日");
    assert_eq!(screen.buffer[(9, 0)].symbol(), "😀");
    assert_eq!(screen.buffer[(11, 0)].symbol(), "c");
}

#[test]
fn selection_starting_after_a_wide_character() {
    let mut h = Harness::at(&["日本x"], 0, 1);
    h.keys("vl");
    let screen = h.render();
    // 日=5-6, 本=7-8, x=9.
    assert_eq!(selected(&screen, 0), span(7, 9));
}

#[test]
fn selection_stops_where_clipped_text_stops() {
    let long = "1+".repeat(40) + "1";
    let mut h = Harness::sized(&[&long], 40, 5);
    h.keys("v$");
    let screen = h.render();
    // Input column is 24 wide: text in cells 5..24.
    assert_eq!(selected(&screen, 0), span(5, 23));
}

#[test]
fn wide_character_dropped_at_clip_edge_is_not_painted() {
    let line = format!("{}日", "a".repeat(18));
    let mut h = Harness::sized(&[&line], 40, 5);
    h.keys("v$");
    let screen = h.render();
    assert_eq!(selected(&screen, 0), span(5, 22));
}

#[test]
fn linewise_is_clipped_to_the_input_column() {
    let mut h = Harness::sized(&["1+1"], 40, 5);
    h.keys("V");
    let screen = h.render();
    assert_eq!(selected(&screen, 0), span(5, 23));
}

#[test]
fn block_paints_a_rectangle() {
    let mut h = Harness::at(&["abcd", "efgh", "ijkl"], 0, 1);
    h.keys("<C-v>jl");
    assert_eq!(h.mode(), Mode::VisualBlock);
    let screen = h.render();
    assert_eq!(selected(&screen, 0), span(6, 7));
    assert_eq!(selected(&screen, 1), span(6, 7));
    assert!(selected(&screen, 2).is_empty());
}

#[test]
fn block_to_line_end_is_ragged() {
    let mut h = Harness::at(&["abcd", "ef", "ijklmn"], 0, 1);
    h.keys("<C-v>jj$");
    let screen = h.render();
    assert_eq!(selected(&screen, 0), span(6, 8));
    assert_eq!(selected(&screen, 1), vec![6]);
    assert_eq!(selected(&screen, 2), span(6, 10));
}

#[test]
fn no_selection_after_esc() {
    for keys in ["vll<Esc>", "Vj<Esc>", "<C-v>jl<Esc>", "vlly"] {
        let mut h = Harness::new(&["abcd", "efgh"]);
        h.keys(keys);
        assert_eq!(h.mode(), Mode::Normal, "{keys}");
        let screen = h.render();
        for y in 0..screen.buffer.area.height {
            assert!(selected(&screen, y).is_empty(), "{keys}: row {y}");
        }
    }
}

#[test]
fn cursor_stays_on_its_character() {
    let mut h = Harness::at(&["abcdef"], 0, 1);
    h.keys("vll");
    let cursor = h.render().cursor.expect("cursor shown");
    assert_eq!((cursor.x, cursor.y), (8, 0));
}
