//! The index file's tables.
//!
//! Text lives in ordinary tables (`pages`, `blocks`); the FTS5 tables index them as "external
//! content" and are kept in step by triggers. That stores each text once, lets a page's rows be
//! found through a normal index on `page_id` (FTS5 can only scan for that), and still gives FTS5
//! its excerpts and highlighting.

/// Version of the layout below. Bump it on any change: an index with another version is deleted
/// and rebuilt, which is always safe because it holds nothing that isn't in the documents.
pub const INDEX_VERSION: u32 = 3;

/// `meta` key holding [`INDEX_VERSION`].
pub(crate) const VERSION_KEY: &str = "index_version";

/// `meta` key that is `1` once a full rebuild finished, `0` while one is due or running.
pub(crate) const COMPLETE_KEY: &str = "complete";

/// Creates every table. `unicode61 remove_diacritics 2` makes matching ignore case and accents
/// ("cafe" finds "café"); the prefix indexes make type-ahead ("pro" finds "project") fast, from
/// the first letter typed (version 3, E11-S07: a one-letter search had to scan every word).
pub(crate) const CREATE: &str = "
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE pages (
  id        INTEGER PRIMARY KEY,
  page_id   TEXT NOT NULL UNIQUE,
  title     TEXT NOT NULL,
  saved_at  INTEGER NOT NULL,  -- PageVersion: when the indexed blocks were saved (ranking)
  saved_seq INTEGER NOT NULL   -- PageVersion: which save they came from (change detection)
);

CREATE TABLE blocks (
  id       INTEGER PRIMARY KEY,
  page_id  TEXT NOT NULL,
  block_id TEXT NOT NULL,
  kind     TEXT NOT NULL,
  text     TEXT NOT NULL
);
CREATE INDEX blocks_by_page ON blocks(page_id);

CREATE VIRTUAL TABLE pages_fts USING fts5(
  title, content = 'pages', content_rowid = 'id',
  tokenize = 'unicode61 remove_diacritics 2', prefix = '1 2 3'
);

CREATE VIRTUAL TABLE blocks_fts USING fts5(
  text, content = 'blocks', content_rowid = 'id',
  tokenize = 'unicode61 remove_diacritics 2', prefix = '1 2 3'
);

CREATE TRIGGER pages_ai AFTER INSERT ON pages BEGIN
  INSERT INTO pages_fts(rowid, title) VALUES (new.id, new.title);
END;
CREATE TRIGGER pages_ad AFTER DELETE ON pages BEGIN
  INSERT INTO pages_fts(pages_fts, rowid, title) VALUES ('delete', old.id, old.title);
END;
CREATE TRIGGER pages_au AFTER UPDATE ON pages BEGIN
  INSERT INTO pages_fts(pages_fts, rowid, title) VALUES ('delete', old.id, old.title);
  INSERT INTO pages_fts(rowid, title) VALUES (new.id, new.title);
END;

CREATE TRIGGER blocks_ai AFTER INSERT ON blocks BEGIN
  INSERT INTO blocks_fts(rowid, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER blocks_ad AFTER DELETE ON blocks BEGIN
  INSERT INTO blocks_fts(blocks_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;
CREATE TRIGGER blocks_au AFTER UPDATE ON blocks BEGIN
  INSERT INTO blocks_fts(blocks_fts, rowid, text) VALUES ('delete', old.id, old.text);
  INSERT INTO blocks_fts(rowid, text) VALUES (new.id, new.text);
END;
";
