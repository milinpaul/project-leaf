//! Spike for E14-S01: `leaf-editor` driving a page in a browser through WebAssembly. Throwaway code.
//!
//! The browser keeps no document state of its own. JavaScript forwards what the person does (typed text, named
//! keys) and redraws from [`Spike::html`], built from `PageEditor`'s view rows: the same view models the desktop app
//! draws. Two tabs stay in step by exchanging Loro snapshots ([`Spike::snapshot`], [`Spike::merge`]).

use leaf_editor::{BlockStyle, Key, Motion, PageEditor};
use leaf_model::{Caret, MarkKind, PageDoc, TextSpan};
use wasm_bindgen::prelude::*;

/// One page being edited in the browser.
#[wasm_bindgen]
pub struct Spike {
    editor: PageEditor,
}

#[wasm_bindgen]
impl Spike {
    /// A new page with one empty paragraph and the caret in it.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<Spike, JsError> {
        let mut editor = PageEditor::new(PageDoc::new());
        let block = editor.ensure_block().map_err(err)?;
        editor.set_caret(Caret { block, at: 0 }).map_err(err)?;
        Ok(Spike { editor })
    }

    /// The page from another copy's snapshot, with the caret at its start.
    pub fn from_snapshot(bytes: &[u8]) -> Result<Spike, JsError> {
        let doc = PageDoc::from_bytes(bytes).map_err(err)?;
        let mut editor = PageEditor::new(doc);
        let block = editor.ensure_block().map_err(err)?;
        editor.set_caret(Caret { block, at: 0 }).map_err(err)?;
        Ok(Spike { editor })
    }

    /// Types text at the caret (what the hidden text input received). Markdown shortcuts apply as on desktop.
    pub fn type_text(&mut self, text: &str) -> Result<(), JsError> {
        self.editor.insert_text(text).map_err(err)?;
        Ok(())
    }

    /// A named key from the browser's `KeyboardEvent.key`, with Shift and Cmd/Ctrl. Returns whether it was used.
    pub fn key(&mut self, key: &str, shift: bool, command: bool) -> Result<bool, JsError> {
        let editor = &mut self.editor;
        match (key, shift, command) {
            ("Enter", _, false) => editor.press(Key::Enter).map(|_| ()),
            ("Backspace", _, _) => editor.press(Key::Backspace).map(|_| ()),
            ("Delete", _, _) => editor.press(Key::Delete).map(|_| ()),
            ("Tab", false, _) => editor.press(Key::Tab).map(|_| ()),
            ("Tab", true, _) => editor.press(Key::ShiftTab).map(|_| ()),
            ("ArrowLeft", _, false) => editor.move_caret(Motion::Left).map(|_| ()),
            ("ArrowRight", _, false) => editor.move_caret(Motion::Right).map(|_| ()),
            ("ArrowLeft", _, true) | ("Home", _, _) => editor.move_caret(Motion::LineStart).map(|_| ()),
            ("ArrowRight", _, true) | ("End", _, _) => editor.move_caret(Motion::LineEnd).map(|_| ()),
            ("z", false, true) => editor.undo().map(|_| ()),
            ("z", true, true) | ("y", false, true) => editor.redo().map(|_| ()),
            ("b", false, true) => editor.toggle_mark(MarkKind::Bold),
            ("i", false, true) => editor.toggle_mark(MarkKind::Italic),
            _ => return Ok(false),
        }
        .map_err(err)?;
        Ok(true)
    }

    /// The whole page as HTML, caret included: what the browser shows.
    pub fn html(&mut self) -> Result<String, JsError> {
        let caret = self.editor.caret().map_err(err)?;
        let rows = self.editor.view().map_err(err)?;
        let mut out = String::new();
        for row in rows {
            let (tag, class) = match &row.style {
                BlockStyle::Heading(1) => ("h1", ""),
                BlockStyle::Heading(2) => ("h2", ""),
                BlockStyle::Heading(_) => ("h3", ""),
                BlockStyle::Bulleted => ("div", "bullet"),
                BlockStyle::Numbered(_) => ("div", "numbered"),
                BlockStyle::Todo { checked: true } => ("div", "todo done"),
                BlockStyle::Todo { .. } => ("div", "todo"),
                BlockStyle::Quote => ("blockquote", ""),
                BlockStyle::Code { .. } => ("pre", ""),
                BlockStyle::Divider => ("hr", ""),
                _ => ("p", ""),
            };
            let number = match row.style {
                BlockStyle::Numbered(n) => format!(" data-n=\"{n}.\""),
                _ => String::new(),
            };
            let indent = row.depth * 24;
            out.push_str(&format!(
                "<{tag} class=\"row {class}\" style=\"margin-left:{indent}px\"{number}>"
            ));
            let at = caret.filter(|c| c.block == row.id).map(|c| c.at);
            out.push_str(&spans_html(row.spans.as_deref().unwrap_or(&[]), at));
            out.push_str(&format!("</{tag}>"));
        }
        Ok(out)
    }

    /// This copy's whole document, to send to another copy.
    pub fn snapshot(&self) -> Result<Vec<u8>, JsError> {
        self.editor.doc().to_bytes().map_err(err)
    }

    /// Merges another copy's document into this one, keeping the caret where it was.
    ///
    /// `PageEditor` has no public way to take in remote changes yet, so this rebuilds it around the merged
    /// document: a gap the spike found (undo history is lost on each merge).
    pub fn merge(&mut self, bytes: &[u8]) -> Result<(), JsError> {
        let caret = self.editor.caret().map_err(err)?;
        let mut doc = PageDoc::from_bytes(&self.snapshot()?).map_err(err)?;
        doc.import_changes(bytes).map_err(err)?;
        self.editor = PageEditor::new(doc);
        if let Some(caret) = caret {
            // The caret's block may have gone in the other copy; then it goes to the first block.
            if self.editor.set_caret(caret).is_err() {
                let block = self.editor.ensure_block().map_err(err)?;
                self.editor.set_caret(Caret { block, at: 0 }).map_err(err)?;
            }
        }
        Ok(())
    }

    /// The page as plain text, one line per block, to compare copies.
    pub fn text(&self) -> Result<String, JsError> {
        let rows = self.editor.rows().map_err(err)?;
        Ok(rows
            .iter()
            .filter_map(|r| r.text.clone())
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

/// A block's runs as HTML, with the caret drawn before the character at `caret` (a char index).
fn spans_html(spans: &[TextSpan], caret: Option<usize>) -> String {
    let mut out = String::new();
    let mut at = 0;
    let mut drawn = caret.is_none();
    for span in spans {
        let (open, close) = marks(span);
        out.push_str(&open);
        for ch in span.text.chars() {
            if !drawn && caret == Some(at) {
                out.push_str("<span class=\"caret\"></span>");
                drawn = true;
            }
            push_escaped(&mut out, ch);
            at += 1;
        }
        out.push_str(&close);
    }
    if !drawn {
        out.push_str("<span class=\"caret\"></span>");
    }
    if spans.is_empty() || at == 0 {
        out.push_str("<br>");
    }
    out
}

fn marks(span: &TextSpan) -> (String, String) {
    let m = &span.marks;
    let mut open = String::new();
    let mut close = String::new();
    for (on, tag) in [
        (m.bold, "strong"),
        (m.italic, "em"),
        (m.strike, "s"),
        (m.code, "code"),
    ] {
        if on {
            open.push_str(&format!("<{tag}>"));
            close.insert_str(0, &format!("</{tag}>"));
        }
    }
    (open, close)
}

fn push_escaped(out: &mut String, ch: char) {
    match ch {
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        '&' => out.push_str("&amp;"),
        '\n' => out.push_str("<br>"),
        _ => out.push(ch),
    }
}

fn err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}
