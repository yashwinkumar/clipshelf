//! History rows: insert, list and search, written as plain SQL over a `Connection`.
//!
//! Every value reaches SQLite as a bound parameter. SQL text is built only from
//! constants. User search input is escaped so `%`, `_` and `\` match literally.

use std::fmt;

use rusqlite::{params, Connection, OptionalExtension, Row, Statement};
use serde::Serialize;

/// Most entries kept. Older rows are removed on insert.
pub const HISTORY_CAP: i64 = 500;
/// Page size used when the caller gives none.
pub const DEFAULT_LIMIT: i64 = 100;
/// Longest search text used. Longer input is cut, not rejected.
pub const MAX_QUERY_CHARS: usize = 200;

const SELECT_COLUMNS: &str = "id, text, captured_at_ms, source_bundle_id, source_name";

/// Where a copy came from: the frontmost app when the change was read. Best effort.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Source {
    pub bundle_id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InsertOutcome {
    Stored(i64),
    /// Same text as the newest entry. Nothing was written.
    Duplicate,
}

#[derive(Clone, Serialize, PartialEq, Eq)]
pub struct Entry {
    pub id: i64,
    pub text: String,
    pub captured_at_ms: i64,
    pub source_bundle_id: Option<String>,
    pub source_name: Option<String>,
}

/// Redacted so a stray `{:?}` cannot print clipboard text.
impl fmt::Debug for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entry")
            .field("id", &self.id)
            .field("captured_at_ms", &self.captured_at_ms)
            .field(
                "text",
                &format_args!("<{} chars redacted>", self.text.chars().count()),
            )
            .field("source_name", &self.source_name)
            .finish()
    }
}

/// Insert one accepted text. Skips it when identical to the newest row, then
/// trims the table to `HISTORY_CAP`. Both steps run in one transaction.
pub fn insert(
    conn: &mut Connection,
    text: &str,
    captured_at_ms: i64,
    source: &Source,
) -> rusqlite::Result<InsertOutcome> {
    let tx = conn.transaction()?;
    let newest: Option<String> = tx
        .query_row(
            "SELECT text FROM entries ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if newest.as_deref() == Some(text) {
        // Returning drops the transaction, which rolls back. Nothing was written.
        return Ok(InsertOutcome::Duplicate);
    }
    tx.execute(
        "INSERT INTO entries (text, captured_at_ms, source_bundle_id, source_name)
         VALUES (?1, ?2, ?3, ?4)",
        params![text, captured_at_ms, source.bundle_id, source.name],
    )?;
    let id = tx.last_insert_rowid();
    tx.execute(
        "DELETE FROM entries
         WHERE id NOT IN (SELECT id FROM entries ORDER BY id DESC LIMIT ?1)",
        params![HISTORY_CAP],
    )?;
    tx.commit()?;
    Ok(InsertOutcome::Stored(id))
}

/// One entry by id.
pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Entry>> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM entries WHERE id = ?1");
    conn.query_row(&sql, params![id], map_row).optional()
}

/// Newest first.
pub fn list(conn: &Connection, limit: i64) -> rusqlite::Result<Vec<Entry>> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM entries ORDER BY id DESC LIMIT ?1");
    let mut stmt = conn.prepare(&sql)?;
    collect(&mut stmt, params![clamp_limit(limit)])
}

/// Case insensitive substring match on the text, newest first.
/// An empty query returns the same rows as `list`.
pub fn search(conn: &Connection, query: &str, limit: i64) -> rusqlite::Result<Vec<Entry>> {
    let needle: String = query.trim().chars().take(MAX_QUERY_CHARS).collect();
    if needle.is_empty() {
        return list(conn, limit);
    }
    let pattern = format!("%{}%", escape_like(&needle));
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM entries
         WHERE text LIKE ?1 ESCAPE '\\'
         ORDER BY id DESC LIMIT ?2"
    );
    let mut stmt = conn.prepare(&sql)?;
    collect(&mut stmt, params![pattern, clamp_limit(limit)])
}

/// Escape the LIKE wildcards and the escape character itself, so the input
/// matches literally. Pair with `ESCAPE '\'` in the query.
pub fn escape_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn clamp_limit(limit: i64) -> i64 {
    limit.clamp(1, HISTORY_CAP)
}

fn collect(
    stmt: &mut Statement<'_>,
    params: impl rusqlite::Params,
) -> rusqlite::Result<Vec<Entry>> {
    stmt.query_map(params, map_row)?.collect()
}

fn map_row(row: &Row<'_>) -> rusqlite::Result<Entry> {
    Ok(Entry {
        id: row.get(0)?,
        text: row.get(1)?,
        captured_at_ms: row.get(2)?,
        source_bundle_id: row.get(3)?,
        source_name: row.get(4)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{Store, StoreError};

    fn src() -> Source {
        Source {
            bundle_id: Some("com.example.editor".into()),
            name: Some("Editor".into()),
        }
    }

    fn store_with(texts: &[&str]) -> Store {
        let mut s = Store::open_in_memory().unwrap();
        for (i, t) in texts.iter().enumerate() {
            s.insert(t, 1_000 + i as i64, &src()).unwrap();
        }
        s
    }

    fn texts(rows: &[Entry]) -> Vec<&str> {
        rows.iter().map(|e| e.text.as_str()).collect()
    }

    #[test]
    fn list_is_newest_first() {
        let s = store_with(&["one", "two", "three"]);
        assert_eq!(texts(&s.list(10).unwrap()), vec!["three", "two", "one"]);
    }

    #[test]
    fn get_returns_entry_or_none() {
        let s = store_with(&["one", "two"]);
        let id = s.list(1).unwrap()[0].id;
        assert_eq!(s.get(id).unwrap().unwrap().text, "two");
        assert!(s.get(9_999).unwrap().is_none());
    }

    #[test]
    fn source_metadata_round_trips() {
        let s = store_with(&["hello"]);
        let row = &s.list(1).unwrap()[0];
        assert_eq!(row.source_bundle_id.as_deref(), Some("com.example.editor"));
        assert_eq!(row.source_name.as_deref(), Some("Editor"));
        assert_eq!(row.captured_at_ms, 1_000);
    }

    #[test]
    fn consecutive_duplicate_is_not_stored() {
        let mut s = Store::open_in_memory().unwrap();
        assert!(matches!(
            s.insert("same", 1, &src()).unwrap(),
            InsertOutcome::Stored(_)
        ));
        assert_eq!(
            s.insert("same", 2, &src()).unwrap(),
            InsertOutcome::Duplicate
        );
        assert_eq!(s.list(10).unwrap().len(), 1);
    }

    #[test]
    fn non_consecutive_repeat_is_stored() {
        let s = store_with(&["a", "b", "a"]);
        assert_eq!(texts(&s.list(10).unwrap()), vec!["a", "b", "a"]);
    }

    #[test]
    fn cap_keeps_500_and_drops_oldest() {
        let mut s = Store::open_in_memory().unwrap();
        for i in 0..505 {
            s.insert(&format!("entry {i}"), i, &src()).unwrap();
        }
        let rows = s.list(HISTORY_CAP).unwrap();
        assert_eq!(rows.len(), 500);
        assert_eq!(rows[0].text, "entry 504");
        assert_eq!(rows[499].text, "entry 5");
        assert!(!rows.iter().any(|e| e.text == "entry 0"));
    }

    #[test]
    fn search_is_substring_and_case_insensitive() {
        let s = store_with(&["staging endpoint", "Production URL", "stage two"]);
        assert_eq!(
            texts(&s.search("STAG", 10).unwrap()),
            vec!["stage two", "staging endpoint"]
        );
    }

    #[test]
    fn empty_search_returns_latest() {
        let s = store_with(&["a", "b"]);
        assert_eq!(texts(&s.search("   ", 10).unwrap()), vec!["b", "a"]);
    }

    #[test]
    fn percent_is_literal() {
        let s = store_with(&["100% done", "1000 done"]);
        assert_eq!(texts(&s.search("100%", 10).unwrap()), vec!["100% done"]);
    }

    #[test]
    fn underscore_is_literal() {
        let s = store_with(&["a_b", "axb"]);
        assert_eq!(texts(&s.search("a_b", 10).unwrap()), vec!["a_b"]);
    }

    #[test]
    fn backslash_is_literal() {
        let s = store_with(&[r"C:\temp", "C:temp"]);
        assert_eq!(texts(&s.search(r"\t", 10).unwrap()), vec![r"C:\temp"]);
    }

    #[test]
    fn sql_like_input_is_only_text() {
        let s = store_with(&["normal row"]);
        assert!(s.search("' OR 1=1 --", 10).unwrap().is_empty());
        assert!(s.search("%", 10).unwrap().is_empty());
        assert_eq!(s.list(10).unwrap().len(), 1, "table must be intact");
    }

    #[test]
    fn limit_is_clamped() {
        let s = store_with(&["a", "b"]);
        assert_eq!(s.list(0).unwrap().len(), 1);
        assert_eq!(s.list(10_000).unwrap().len(), 2);
    }

    #[test]
    fn escape_like_escapes_wildcards_and_backslash() {
        assert_eq!(escape_like(r"100%_\"), r"100\%\_\\");
    }

    #[test]
    fn entry_debug_never_prints_text() {
        let s = store_with(&["secret phrase"]);
        let shown = format!("{:?}", s.list(1).unwrap()[0]);
        assert!(!shown.contains("secret phrase"));
        assert!(shown.contains("redacted"));
    }

    #[test]
    fn store_error_display_has_no_detail() {
        let e = StoreError::Db(rusqlite::Error::InvalidQuery);
        assert_eq!(e.to_string(), "history database operation failed");
    }
}
