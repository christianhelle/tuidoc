mod common;

use common::{app, item, p, rich_app, styled, table, text, type_keys, typed};
use tuidoc::app::{App, Mode};
use tuidoc::changes::Register;
use tuidoc::model::{BlockPath, Position};

fn at(app: &App) -> (usize, usize) {
    let cursor = app.cursor();
    (cursor.path.top(), cursor.offset)
}

fn two() -> App {
    app(&format!("{}{}", p("one two three"), p("four five")))
}

fn bold(app: &App, paragraph: usize, offset: usize) -> bool {
    let document = app.document();
    let paragraph = document.paragraph(BlockPath::Top(paragraph)).unwrap();
    document
        .styles
        .format(&paragraph.props, paragraph.cells()[offset].props)
        .bold
}

// ---- moving ----

#[test]
fn l_and_h_move_along_the_paragraph_and_stop_at_its_ends() {
    let app = typed(two(), "lll");
    assert_eq!(at(&app), (0, 3));
    let app = typed(app, "99l");
    assert_eq!(at(&app), (0, 12));
    let app = typed(app, "99h");
    assert_eq!(at(&app), (0, 0));
}

#[test]
fn w_b_and_e_move_by_words_and_cross_paragraphs() {
    let app = typed(two(), "w");
    assert_eq!(at(&app), (0, 4));
    let app = typed(app, "e");
    assert_eq!(at(&app), (0, 6));
    let app = typed(app, "ww");
    assert_eq!(at(&app), (1, 0));
    let app = typed(app, "b");
    assert_eq!(at(&app), (0, 8));
}

#[test]
fn dollar_and_zero_go_to_the_ends_of_the_paragraph() {
    let app = typed(two(), "$");
    assert_eq!(at(&app), (0, 12));
    let app = typed(app, "0");
    assert_eq!(at(&app), (0, 0));
}

#[test]
fn j_moves_by_screen_line_within_a_wrapped_paragraph() {
    let long = "word ".repeat(40);
    let app = typed(app(&format!("{}{}", p(&long), p("next"))), "j");
    assert_eq!(app.cursor().path, BlockPath::Top(0));
    assert_eq!(app.cursor().offset, 60);
}

#[test]
fn j_and_k_keep_to_their_column_through_a_short_line() {
    let app = app(&format!(
        "{}{}{}",
        p("a long first line"),
        p("x"),
        p("a long third line")
    ));
    let app = typed(app, "8ljj");
    assert_eq!(at(&app), (2, 8));
    let app = typed(app, "k");
    assert_eq!(at(&app), (1, 0));
}

#[test]
fn g_twice_jumps_to_the_top_and_capital_g_to_the_bottom() {
    let app = typed(two(), "G");
    assert_eq!(at(&app), (1, 0));
    let app = typed(app, "gg");
    assert_eq!(at(&app), (0, 0));
    let app = typed(app, "2G");
    assert_eq!(at(&app), (1, 0));
}

#[test]
fn braces_move_by_paragraph() {
    let app = typed(two(), "}");
    assert_eq!(at(&app), (1, 0));
    let app = typed(app, "{");
    assert_eq!(at(&app), (0, 0));
}

#[test]
fn f_finds_a_character_and_semicolon_finds_it_again() {
    let app = typed(two(), "ft");
    assert_eq!(at(&app), (0, 4));
    let app = typed(app, ";");
    assert_eq!(at(&app), (0, 8));
    let app = typed(app, ",");
    assert_eq!(at(&app), (0, 4));
}

#[test]
fn double_brackets_jump_between_headings() {
    let app = rich_app(&format!(
        "{}{}{}{}",
        styled("Heading1", "One"),
        p("text"),
        styled("Heading2", "Two"),
        p("more")
    ));
    let app = typed(app, "]]");
    assert_eq!(at(&app), (2, 0));
    let app = typed(app, "[[");
    assert_eq!(at(&app), (0, 0));
}

#[test]
fn tab_moves_from_cell_to_cell_in_a_table() {
    let app = typed(
        app(&format!(
            "{}{}",
            table(&[&["a", "b"], &["c", "d"]]),
            p("after")
        )),
        "⇥",
    );
    assert_eq!(
        app.cursor().path,
        BlockPath::Cell {
            table: 0,
            row: 0,
            cell: 1,
            block: 0
        }
    );
    let app = typed(app, "⇥");
    assert_eq!(
        app.cursor().path,
        BlockPath::Cell {
            table: 0,
            row: 1,
            cell: 0,
            block: 0
        }
    );
    let app = typed(app, "⇤");
    assert_eq!(
        app.cursor().path,
        BlockPath::Cell {
            table: 0,
            row: 0,
            cell: 1,
            block: 0
        }
    );
}

#[test]
fn j_goes_down_a_tables_column() {
    let app = typed(app(&table(&[&["a", "b"], &["c", "d"]])), "⇥j");
    assert_eq!(
        app.cursor().path,
        BlockPath::Cell {
            table: 0,
            row: 1,
            cell: 1,
            block: 0
        }
    );
}

#[test]
fn slash_searches_without_regard_to_case_and_n_repeats() {
    let app = typed(
        app(&format!("{}{}", p("Alpha beta"), p("gamma BETA"))),
        "/beta⏎",
    );
    assert_eq!(at(&app), (0, 6));
    let app = typed(app, "n");
    assert_eq!(at(&app), (1, 6));
    let app = typed(app, "n");
    assert_eq!(at(&app), (0, 6));
    let app = typed(app, "N");
    assert_eq!(at(&app), (1, 6));
}

#[test]
fn a_search_that_finds_nothing_says_so() {
    let app = typed(two(), "/zebra⏎");
    assert!(app.status_is_error());
    assert_eq!(app.status(), "Not found: zebra");
}

#[test]
fn star_searches_for_the_word_under_the_cursor() {
    let app = typed(app(&format!("{}{}", p("one two"), p("two one"))), "*");
    assert_eq!(at(&app), (1, 4));
}

#[test]
fn ctrl_g_asks_for_a_paragraph_number() {
    let app = typed(two(), "⌃g");
    assert_eq!(app.mode(), Mode::GoTo);
    let app = typed(app, "2⏎");
    assert_eq!(app.mode(), Mode::Normal);
    assert_eq!(at(&app), (1, 0));
}

#[test]
fn key_releases_are_ignored() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    let mut app = two();
    app.handle_key(KeyEvent {
        code: KeyCode::Char('l'),
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Release,
        state: KeyEventState::NONE,
    });
    assert_eq!(at(&app), (0, 0));
}

// ---- typing ----

#[test]
fn i_types_text_and_escape_steps_back() {
    let app = typed(two(), "iHello ⎋");
    assert_eq!(text(&app)[0], "Hello one two three");
    assert_eq!(app.mode(), Mode::Normal);
    assert_eq!(at(&app), (0, 5));
}

#[test]
fn a_and_capital_a_type_after_the_cursor_and_at_the_end() {
    let app = typed(two(), "aX⎋");
    assert_eq!(text(&app)[0], "oXne two three");
    let app = typed(app, "A!⎋");
    assert_eq!(text(&app)[0], "oXne two three!");
}

#[test]
fn enter_splits_the_paragraph() {
    let app = typed(two(), "wi⏎⎋");
    assert_eq!(text(&app), ["one ", "two three", "four five"]);
    assert_eq!(at(&app), (1, 0));
}

#[test]
fn backspace_at_the_start_joins_with_the_paragraph_before() {
    let app = typed(two(), "ji⌫⎋");
    assert_eq!(text(&app), ["one two threefour five"]);
    assert_eq!(at(&app), (0, 12));
}

#[test]
fn delete_at_the_end_joins_with_the_paragraph_after() {
    let app = typed(two(), "A⌦⎋");
    assert_eq!(text(&app), ["one two threefour five"]);
}

#[test]
fn o_opens_a_paragraph_below_and_capital_o_above() {
    let app = typed(two(), "onew⎋");
    assert_eq!(text(&app), ["one two three", "new", "four five"]);
    let app = typed(app, "Otop⎋");
    assert_eq!(text(&app), ["one two three", "top", "new", "four five"]);
}

#[test]
fn ctrl_w_deletes_the_word_before_the_cursor() {
    let app = typed(two(), "A more⌃w⎋");
    assert_eq!(text(&app)[0], "one two three ");
}

#[test]
fn typing_in_a_table_cell_changes_only_that_cell() {
    let app = typed(app(&table(&[&["a", "b"], &["c", "d"]])), "⇥Ax⎋");
    assert_eq!(text(&app), ["a", "bx", "c", "d"]);
}

#[test]
fn enter_on_an_empty_list_item_ends_the_list() {
    let app = typed(rich_app(&item(1, 0, "first")), "o⏎");
    let document = app.document();
    let second = document.paragraph(BlockPath::Top(1)).unwrap();
    assert!(document.styles.list_item(&second.props).is_none());
    assert_eq!(text(&app), ["first", ""]);
}

#[test]
fn a_new_item_after_a_list_item_is_in_the_list() {
    let app = typed(rich_app(&item(1, 0, "first")), "osecond⎋");
    let document = app.document();
    let second = document.paragraph(BlockPath::Top(1)).unwrap();
    assert!(document.styles.list_item(&second.props).is_some());
}

#[test]
fn enter_at_the_end_of_a_heading_starts_ordinary_text() {
    let app = typed(rich_app(&styled("Heading1", "Title")), "A⏎body⎋");
    let document = app.document();
    let second = document.paragraph(BlockPath::Top(1)).unwrap();
    assert_eq!(document.styles.heading_level(&second.props), None);
    assert_eq!(text(&app), ["Title", "body"]);
}

#[test]
fn pasted_text_goes_in_and_line_feeds_start_paragraphs() {
    let mut app = typed(two(), "i");
    app.handle_paste("alpha\r\nbeta ");
    assert_eq!(text(&app), ["alpha", "beta one two three", "four five"]);
}

#[test]
fn the_other_stories_cannot_be_edited() {
    let app = typed(rich_app(&p("body")), "gtix");
    assert_eq!(app.mode(), Mode::Normal);
    assert!(app.status_is_error());
    assert_eq!(
        app.status(),
        "Headers, footers and notes can't be edited yet"
    );
}

// ---- operators ----

#[test]
fn x_deletes_characters() {
    let app = typed(two(), "3x");
    assert_eq!(text(&app)[0], " two three");
}

#[test]
fn dw_deletes_a_word_and_stops_at_the_paragraphs_end() {
    let app = typed(two(), "dw");
    assert_eq!(text(&app)[0], "two three");
    let app = typed(app, "wdw");
    assert_eq!(text(&app), ["two ", "four five"]);
}

#[test]
fn d_dollar_and_capital_d_delete_to_the_end() {
    let app = typed(two(), "wd$");
    assert_eq!(text(&app)[0], "one ");
    let app = typed(app, "jwD");
    assert_eq!(text(&app)[1], "four ");
}

#[test]
fn de_takes_the_character_it_lands_on() {
    let app = typed(two(), "de");
    assert_eq!(text(&app)[0], " two three");
}

#[test]
fn dd_deletes_the_paragraph_and_p_puts_it_back_below() {
    let app = typed(two(), "dd");
    assert_eq!(text(&app), ["four five"]);
    let app = typed(app, "p");
    assert_eq!(text(&app), ["four five", "one two three"]);
    assert_eq!(at(&app), (1, 0));
}

#[test]
fn deleting_every_paragraph_leaves_an_empty_one() {
    let app = typed(two(), "2dd");
    assert_eq!(text(&app), [""]);
    assert_eq!(at(&app), (0, 0));
}

#[test]
fn dj_deletes_two_paragraphs() {
    let app = typed(app(&format!("{}{}{}", p("a"), p("b"), p("c"))), "dj");
    assert_eq!(text(&app), ["c"]);
}

#[test]
fn diw_and_daw_delete_the_word_under_the_cursor() {
    let app = typed(two(), "wldiw");
    assert_eq!(text(&app)[0], "one  three");
    let app = typed(two(), "wldaw");
    assert_eq!(text(&app)[0], "one three");
}

#[test]
fn cw_changes_a_word() {
    let app = typed(two(), "cwONE ⎋");
    assert_eq!(text(&app)[0], "ONE two three");
}

#[test]
fn cc_empties_the_paragraph_and_types() {
    let app = typed(two(), "ccnew⎋");
    assert_eq!(text(&app), ["new", "four five"]);
}

#[test]
fn yw_copies_and_capital_p_pastes_before_the_cursor() {
    let app = typed(two(), "ywP");
    assert_eq!(text(&app)[0], "one one two three");
}

#[test]
fn yy_copies_the_paragraph_for_the_clipboard_too() {
    let mut app = typed(two(), "yy");
    assert!(matches!(app.register(), Some(Register::Lines(blocks)) if blocks.len() == 1));
    assert_eq!(app.take_clipboard().as_deref(), Some("one two three\n"));
    assert_eq!(app.take_clipboard(), None);
}

#[test]
fn r_replaces_one_character() {
    let app = typed(two(), "rO");
    assert_eq!(text(&app)[0], "One two three");
    assert_eq!(at(&app), (0, 0));
}

#[test]
fn capital_j_joins_paragraphs_with_a_space() {
    let app = typed(two(), "J");
    assert_eq!(text(&app), ["one two three four five"]);
}

#[test]
fn deleting_across_paragraphs_joins_what_is_left() {
    let app = typed(
        app(&format!(
            "{}{}{}",
            p("one two"),
            p("middle"),
            p("three four")
        )),
        "wvjjd",
    );
    assert_eq!(text(&app), ["one  four"]);
}

#[test]
fn a_change_cannot_run_out_of_a_table_cell() {
    let app = typed(app(&format!("{}{}", table(&[&["a"]]), p("after"))), "vjd");
    assert!(app.status_is_error());
    assert_eq!(text(&app), ["a", "after"]);
}

#[test]
fn deleting_a_cells_only_paragraph_leaves_it_empty() {
    let app = typed(app(&table(&[&["a", "b"]])), "dd");
    assert_eq!(text(&app), ["", "b"]);
}

// ---- visual ----

#[test]
fn v_selects_characters_and_d_deletes_them() {
    let app = typed(two(), "vee");
    assert_eq!(app.mode(), Mode::Visual);
    assert!(app.is_selected(BlockPath::Top(0), 0) && app.is_selected(BlockPath::Top(0), 6));
    assert!(!app.is_selected(BlockPath::Top(0), 7));
    let app = typed(app, "d");
    assert_eq!(text(&app)[0], " three");
    assert_eq!(app.mode(), Mode::Normal);
}

#[test]
fn capital_v_selects_whole_paragraphs() {
    let app = typed(app(&format!("{}{}{}", p("a"), p("b"), p("c"))), "Vjd");
    assert_eq!(text(&app), ["c"]);
}

#[test]
fn o_swaps_the_ends_of_the_selection() {
    let app = typed(two(), "wvlo");
    assert_eq!(at(&app), (0, 4));
    let app = typed(app, "hd");
    assert_eq!(text(&app)[0], "oneo three");
}

#[test]
fn escape_leaves_visual_mode() {
    let app = typed(two(), "v⎋");
    assert_eq!(app.mode(), Mode::Normal);
}

// ---- undo ----

#[test]
fn u_undoes_and_ctrl_r_redoes_with_the_cursor_where_it_was() {
    let app = typed(two(), "wdw");
    assert_eq!(text(&app)[0], "one three");
    let app = typed(app, "u");
    assert_eq!(text(&app)[0], "one two three");
    assert_eq!(at(&app), (0, 4));
    let app = typed(app, "⌃r");
    assert_eq!(text(&app)[0], "one three");
}

#[test]
fn everything_typed_in_one_go_undoes_as_one() {
    let app = typed(two(), "ihello⏎world⎋u");
    assert_eq!(text(&app), ["one two three", "four five"]);
}

#[test]
fn undoing_back_to_the_start_leaves_nothing_unsaved() {
    let app = typed(two(), "x");
    assert!(app.document().is_dirty());
    let app = typed(app, "u");
    assert!(!app.document().is_dirty());
    let app = typed(app, "u");
    assert_eq!(app.status(), "Nothing to undo");
}

#[test]
fn a_new_change_after_undo_leaves_nothing_to_redo() {
    let app = typed(two(), "xux⌃r");
    assert_eq!(app.status(), "Nothing to redo");
}

// ---- formatting ----

#[test]
fn gb_with_a_motion_makes_text_bold_and_again_plain() {
    let app = typed(two(), "gbiw");
    assert!(bold(&app, 0, 0) && bold(&app, 0, 2));
    assert!(!bold(&app, 0, 3));
    assert_eq!(text(&app)[0], "one two three");
    let app = typed(app, "gbiw");
    assert!(!bold(&app, 0, 0));
}

#[test]
fn bold_over_partly_bold_text_makes_all_of_it_bold() {
    let app = typed(two(), "gbiwgb$");
    assert!(bold(&app, 0, 0) && bold(&app, 0, 12));
}

#[test]
fn in_visual_mode_gb_formats_the_selection() {
    let app = typed(two(), "wvegb");
    assert!(!bold(&app, 0, 3));
    assert!(bold(&app, 0, 4) && bold(&app, 0, 6));
    assert!(!bold(&app, 0, 7));
    assert_eq!(app.mode(), Mode::Normal);
}

#[test]
fn bold_is_switched_off_in_a_heading_by_saying_so() {
    let app = typed(rich_app(&styled("Heading1", "Title")), "gbip");
    assert!(!bold(&app, 0, 0));
    let paragraph = app.document().paragraph(BlockPath::Top(0)).unwrap();
    assert_eq!(
        paragraph.cells()[0].props.get("b"),
        Some("<w:b w:val=\"0\"/>")
    );
}

#[test]
fn typing_after_bold_text_is_bold() {
    let app = typed(two(), "gbiweaX⎋");
    assert_eq!(text(&app)[0], "oneX two three");
    assert!(bold(&app, 0, 3));
}

#[test]
fn g_and_a_digit_make_a_heading_and_g0_ordinary_text() {
    let app = typed(rich_app(&p("Title")), "g2");
    let level = |app: &App| {
        let document = app.document();
        document
            .styles
            .heading_level(&document.paragraph(BlockPath::Top(0)).unwrap().props)
    };
    assert_eq!(level(&app), Some(2));
    let app = typed(app, "g0");
    assert_eq!(level(&app), None);
}

#[test]
fn a_heading_style_the_file_lacks_is_added_to_it() {
    let app = typed(rich_app(&p("Title")), "g4");
    let document = app.document();
    let props = &document.paragraph(BlockPath::Top(0)).unwrap().props;
    assert_eq!(document.styles.heading_level(props), Some(4));
    assert_eq!(props.val("pStyle").as_deref(), Some("Heading4"));
}

#[test]
fn gl_cycles_from_no_list_to_bullets_to_numbers_and_back() {
    let listed = |app: &App| {
        let document = app.document();
        let props = &document.paragraph(BlockPath::Top(0)).unwrap().props;
        document
            .styles
            .list_item(props)
            .map(|(list, _)| document.numbering.is_bulleted(&list).unwrap())
    };
    let app = typed(rich_app(&p("item")), "gl");
    assert_eq!(listed(&app), Some(true));
    let app = typed(app, "gl");
    assert_eq!(listed(&app), Some(false));
    let app = typed(app, "gl");
    assert_eq!(listed(&app), None);
}

#[test]
fn a_list_is_made_in_a_document_that_has_no_numbering() {
    let app = typed(app(&p("item")), "gl");
    let document = app.document();
    let props = &document.paragraph(BlockPath::Top(0)).unwrap().props;
    let (list, depth) = document.styles.list_item(props).unwrap();
    assert_eq!(depth, 0);
    assert_eq!(document.numbering.is_bulleted(&list), Some(true));
}

#[test]
fn angle_brackets_move_a_list_item_deeper_and_back() {
    let depth = |app: &App| {
        let document = app.document();
        let props = &document.paragraph(BlockPath::Top(0)).unwrap().props;
        document.styles.list_item(props).unwrap().1
    };
    let app = typed(rich_app(&item(1, 0, "item")), ">>");
    assert_eq!(depth(&app), 1);
    let app = typed(app, "<<<<");
    assert_eq!(depth(&app), 0);
}

#[test]
fn angle_brackets_indent_an_ordinary_paragraph() {
    let app = typed(two(), ">>>>");
    let props = &app.document().paragraph(BlockPath::Top(0)).unwrap().props;
    assert_eq!(props.get("ind"), Some("<w:ind w:left=\"1440\"/>"));
    let app = typed(app, "<<");
    let props = &app.document().paragraph(BlockPath::Top(0)).unwrap().props;
    assert_eq!(props.get("ind"), Some("<w:ind w:left=\"720\"/>"));
}

#[test]
fn gh_picks_a_style_from_a_list_that_narrows() {
    let app = typed(rich_app(&p("text")), "gh");
    assert_eq!(app.mode(), Mode::StylePicker);
    let app = typed(app, "quo⏎");
    assert_eq!(app.mode(), Mode::Normal);
    let props = &app.document().paragraph(BlockPath::Top(0)).unwrap().props;
    assert_eq!(props.val("pStyle").as_deref(), Some("Quote"));
}

// ---- stories, outline, peek ----

#[test]
fn gt_and_capital_gt_go_round_the_stories() {
    let app = typed(rich_app(&p("body")), "gt");
    assert_eq!(app.story().title, "Header");
    let app = typed(app, "gT");
    assert_eq!(app.story().title, "Body");
    let app = typed(app, "gT");
    assert_eq!(app.story().title, "Comments");
}

#[test]
fn capital_k_shows_the_footnote_under_the_cursor() {
    let body = "<w:p><w:r><w:t>See</w:t></w:r><w:r><w:footnoteReference w:id=\"1\"/></w:r></w:p>";
    let app = typed(rich_app(body), "$K");
    assert_eq!(app.mode(), Mode::Peek);
    assert_eq!(app.peek(), ["[1] A note at the foot."]);
    let app = typed(app, "x");
    assert_eq!(app.mode(), Mode::Normal);
    assert_eq!(text(&app)[0], "See\u{fffc}");
}

#[test]
fn capital_k_shows_where_a_link_leads() {
    let body = r#"<w:p><w:hyperlink r:id="rId6"><w:r><w:t>site</w:t></w:r></w:hyperlink></w:p>"#;
    let app = typed(rich_app(body), "K");
    assert_eq!(app.peek(), ["https://example.com/"]);
}

#[test]
fn capital_k_on_plain_text_has_nothing_to_show() {
    let app = typed(two(), "K");
    assert_eq!(app.mode(), Mode::Normal);
    assert_eq!(app.status(), "Nothing to look up here");
}

#[test]
fn the_outline_lists_headings_and_enter_goes_to_one() {
    let app = rich_app(&format!(
        "{}{}{}",
        styled("Heading1", "One"),
        p("text"),
        styled("Heading2", "Two")
    ));
    let app = typed(app, "gO");
    assert_eq!(app.mode(), Mode::Outline);
    assert_eq!(app.outline().len(), 2);
    let app = typed(app, "j⏎");
    assert_eq!(app.mode(), Mode::Normal);
    assert_eq!(at(&app), (2, 0));
    assert!(app.outline_is_open());
    let app = typed(app, "gOgO");
    assert!(!app.outline_is_open());
}

// ---- quitting ----

#[test]
fn q_quits_a_document_with_nothing_unsaved() {
    let app = typed(two(), "q");
    assert!(app.should_quit());
}

#[test]
fn q_asks_before_losing_changes() {
    let app = typed(two(), "xq");
    assert_eq!(app.mode(), Mode::ConfirmDiscard);
    assert!(!app.should_quit());
    let stays = typed(app, "⎋");
    assert_eq!(stays.mode(), Mode::Normal);
    let app = typed(stays, "qd");
    assert!(app.should_quit());
}

#[test]
fn capital_zq_quits_without_asking() {
    let app = typed(two(), "xZQ");
    assert!(app.should_quit());
}

#[test]
fn question_mark_shows_help_and_any_key_closes_it() {
    let app = typed(two(), "?");
    assert_eq!(app.mode(), Mode::Help);
    let app = typed(app, "x");
    assert_eq!(app.mode(), Mode::Normal);
    assert_eq!(text(&app)[0], "one two three");
}

#[test]
fn keys_typed_so_far_are_shown_until_the_command_is_complete() {
    let mut app = two();
    type_keys(&mut app, "2d3");
    assert_eq!(app.pending_keys(), "2d3");
    type_keys(&mut app, "l");
    assert_eq!(app.pending_keys(), "");
    assert_eq!(text(&app)[0], "o three");
}

#[test]
fn the_cursor_never_rests_past_the_last_character() {
    let app = typed(two(), "$x");
    assert_eq!(at(&app), (0, 11));
    assert_eq!(app.cursor(), Position::new(BlockPath::Top(0), 11));
}
