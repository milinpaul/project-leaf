//! Tests for pasting plain text and Markdown, and writing Markdown and plain text on copy (E05-S06).
//! The last tests paste a real backlog story (`story-E05-S01.md`).

use leaf_core::{AssetId, BlockId};
use leaf_model::{BlockKind, Caret, Marks, PageCommand, PageDoc, PageOutcome, TextSpan};

use super::{fragment_from_text, inline, parse};
use crate::{Fragment, FragmentBlock, PageEditor};

fn span(text: &str, marks: Marks) -> TextSpan {
    TextSpan {
        text: text.into(),
        marks,
    }
}

fn plain(text: &str) -> TextSpan {
    span(text, Marks::default())
}

fn block(kind: BlockKind, text: &str) -> FragmentBlock {
    FragmentBlock {
        kind,
        text: if text.is_empty() {
            Vec::new()
        } else {
            vec![plain(text)]
        },
        children: Vec::new(),
    }
}

fn with_children(mut parent: FragmentBlock, children: Vec<FragmentBlock>) -> FragmentBlock {
    parent.children = children;
    parent
}

/// A readable picture of blocks: one line each, indented by nesting, marks in brackets.
fn picture(blocks: &[FragmentBlock]) -> String {
    fn walk(blocks: &[FragmentBlock], depth: usize, out: &mut String) {
        for b in blocks {
            let kind = match &b.kind {
                BlockKind::Heading { level } => format!("h{level}"),
                BlockKind::Todo { checked } => format!("todo[{}]", if *checked { "x" } else { " " }),
                BlockKind::Code { language } => format!("code({language})"),
                other => other.type_name().to_owned(),
            };
            let text: String = b
                .text
                .iter()
                .map(|s| {
                    let mut tags = Vec::new();
                    if s.marks.bold {
                        tags.push("bold".to_owned());
                    }
                    if s.marks.italic {
                        tags.push("italic".to_owned());
                    }
                    if s.marks.strike {
                        tags.push("strike".to_owned());
                    }
                    if s.marks.code {
                        tags.push("code".to_owned());
                    }
                    if let Some(l) = &s.marks.link {
                        tags.push(format!("link {l}"));
                    }
                    if tags.is_empty() {
                        s.text.replace('\n', "⏎")
                    } else {
                        format!("«{}»[{}]", s.text.replace('\n', "⏎"), tags.join(","))
                    }
                })
                .collect();
            out.push_str(&"  ".repeat(depth));
            out.push_str(&kind);
            out.push_str(": ");
            out.push_str(&text);
            out.push('\n');
            walk(&b.children, depth + 1, out);
        }
    }
    let mut out = String::new();
    walk(blocks, 0, &mut out);
    out
}

// ---- Reading blocks ----

#[test]
fn plain_lines_become_paragraphs_and_blank_lines_are_skipped() {
    assert_eq!(
        parse("first line\r\nsecond line\n\n\nthird"),
        vec![
            block(BlockKind::Paragraph, "first line"),
            block(BlockKind::Paragraph, "second line"),
            block(BlockKind::Paragraph, "third"),
        ]
    );
}

#[test]
fn headings_lists_todos_quotes_and_dividers() {
    let text = "# One #\n## Two\n### Three\n#### Four\n#NoSpace\n- dash\n* star\n+ plus\n1. one\n2) two\n- [ ] open\n- [x] done\n[] bare\n> quoted\n---\n* * *\n___";
    assert_eq!(
        picture(&parse(text)),
        "h1: One\nh2: Two\nh3: Three\nh3: Four\nparagraph: #NoSpace\nbulleted_list: dash\nbulleted_list: star\n\
         bulleted_list: plus\nnumbered_list: one\nnumbered_list: two\ntodo[ ]: open\ntodo[x]: done\ntodo[ ]: bare\n\
         quote: quoted\ndivider: \ndivider: \ndivider: \n"
    );
}

#[test]
fn indentation_nests_and_wrapped_lines_continue_their_item() {
    let text = "- parent\n  - child\n    - grandchild\n  wrapped onto two lines\n- next\n\tindented by tab\n  - tab child";
    assert_eq!(
        picture(&parse(text)),
        "bulleted_list: parent\n  bulleted_list: child\n    bulleted_list: grandchild wrapped onto two lines\n\
         bulleted_list: next indented by tab\n  bulleted_list: tab child\n"
    );
    // Not indented, or after a blank line: a paragraph of its own.
    assert_eq!(
        picture(&parse("- item\nnot indented\n- item\n\n  after a gap")),
        "bulleted_list: item\nparagraph: not indented\nbulleted_list: item\n  paragraph: after a gap\n"
    );
}

#[test]
fn code_fences_keep_their_text_exactly_and_their_language() {
    let text = "```rust\nfn main() {\n    let a = **not bold**;\n}\n```\n````\n```\nnested fence\n```\n````\n```\nnever closed\n# still code";
    assert_eq!(
        picture(&parse(text)),
        "code(rust): fn main() {⏎    let a = **not bold**;⏎}\ncode(): ```⏎nested fence⏎```\ncode(): never closed⏎# still code\n"
    );
    assert_eq!(picture(&parse("```\n```")), "code(): \n", "an empty fence");
}

// ---- Inline formatting ----

#[test]
fn inline_marks_links_and_code() {
    let bold = Marks {
        bold: true,
        ..Marks::default()
    };
    let italic = Marks {
        italic: true,
        ..Marks::default()
    };
    let link = |l: &str| Marks {
        link: Some(l.into()),
        ..Marks::default()
    };
    assert_eq!(
        inline::parse("**b** *i* __b2__ _i2_ ~~s~~ `c`"),
        vec![
            span("b", bold.clone()),
            plain(" "),
            span("i", italic.clone()),
            plain(" "),
            span("b2", bold.clone()),
            plain(" "),
            span("i2", italic.clone()),
            plain(" "),
            span(
                "s",
                Marks {
                    strike: true,
                    ..Marks::default()
                }
            ),
            plain(" "),
            span(
                "c",
                Marks {
                    code: true,
                    ..Marks::default()
                }
            ),
        ]
    );
    assert_eq!(
        inline::parse("see [the **docs**](https://leaf.app) or <https://x.y>"),
        vec![
            plain("see "),
            span("the ", link("https://leaf.app")),
            span(
                "docs",
                Marks {
                    bold: true,
                    link: Some("https://leaf.app".into()),
                    ..Marks::default()
                }
            ),
            plain(" or "),
            span("https://x.y", link("https://x.y")),
        ]
    );
}

#[test]
fn things_that_only_look_like_formatting_stay_text() {
    for text in [
        "snake_case_name",
        "a * b * c",
        "unclosed **bold",
        "[not a link] and [also](not one) and <not a url>",
        "[E04-S05] then (later)",
        "``` alone",
        "``` and ` and ``",
        "price: $5 ~ $6",
    ] {
        assert_eq!(inline::parse(text), vec![plain(text)], "{text:?}");
    }
    assert_eq!(
        inline::parse(r"\*kept\* \[x\]"),
        vec![plain("*kept* [x]")],
        "escapes"
    );
    assert_eq!(
        inline::parse("```` ``` ```` and `` ` ``"),
        vec![
            span(
                "```",
                Marks {
                    code: true,
                    ..Marks::default()
                }
            ),
            plain(" and "),
            span(
                "`",
                Marks {
                    code: true,
                    ..Marks::default()
                }
            ),
        ],
        "a code span holds shorter backtick runs"
    );
    assert_eq!(
        inline::parse("``` and `---`"),
        vec![
            plain("``` and "),
            span(
                "---",
                Marks {
                    code: true,
                    ..Marks::default()
                }
            )
        ],
        "an unmatched run stays text, whole, and doesn't pair with a later backtick"
    );
}

#[test]
fn one_line_pastes_into_the_line_and_more_becomes_blocks() {
    assert_eq!(
        fragment_from_text("just **words**\n"),
        Fragment::Text(vec![
            plain("just "),
            span(
                "words",
                Marks {
                    bold: true,
                    ..Marks::default()
                }
            )
        ])
    );
    assert!(matches!(fragment_from_text("# a heading"), Fragment::Blocks(_)));
    assert!(matches!(fragment_from_text("```"), Fragment::Blocks(_)));
    assert!(matches!(fragment_from_text("two\nlines"), Fragment::Blocks(_)));
}

// ---- Writing on copy ----

#[test]
fn copy_writes_markdown_by_the_mapping_table() {
    let bold = Marks {
        bold: true,
        ..Marks::default()
    };
    let fragment = Fragment::Blocks(vec![
        block(BlockKind::Heading { level: 2 }, "Plan"),
        FragmentBlock {
            kind: BlockKind::Paragraph,
            text: vec![plain("make "), span("it", bold), plain(" 2*3")],
            children: Vec::new(),
        },
        with_children(
            block(BlockKind::NumberedList, "first"),
            vec![block(BlockKind::Todo { checked: true }, "done")],
        ),
        block(BlockKind::NumberedList, "second"),
        block(BlockKind::Quote, "wise words"),
        block(
            BlockKind::Code {
                language: "rust".into(),
            },
            "let a = 1;\nlet b = 2;",
        ),
        block(BlockKind::Divider, ""),
        FragmentBlock {
            kind: BlockKind::Image {
                asset: AssetId::of(b"pic"),
                width: None,
                alt: "a leaf".into(),
            },
            text: vec![plain("caption")],
            children: Vec::new(),
        },
        block(BlockKind::Paragraph, "- not a list"),
        block(BlockKind::Paragraph, "3. not a number"),
    ]);
    let asset = AssetId::of(b"pic");
    assert_eq!(
        fragment.to_markdown(),
        format!(
            "## Plan\n\nmake **it** 2\\*3\n\n1. first\n  - [x] done\n2. second\n\n> wise words\n\n```rust\nlet a = 1;\nlet b = 2;\n```\n\n---\n\n![a leaf](assets/{asset})\n*caption*\n\n\\- not a list\n\n3\\. not a number"
        )
    );
    assert_eq!(
        fragment.to_plain_text(),
        "Plan\nmake it 2*3\nfirst\ndone\nsecond\nwise words\nlet a = 1;\nlet b = 2;\ncaption\n- not a list\n3. not a number"
    );
    let text = Fragment::Text(vec![span(
        "x",
        Marks {
            code: true,
            link: Some("u".into()),
            ..Marks::default()
        },
    )]);
    assert_eq!(
        (text.to_markdown(), text.to_plain_text()),
        ("[`x`](u)".to_owned(), "x".to_owned())
    );
}

#[test]
fn copied_markdown_escapes_only_what_could_read_as_formatting() {
    let text = "depends_on: [E04-S05] a_b _edge_ [x](y) <https://u> *star* `tick` ~tilde~ \\";
    let written = inline::write(&[plain(text)]);
    assert_eq!(
        written,
        r"depends_on: [E04-S05] a_b \_edge\_ \[x](y) \<https://u> \*star\* \`tick\` \~tilde\~ \\"
    );
    assert_eq!(inline::parse(&written), vec![plain(text)]);
}

#[test]
fn markdown_written_on_copy_reads_back_as_the_same_blocks() {
    let all = Marks {
        bold: true,
        italic: true,
        strike: true,
        code: false,
        link: Some("https://leaf.app".into()),
    };
    let blocks = vec![
        block(
            BlockKind::Heading { level: 1 },
            "Title with *stars* and _underscores_",
        ),
        FragmentBlock {
            kind: BlockKind::Paragraph,
            text: vec![
                plain("mixed "),
                span("everything", all),
                plain(" and "),
                span(
                    "a `tick`",
                    Marks {
                        code: true,
                        ..Marks::default()
                    },
                ),
            ],
            children: Vec::new(),
        },
        with_children(
            block(BlockKind::BulletedList, "parent"),
            vec![with_children(
                block(BlockKind::Todo { checked: false }, "child"),
                vec![block(BlockKind::NumberedList, "deep")],
            )],
        ),
        block(BlockKind::Paragraph, "# not a heading"),
        block(BlockKind::Paragraph, "---"),
        block(
            BlockKind::Code {
                language: String::new(),
            },
            "```inner fence```\n  indented",
        ),
    ];
    let markdown = Fragment::Blocks(blocks.clone()).to_markdown();
    assert_eq!(parse(&markdown), blocks, "{markdown}");
}

// ---- Pasting into a page ----

fn editor_with(blocks: &[(BlockKind, &str)]) -> (PageEditor, Vec<BlockId>) {
    let mut doc = PageDoc::new();
    let ids = blocks
        .iter()
        .map(|(kind, text)| {
            let index = doc.root_blocks().len();
            let Ok(PageOutcome::BlockCreated(id)) = doc.apply(PageCommand::InsertBlock {
                parent: None,
                index,
                kind: kind.clone(),
                text: (*text).into(),
            }) else {
                panic!("insert failed");
            };
            id
        })
        .collect();
    let mut doc = PageDoc::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    doc.set_undo_merge_interval(0);
    (PageEditor::new(doc), ids)
}

fn page_picture(editor: &PageEditor) -> String {
    fn tree(editor: &PageEditor, ids: Vec<BlockId>) -> Vec<FragmentBlock> {
        ids.into_iter()
            .map(|id| FragmentBlock {
                kind: editor.doc().kind(id).unwrap(),
                text: editor.doc().text(id).unwrap().unwrap_or_default(),
                children: tree(editor, editor.doc().children(id).unwrap()),
            })
            .collect()
    }
    picture(&tree(editor, editor.doc().root_blocks()))
}

#[test]
fn pasting_multi_line_text_into_a_page_makes_blocks_in_one_undo_step() {
    let (mut editor, ids) = editor_with(&[(BlockKind::Paragraph, "")]);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    editor.paste_text("# Groceries\n- milk\n- [x] eggs").unwrap();
    assert_eq!(
        page_picture(&editor),
        "h1: Groceries\nbulleted_list: milk\ntodo[x]: eggs\n"
    );
    editor.undo().unwrap();
    assert_eq!(page_picture(&editor), "paragraph: \n");
}

#[test]
fn pasting_into_a_code_block_keeps_the_text_exactly() {
    let code = BlockKind::Code {
        language: String::new(),
    };
    let (mut editor, ids) = editor_with(&[(code, "")]);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    editor.paste_text("# not a heading\r\n**not bold**").unwrap();
    assert_eq!(page_picture(&editor), "code(): # not a heading⏎**not bold**\n");
    // Whole blocks selected: pasted after them, converted.
    editor.select_blocks(&[ids[0]]).unwrap();
    editor.paste_text("- item").unwrap();
    assert!(page_picture(&editor).ends_with("bulleted_list: item\n"));
}

// ---- A real backlog story ----

const STORY: &str = include_str!("story-E05-S01.md");

#[test]
fn a_backlog_story_pastes_as_the_blocks_it_describes() {
    let blocks = parse(STORY);
    insta::assert_snapshot!(picture(&blocks));
    // Through a real page: pasted on an empty line, it reads back as the same blocks.
    let (mut editor, ids) = editor_with(&[(BlockKind::Paragraph, "")]);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    editor.paste_text(STORY).unwrap();
    assert_eq!(page_picture(&editor), picture(&blocks));
}

#[test]
fn a_backlog_story_survives_copy_as_markdown_and_paste_again() {
    let blocks = parse(STORY);
    let again = parse(&Fragment::Blocks(blocks.clone()).to_markdown());
    assert_eq!(picture(&again), picture(&blocks));
}
