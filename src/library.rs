use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};

pub struct Library {
    conn: Connection,
    /// Where `move_to_trash` parks deleted files. A field rather than a direct
    /// `glib::user_data_dir()` call so tests can point it at a temp dir instead
    /// of the real data dir.
    trash_dir: PathBuf,
    /// Derives the author tags (see `authors.rs`) each time a document is
    /// scanned, opened or saved.
    authors: AuthorSync,
}

/// The bibliography fallback for documents that don't name one themselves,
/// plus the parse cache that keeps a scan from re-reading it per document.
#[derive(Default)]
struct AuthorSync {
    bib_source: Option<PathBuf>,
    index: crate::authors::AuthorIndex,
}

fn default_trash_dir() -> PathBuf {
    crate::config::zerkalo_data_dir().join("trash")
}

fn labels_migrated(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT value FROM meta WHERE key = 'labels_migrated'",
        [],
        |r| r.get::<_, String>(0),
    )
    .map(|v| v == "1")
    .unwrap_or(false)
}

/// There is an old-style library whose tags and categories haven't become
/// labels yet.
fn labels_migration_pending(conn: &Connection) -> bool {
    let has_tags = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'tags'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;
    has_tags && !labels_migrated(conn)
}

/// `move_to_trash` must never mark a document deleted in the database unless
/// the file actually landed in the trash directory — a filesystem failure
/// (read-only fs, full disk, permissions) has to abort the whole operation
/// rather than leave the database and the filesystem disagreeing about
/// where the document is.
#[derive(Debug, thiserror::Error)]
pub enum TrashError {
    #[error("could not move file to trash: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Db(#[from] rusqlite::Error),
}

impl TrashError {
    pub fn user_message(&self) -> String {
        match self {
            TrashError::Io(e) => crate::error::io_reason(e),
            TrashError::Db(_) => "the library's list of documents couldn't be updated".into(),
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)] // mirrors the documents table; not every column is read yet
pub struct Document {
    pub id: i64,
    pub path: PathBuf,
    pub title: String,
    pub archived: bool,
    pub pinned: bool,
    pub notes: Option<String>,
    pub created_at: String,
    pub modified_at: String,
    pub last_opened_at: Option<String>,
    /// Prose word count, kept up to date by the index so the list never has to
    /// read files to draw itself. `None` until the document has been indexed.
    pub words: Option<usize>,
}

/// One document's own data, as the folder export writes it.
#[derive(Clone, Debug, PartialEq)]
pub struct DocState {
    pub id: i64,
    pub path: PathBuf,
    /// Only when the user renamed it in the Library; otherwise the title comes
    /// from the file and there is nothing to save.
    pub title: Option<String>,
    pub pinned: bool,
    pub archived: bool,
    pub notes: Option<String>,
    pub labels: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectState {
    pub name: String,
    pub root: Option<PathBuf>,
    /// In the project's own order.
    pub documents: Vec<PathBuf>,
}

/// Everything the library knows that isn't recoverable from the documents
/// themselves — what the folder export writes out.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub docs: Vec<DocState>,
    pub labels: Vec<Label>,
    pub projects: Vec<ProjectState>,
}

/// How many documents the Recent view lists at most.
const RECENT_LIMIT: u32 = 30;

/// Everything the sidebar counts, from `Library::counts`.
#[derive(Debug, Default)]
pub struct Counts {
    pub all: i64,
    pub archive: i64,
    pub trash: i64,
    pub recent: i64,
    pub unlabelled: i64,
    pub projects: std::collections::HashMap<i64, i64>,
    pub labels: std::collections::HashMap<i64, i64>,
    pub authors: std::collections::HashMap<i64, i64>,
}

/// A word a document can carry — what tags and categories used to be, as one
/// thing. Flat: nesting is written into the name (`Liturgy › Advent`).
#[derive(Clone, Debug)]
pub struct Label {
    pub id: i64,
    pub name: String,
    /// `None` until a colour is chosen — callers substitute a per-name palette
    /// colour so distinct labels stay distinct.
    pub color_hex: Option<String>,
}

/// A cited author's tag, e.g. `Butler, J.` — derived, never typed in.
#[derive(Clone, Debug)]
pub struct Author {
    pub id: i64,
    pub name: String,
}

#[derive(Clone, Debug)]
#[allow(dead_code)] // mirrors the projects table; not every column is read yet
pub struct Project {
    pub id: i64,
    pub name: String,
    pub root_doc_id: Option<i64>,
    pub created_at: String,
}
#[derive(Clone, Debug, PartialEq)]
pub enum LibraryFilter {
    All,
    Project(i64),
    Label(i64),
    /// Everything citing one author — see `authors.rs`.
    Author(i64),
    Archive,
    Recent,
    /// Documents with no label yet.
    Unlabelled,
    Trash,
    /// Every document that isn't in the Trash, archived or not — what a
    /// search looks through unless it's been narrowed to the current view.
    Everywhere,
}

/// The parts of `documents()`'s query that vary by filter: which columns to
/// select, what to join, the conditions before the search clause, the ordering,
/// and an optional leading parameter.
struct FilterSpec {
    /// Column prefix for the search clause and sort: `""` or `"d."`.
    prefix: &'static str,
    select: String,
    from: String,
    conditions: String,
    /// `None` means `pinned DESC` followed by the caller's sort. `Some`
    /// replaces the ordering entirely — project position, recency, trash order.
    order_override: Option<&'static str>,
    /// Caps the result, applied after ordering — the Recent list's 30.
    limit: Option<u32>,
    param: Option<rusqlite::types::Value>,
}

impl FilterSpec {
    fn limit_clause(&self) -> String {
        self.limit
            .map(|n| format!(" LIMIT {n}"))
            .unwrap_or_default()
    }

    fn order(&self, sort: &SortOrder) -> String {
        match self.order_override {
            Some(o) => o.to_string(),
            None => format!("{}pinned DESC, {}", self.prefix, sort.clause(self.prefix)),
        }
    }
}

impl LibraryFilter {
    fn query(self) -> FilterSpec {
        let plain = |conditions: &str| FilterSpec {
            prefix: "",
            select: DOC_COLS.to_string(),
            from: "documents".to_string(),
            conditions: conditions.to_string(),
            order_override: None,
            limit: None,
            param: None,
        };
        match self {
            LibraryFilter::All => plain("archived = 0 AND deleted = 0"),
            LibraryFilter::Archive => plain("archived = 1 AND deleted = 0"),
            LibraryFilter::Everywhere => plain("deleted = 0"),
            LibraryFilter::Unlabelled => plain(
                "archived = 0 AND deleted = 0 \
                 AND id NOT IN (SELECT DISTINCT doc_id FROM doc_labels)",
            ),
            LibraryFilter::Recent => FilterSpec {
                order_override: Some("pinned DESC, last_opened_at DESC"),
                limit: Some(RECENT_LIMIT),
                ..plain("last_opened_at IS NOT NULL AND archived = 0 AND deleted = 0")
            },
            LibraryFilter::Trash => FilterSpec {
                order_override: Some("modified_at DESC"),
                ..plain("deleted = 1")
            },
            LibraryFilter::Project(pid) => FilterSpec {
                prefix: "d.",
                select: doc_cols_prefixed("d"),
                from: "documents d JOIN project_docs pd ON pd.doc_id = d.id".to_string(),
                conditions: "pd.project_id = ?1 AND d.deleted = 0".to_string(),
                order_override: Some("d.pinned DESC, pd.position, d.title"),
                limit: None,
                param: Some(pid.into()),
            },
            LibraryFilter::Author(aid) => FilterSpec {
                prefix: "d.",
                select: doc_cols_prefixed("d"),
                from: "documents d JOIN doc_authors da ON da.doc_id = d.id".to_string(),
                conditions: "da.author_id = ?1 AND d.archived = 0 AND d.deleted = 0".to_string(),
                order_override: None,
                limit: None,
                param: Some(aid.into()),
            },
            LibraryFilter::Label(lid) => FilterSpec {
                prefix: "d.",
                select: doc_cols_prefixed("d"),
                from: "documents d JOIN doc_labels dl ON dl.doc_id = d.id".to_string(),
                conditions: "dl.label_id = ?1 AND d.archived = 0 AND d.deleted = 0".to_string(),
                order_override: None,
                limit: None,
                param: Some(lid.into()),
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SortOrder {
    Modified,
    Created,
    Opened,
    Title,
}

impl SortOrder {
    fn clause(&self, prefix: &str) -> String {
        match self {
            SortOrder::Modified => format!("{prefix}modified_at DESC"),
            SortOrder::Created => format!("{prefix}created_at DESC"),
            SortOrder::Opened => format!("{prefix}last_opened_at DESC NULLS LAST"),
            SortOrder::Title => format!("{prefix}title COLLATE NOCASE ASC"),
        }
    }
}

const DOC_COLS: &str =
    "id, path, title, archived, pinned, notes, created_at, modified_at, last_opened_at, words";

/// Suffix for `path = ?N` comparisons — case-insensitive on Windows, where
/// the same file reached via two differently-cased paths would otherwise
/// upsert as two distinct library rows (Windows filesystems are normally
/// case-insensitive, so this doesn't change what file the path resolves
/// to). No-op on other platforms: the stored path's real case is never
/// touched either way, only how it's matched.
#[cfg(windows)]
const PATH_COLLATE: &str = " COLLATE NOCASE";
#[cfg(not(windows))]
const PATH_COLLATE: &str = "";

fn doc_cols_prefixed(prefix: &str) -> String {
    DOC_COLS
        .split(", ")
        .map(|c| format!("{prefix}.{c}"))
        .collect::<Vec<_>>()
        .join(", ")
}

impl Library {
    pub fn open() -> SqlResult<Self> {
        let dir = crate::config::zerkalo_data_dir();
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("library.sqlite");
        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        // Tags and categories are about to be folded into labels. Keep a copy
        // of the library as it was, once, so nothing is lost if that goes
        // wrong or the change is unwelcome.
        if labels_migration_pending(&conn) {
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").ok();
            let backup = dir.join("library.before-labels.sqlite");
            if !backup.exists() {
                std::fs::copy(&path, backup).ok();
            }
        }
        let lib = Self {
            conn,
            trash_dir: default_trash_dir(),
            authors: AuthorSync::default(),
        };
        lib.migrate()?;
        Ok(lib)
    }

    pub fn open_in_memory() -> Self {
        Self::in_memory_with_trash_dir(default_trash_dir())
    }

    fn in_memory_with_trash_dir(trash_dir: PathBuf) -> Self {
        let conn = Connection::open_in_memory().expect("in-memory DB");
        conn.execute_batch("PRAGMA foreign_keys = ON;").ok();
        let lib = Self {
            conn,
            trash_dir,
            authors: AuthorSync::default(),
        };
        lib.migrate().ok();
        lib
    }

    fn migrate(&self) -> SqlResult<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS documents (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                -- `category` is vestigial: superseded by the many-to-many
                -- `doc_categories` table below (a document can now hold more
                -- than one category, the same way `doc_tags` already works
                -- for tags). Left in place rather than rebuilt out, since
                -- `documents` has several other tables' foreign keys
                -- pointing at it. Nothing reads or writes this column
                -- anymore except the one-time backfill into
                -- `doc_categories` further down in this function.
                category TEXT,
                archived INTEGER NOT NULL DEFAULT 0,
                notes TEXT,
                created_at TEXT NOT NULL,
                modified_at TEXT NOT NULL,
                last_opened_at TEXT
            );
            CREATE TABLE IF NOT EXISTS projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                root_doc_id INTEGER REFERENCES documents(id) ON DELETE SET NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS project_docs (
                project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                doc_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                position INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (project_id, doc_id)
            );
            CREATE TABLE IF NOT EXISTS tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                color_hex TEXT NOT NULL DEFAULT '#3584e4'
            );
            CREATE TABLE IF NOT EXISTS doc_tags (
                doc_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
                PRIMARY KEY (doc_id, tag_id)
            );",
        )?;
        self.conn
            .execute_batch("ALTER TABLE documents ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;")
            .ok();
        self.conn
            .execute_batch("ALTER TABLE documents ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0;")
            .ok();
        self.conn
            .execute_batch("ALTER TABLE documents ADD COLUMN trash_path TEXT;")
            .ok();
        // 1 once the user has renamed the document in the Library: from then on
        // a rescan must not overwrite the title with the one in the file.
        let had_custom_titles = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('documents') WHERE name = 'title_custom'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;
        self.conn
            .execute_batch(
                "ALTER TABLE documents ADD COLUMN title_custom INTEGER NOT NULL DEFAULT 0;",
            )
            .ok();
        if !had_custom_titles {
            self.protect_existing_titles();
        }
        // The text index: prose word count and a full-text table over title,
        // notes, file name and body. `indexed_mtime`/`indexed_len` say which
        // version of the file the index was built from, so a rescan only reads
        // files that have changed.
        for col in [
            "words INTEGER",
            "indexed_mtime INTEGER",
            "indexed_len INTEGER",
        ] {
            self.conn
                .execute_batch(&format!("ALTER TABLE documents ADD COLUMN {col};"))
                .ok();
        }
        self.conn
            .execute_batch(
                "CREATE VIRTUAL TABLE IF NOT EXISTS doc_text USING fts5(
                    title, notes, path, body,
                    tokenize = 'unicode61 remove_diacritics 2'
                );
                CREATE TRIGGER IF NOT EXISTS documents_forget_text
                AFTER DELETE ON documents
                BEGIN
                    DELETE FROM doc_text WHERE rowid = old.id;
                END;",
            )
            .ok();
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS authors (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL UNIQUE COLLATE NOCASE
                );
                CREATE TABLE IF NOT EXISTS doc_authors (
                    doc_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                    author_id INTEGER NOT NULL REFERENCES authors(id) ON DELETE CASCADE,
                    PRIMARY KEY (doc_id, author_id)
                );
                CREATE INDEX IF NOT EXISTS idx_doc_authors_author ON doc_authors(author_id);",
            )
            .ok();
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS categories (
                    name TEXT NOT NULL PRIMARY KEY,
                    color_hex TEXT,
                    parent TEXT REFERENCES categories(name)
                );",
            )
            .ok();
        self.conn
            .execute_batch(
                "ALTER TABLE categories ADD COLUMN parent TEXT REFERENCES categories(name);",
            )
            .ok();
        self.migrate_category_colors_to_nullable();
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS doc_categories (
                    doc_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                    category TEXT NOT NULL REFERENCES categories(name) ON DELETE CASCADE,
                    PRIMARY KEY (doc_id, category)
                );",
            )
            .ok();
        self.conn
            .execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_doc_category ON documents(category);
             CREATE INDEX IF NOT EXISTS idx_doc_archived ON documents(archived, deleted);
             CREATE INDEX IF NOT EXISTS idx_doc_last_opened ON documents(last_opened_at);
             CREATE INDEX IF NOT EXISTS idx_doc_modified ON documents(modified_at);
             CREATE INDEX IF NOT EXISTS idx_doc_tags_tag ON doc_tags(tag_id);
             CREATE INDEX IF NOT EXISTS idx_doc_tags_doc ON doc_tags(doc_id);
             CREATE INDEX IF NOT EXISTS idx_doc_categories_cat ON doc_categories(category);
             CREATE INDEX IF NOT EXISTS idx_doc_categories_doc ON doc_categories(doc_id);",
            )
            .ok();
        self.conn
            .execute_batch(
                "INSERT OR IGNORE INTO categories (name)
             SELECT DISTINCT category FROM documents WHERE category IS NOT NULL;",
            )
            .ok();
        self.conn
            .execute_batch(
                "INSERT OR IGNORE INTO doc_categories (doc_id, category)
             SELECT id, category FROM documents WHERE category IS NOT NULL;",
            )
            .ok();
        // Labels, which replace tags and categories. The old tables stay where
        // they are, untouched and unread, so the migration can be checked
        // against them and a downgrade still finds its data.
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS labels (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL UNIQUE COLLATE NOCASE,
                 color_hex TEXT
             );
             CREATE TABLE IF NOT EXISTS doc_labels (
                 doc_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                 label_id INTEGER NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
                 PRIMARY KEY (doc_id, label_id)
             );
             CREATE INDEX IF NOT EXISTS idx_doc_labels_label ON doc_labels(label_id);
             -- What the folder export last wrote to each file, so it can tell a
             -- file it wrote (and may update) from one that changed elsewhere.
             CREATE TABLE IF NOT EXISTS export_files (
                 path TEXT PRIMARY KEY,
                 content TEXT NOT NULL
             );",
        )?;
        self.migrate_to_labels()?;
        Ok(())
    }

    /// `categories.color_hex` was originally `NOT NULL DEFAULT '#3584e4'`, which
    /// meant a category had a colour the instant it existed — so
    /// `get_category_color` could never report "none chosen", the per-name
    /// palette fallback was unreachable, and every category the user hadn't
    /// explicitly coloured rendered the same blue. SQLite can't drop NOT NULL in
    /// place, so rebuild the table with a nullable column. The old default is
    /// treated as unset: it was applied automatically, never chosen, and a
    /// category that had it already looked exactly like an uncoloured one.
    fn migrate_category_colors_to_nullable(&self) {
        let color_is_not_null = self
            .conn
            .query_row(
                "SELECT \"notnull\" FROM pragma_table_info('categories') WHERE name = 'color_hex'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0)
            == 1;
        if !color_is_not_null {
            return;
        }
        // foreign_keys can't be toggled inside a transaction, hence the split.
        self.conn.execute_batch("PRAGMA foreign_keys = OFF;").ok();
        let rebuilt = self.conn.execute_batch(
            "BEGIN;
             CREATE TABLE categories_migrated (
                 name TEXT NOT NULL PRIMARY KEY,
                 color_hex TEXT,
                 parent TEXT REFERENCES categories(name)
             );
             INSERT INTO categories_migrated (name, color_hex, parent)
                 SELECT name, NULLIF(color_hex, '#3584e4'), parent FROM categories;
             DROP TABLE categories;
             ALTER TABLE categories_migrated RENAME TO categories;
             COMMIT;",
        );
        if rebuilt.is_err() {
            self.conn.execute_batch("ROLLBACK;").ok();
        }
        self.conn.execute_batch("PRAGMA foreign_keys = ON;").ok();
    }

    /// Libraries from before titles could be protected have no way to say "this
    /// title was chosen by the user". The one sign is a title that differs from
    /// what the file itself would give — so, once, as the column is added, every
    /// such title is marked as chosen. Without this the first rescan would
    /// replace it with the file's own.
    fn protect_existing_titles(&self) {
        let rows: Vec<(i64, String, String)> = {
            let Ok(mut stmt) = self.conn.prepare("SELECT id, path, title FROM documents") else {
                return;
            };
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .into_iter()
                .flatten()
                .filter_map(|r| r.ok())
                .collect()
        };
        for (id, path, title) in rows {
            let p = Path::new(&path);
            let derived = extract_typst_title(p).unwrap_or_else(|| {
                p.file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone())
            });
            if title != derived {
                self.conn
                    .execute(
                        "UPDATE documents SET title_custom = 1 WHERE id = ?1",
                        params![id],
                    )
                    .ok();
            }
        }
    }

    /// Folds the old tags and categories into labels, once.
    ///
    /// - A tag becomes a label of the same name and carries the same documents.
    /// - A category becomes a label too; a nested one is named for its whole
    ///   path (`Liturgy › Advent`). A parent that only ever grouped children and
    ///   held no documents of its own is dropped — its name lives on in theirs.
    /// - A tag and a category with the same name become one label.
    /// - The old default blue (applied automatically, never chosen) counts as
    ///   "no colour", so migrated labels get distinct palette colours instead of
    ///   all being blue.
    fn migrate_to_labels(&self) -> SqlResult<()> {
        if labels_migrated(&self.conn) {
            return Ok(());
        }
        let name_expr = "CASE WHEN c.parent IS NULL THEN c.name \
                         ELSE c.parent || ' \u{203a} ' || c.name END";
        let keeps = "NOT (EXISTS (SELECT 1 FROM categories k WHERE k.parent = c.name) \
                     AND NOT EXISTS (SELECT 1 FROM doc_categories x WHERE x.category = c.name))";
        let tx = self.conn.unchecked_transaction()?;
        tx.execute_batch(&format!(
            "INSERT OR IGNORE INTO labels (name, color_hex)
                 SELECT name, NULLIF(color_hex, '#3584e4') FROM tags;
             INSERT OR IGNORE INTO doc_labels (doc_id, label_id)
                 SELECT dt.doc_id, l.id FROM doc_tags dt
                 JOIN tags t ON t.id = dt.tag_id
                 JOIN labels l ON l.name = t.name;
             INSERT OR IGNORE INTO labels (name, color_hex)
                 SELECT {name_expr}, c.color_hex FROM categories c WHERE {keeps};
             UPDATE labels SET color_hex = (
                     SELECT c.color_hex FROM categories c
                     WHERE {name_expr} = labels.name AND c.color_hex IS NOT NULL
                       AND c.color_hex <> '#3584e4' LIMIT 1)
                 WHERE color_hex IS NULL AND EXISTS (
                     SELECT 1 FROM categories c
                     WHERE {name_expr} = labels.name AND c.color_hex IS NOT NULL
                       AND c.color_hex <> '#3584e4');
             INSERT OR IGNORE INTO doc_labels (doc_id, label_id)
                 SELECT dc.doc_id, l.id FROM doc_categories dc
                 JOIN categories c ON c.name = dc.category
                 JOIN labels l ON l.name = {name_expr};
             INSERT OR REPLACE INTO meta (key, value) VALUES ('labels_migrated', '1');"
        ))?;
        tx.commit()
    }

    pub fn upsert_document(&mut self, path: &Path) -> SqlResult<i64> {
        let path_str = path.to_string_lossy().to_string();
        let title = extract_typst_title(path).unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| path_str.clone())
        });
        let now = Utc::now().to_rfc3339();
        let meta = std::fs::metadata(path).ok();
        let fs_modified = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                let dt: chrono::DateTime<Utc> = t.into();
                dt.to_rfc3339()
            })
            .unwrap_or_else(|| now.clone());
        let fs_created = meta
            .as_ref()
            .and_then(|m| m.created().ok())
            .map(|t| {
                let dt: chrono::DateTime<Utc> = t.into();
                dt.to_rfc3339()
            })
            .unwrap_or_else(|| fs_modified.clone());

        let existing: Option<(i64, String, bool)> = self
            .conn
            .query_row(
                &format!(
                    "SELECT id, title, title_custom FROM documents WHERE path = ?1{PATH_COLLATE}"
                ),
                params![path_str],
                |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? != 0)),
            )
            .optional()?;
        // Whether the searchable metadata (title, notes, file name) may have
        // changed — the body is tracked separately by the file's stamp.
        let metadata_changed = match &existing {
            None => true,
            Some((_, old, custom)) => !custom && *old != title,
        };

        let id = if let Some((id, _, _)) = existing {
            self.conn.execute(
                "UPDATE documents SET modified_at = ?1,
                     title = CASE WHEN title_custom = 1 THEN title ELSE ?2 END
                 WHERE id = ?3",
                params![fs_modified, title, id],
            )?;
            id
        } else {
            self.conn.execute(
                "INSERT INTO documents (path, title, created_at, modified_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![path_str, title, fs_created, fs_modified],
            )?;
            self.conn.last_insert_rowid()
        };
        // Best effort: a bibliography that won't parse, or a file that can't be
        // read, must not stop the document being listed.
        self.refresh_authors(id, path).ok();
        self.index_document(id, path, metadata_changed).ok();
        Ok(id)
    }

    /// Brings the word count and full-text row for a document up to date. The
    /// file is only read when its size or modification time differ from what
    /// the index was built from, so rescanning an unchanged library costs a
    /// `stat` per document.
    fn index_document(&mut self, id: i64, path: &Path, metadata_changed: bool) -> SqlResult<()> {
        let stamp = file_stamp(path);
        let stored: (Option<i64>, Option<i64>) = self.conn.query_row(
            "SELECT indexed_mtime, indexed_len FROM documents WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let mut body = None;
        if let Some((mtime, len)) = stamp {
            if stored != (Some(mtime), Some(len)) {
                if let Ok(content) = std::fs::read_to_string(path) {
                    let words = prose_words(&content);
                    self.conn.execute(
                        "UPDATE documents SET words = ?1, indexed_mtime = ?2, indexed_len = ?3
                         WHERE id = ?4",
                        params![words.len() as i64, mtime, len, id],
                    )?;
                    body = Some(words.join(" "));
                }
            }
        }
        if body.is_some() || metadata_changed {
            self.reindex_row(id, body)?;
        }
        Ok(())
    }

    /// Rewrites a document's full-text row from its current title, notes and
    /// file name. `body` is the new prose if it has just been read; otherwise
    /// the body already in the index is kept.
    fn reindex_row(&self, id: i64, body: Option<String>) -> SqlResult<()> {
        let body = match body {
            Some(b) => b,
            None => self
                .conn
                .query_row(
                    "SELECT body FROM doc_text WHERE rowid = ?1",
                    params![id],
                    |r| r.get(0),
                )
                .optional()?
                .unwrap_or_default(),
        };
        let row: Option<(String, String, String)> = self
            .conn
            .query_row(
                "SELECT title, COALESCE(notes, ''), path FROM documents WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((title, notes, path)) = row else {
            return Ok(());
        };
        // The file name, not the folder: every document shares its folders'
        // names, so indexing them would make searching "Documents" match all.
        let file_name = Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.conn
            .execute("DELETE FROM doc_text WHERE rowid = ?1", params![id])?;
        self.conn.execute(
            "INSERT INTO doc_text (rowid, title, notes, path, body) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, title, notes, file_name, body],
        )?;
        Ok(())
    }

    /// A short excerpt of each document's text around what `search` matched,
    /// with the matches wrapped in `MARK_START`/`MARK_END`. Only documents
    /// whose *body* matches appear — a hit in the title or a tag needs no
    /// excerpt.
    pub fn snippets(&self, search: &str) -> std::collections::HashMap<i64, String> {
        let Some(q) = fts_query(search) else {
            return Default::default();
        };
        let run = || -> SqlResult<std::collections::HashMap<i64, String>> {
            let mut stmt = self.conn.prepare(
                "SELECT rowid, snippet(doc_text, 3, ?2, ?3, '…', 16)
                 FROM doc_text WHERE doc_text MATCH ?1",
            )?;
            let rows = stmt.query_map(
                params![format!("body : ({q})"), MARK_START, MARK_END],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
            )?;
            rows.collect()
        };
        run().unwrap_or_default()
    }

    /// Drops index rows for documents that no longer exist.
    pub fn prune_index(&mut self) {
        self.conn
            .execute(
                "DELETE FROM doc_text WHERE rowid NOT IN (SELECT id FROM documents)",
                [],
            )
            .ok();
    }

    /// Sets the bibliography used for documents that don't name their own,
    /// returning whether it changed (the caller then wants `resync_authors`).
    pub fn set_bibliography(&mut self, source: Option<PathBuf>) -> bool {
        if self.authors.bib_source == source {
            return false;
        }
        self.authors.bib_source = source;
        true
    }

    /// Replaces the document's author tags with what it cites now, and drops
    /// any author no document cites any more.
    fn refresh_authors(&mut self, doc_id: i64, path: &Path) -> SqlResult<()> {
        let tags = self
            .authors
            .index
            .tags_for_document(path, self.authors.bib_source.as_deref());
        self.set_doc_authors(doc_id, &tags)
    }

    /// Recomputes every document's authors — for when the bibliography itself
    /// changed rather than any one document.
    pub fn resync_authors(&mut self) {
        let docs: Vec<(i64, String)> = {
            let Ok(mut stmt) = self
                .conn
                .prepare("SELECT id, path FROM documents WHERE deleted = 0")
            else {
                return;
            };
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .into_iter()
                .flatten()
                .filter_map(|r| r.ok())
                .collect()
        };
        for (id, path) in docs {
            self.refresh_authors(id, Path::new(&path)).ok();
        }
    }

    fn set_doc_authors(&mut self, doc_id: i64, names: &[String]) -> SqlResult<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM doc_authors WHERE doc_id = ?1", params![doc_id])?;
        for name in names {
            tx.execute(
                "INSERT OR IGNORE INTO authors (name) VALUES (?1)",
                params![name],
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO doc_authors (doc_id, author_id)
                 SELECT ?1, id FROM authors WHERE name = ?2",
                params![doc_id, name],
            )?;
        }
        tx.execute(
            "DELETE FROM authors WHERE id NOT IN (SELECT author_id FROM doc_authors)",
            [],
        )?;
        tx.commit()
    }

    /// Authors cited by at least one live document, with how many, by name.
    pub fn all_authors_with_counts(&self) -> SqlResult<Vec<(Author, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.id, a.name, COUNT(*) FROM authors a
             JOIN doc_authors da ON da.author_id = a.id
             JOIN documents d ON d.id = da.doc_id
             WHERE d.archived = 0 AND d.deleted = 0
             GROUP BY a.id ORDER BY a.name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                Author {
                    id: r.get(0)?,
                    name: r.get(1)?,
                },
                r.get(2)?,
            ))
        })?;
        rows.collect()
    }

    /// Every document's authors in one query, for the list's tooltips.
    pub fn authors_by_doc(&self) -> SqlResult<std::collections::HashMap<i64, Vec<String>>> {
        let mut stmt = self.conn.prepare(
            "SELECT da.doc_id, a.name FROM doc_authors da
             JOIN authors a ON a.id = da.author_id
             ORDER BY a.name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        let mut map: std::collections::HashMap<i64, Vec<String>> = Default::default();
        for r in rows {
            let (doc, name) = r?;
            map.entry(doc).or_default().push(name);
        }
        Ok(map)
    }

    pub fn touch_opened(&mut self, path: &Path) -> SqlResult<()> {
        self.upsert_document(path)?;
        let path_str = path.to_string_lossy().to_string();
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            &format!("UPDATE documents SET last_opened_at = ?1 WHERE path = ?2{PATH_COLLATE}"),
            params![now, path_str],
        )?;
        Ok(())
    }

    pub fn touch_saved(&mut self, path: &Path) -> SqlResult<()> {
        self.upsert_document(path)?;
        let path_str = path.to_string_lossy().to_string();
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            &format!("UPDATE documents SET modified_at = ?1 WHERE path = ?2{PATH_COLLATE}"),
            params![now, path_str],
        )?;
        Ok(())
    }

    /// Points an existing document at a new on-disk path — used when a
    /// document is moved into the Zerkalo folder (see the Library's "Move
    /// into Zerkalo Folder…" action). Callers are responsible for actually
    /// moving the file (and any sidecars) first; this only updates the row.
    pub fn update_path(&mut self, doc_id: i64, new_path: &Path) -> SqlResult<()> {
        let path_str = new_path.to_string_lossy().to_string();
        self.conn.execute(
            "UPDATE documents SET path = ?1 WHERE id = ?2",
            params![path_str, doc_id],
        )?;
        self.reindex_row(doc_id, None)?;
        self.index_document(doc_id, new_path, false)?;
        Ok(())
    }

    /// Correct created_at for existing documents using filesystem creation time.
    /// Runs once at startup in the background thread after initial scan.
    pub fn fix_created_dates_from_fs(&mut self) {
        let paths: Vec<(i64, String)> = {
            let mut stmt = match self.conn.prepare("SELECT id, path FROM documents") {
                Ok(s) => s,
                Err(_) => return,
            };
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .into_iter()
                .flatten()
                .filter_map(|r| r.ok())
                .collect()
        };
        for (id, path_str) in paths {
            let path = std::path::Path::new(&path_str);
            if let Ok(meta) = std::fs::metadata(path) {
                let fs_created = meta
                    .created()
                    .or_else(|_| meta.modified())
                    .map(|t| {
                        let dt: chrono::DateTime<Utc> = t.into();
                        dt.to_rfc3339()
                    })
                    .ok();
                if let Some(created) = fs_created {
                    self.conn
                        .execute(
                            "UPDATE documents SET created_at = ?1 WHERE id = ?2",
                            rusqlite::params![created, id],
                        )
                        .ok();
                }
            }
        }
    }

    pub fn documents(
        &self,
        filter: LibraryFilter,
        search: &str,
        sort: SortOrder,
    ) -> SqlResult<Vec<Document>> {
        let search_pat = like_pattern(search);
        let fts = fts_query(search);

        // Every filter is the same query with five slots swapped: which columns
        // to select, what to join, the conditions before the search clause, the
        // ordering, and an optional leading parameter. This used to be nine
        // near-identical arms, each preparing and draining its own statement.
        let q = filter.query();
        let search_idx = if q.param.is_some() { 2 } else { 1 };
        let sql = format!(
            "SELECT {} FROM {} WHERE {} AND {} ORDER BY {}{}",
            q.select,
            q.from,
            q.conditions,
            search_clause(q.prefix, search_idx, fts.is_some()),
            q.order(&sort),
            q.limit_clause(),
        );

        let mut args: Vec<rusqlite::types::Value> = Vec::new();
        if let Some(v) = q.param {
            args.push(v);
        }
        args.push(search_pat.into());
        if let Some(q) = fts {
            args.push(q.into());
        }

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(args), row_to_doc)?;
        let mut docs = Vec::new();
        for r in rows {
            docs.push(r?);
        }
        Ok(docs)
    }

    // The sidebar now takes its numbers from `counts()`; this stays as the
    // definition `counts()` is tested against.
    #[cfg_attr(not(test), allow(dead_code))]
    /// How many documents `filter` lists — built from the same `FilterSpec` as
    /// `documents()`, so the number beside a sidebar row can't drift from what
    /// clicking it shows.
    pub fn doc_count(&self, filter: &LibraryFilter) -> SqlResult<i64> {
        let q = filter.clone().query();
        let sql = format!(
            "SELECT COUNT(*) FROM (SELECT 1 FROM {} WHERE {}{})",
            q.from,
            q.conditions,
            q.limit_clause()
        );
        let args: Vec<rusqlite::types::Value> = q.param.into_iter().collect();
        self.conn
            .query_row(&sql, rusqlite::params_from_iter(args), |r| r.get(0))
    }

    /// Every number the sidebar shows, in a handful of grouped queries rather
    /// than one `COUNT(*)` per row.
    pub fn counts(&self) -> SqlResult<Counts> {
        let mut c = Counts::default();
        let (all, archive, trash, recent, unlabelled) = self.conn.query_row(
            "SELECT
                COALESCE(SUM(archived = 0 AND deleted = 0), 0),
                COALESCE(SUM(archived = 1 AND deleted = 0), 0),
                COALESCE(SUM(deleted = 1), 0),
                COALESCE(SUM(archived = 0 AND deleted = 0 AND last_opened_at IS NOT NULL), 0),
                COALESCE(SUM(archived = 0 AND deleted = 0
                             AND id NOT IN (SELECT doc_id FROM doc_labels)), 0)
             FROM documents",
            [],
            |r| {
                Ok::<(i64, i64, i64, i64, i64), rusqlite::Error>((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                ))
            },
        )?;
        c.all = all;
        c.archive = archive;
        c.trash = trash;
        c.recent = recent.min(RECENT_LIMIT as i64);
        c.unlabelled = unlabelled;

        let grouped = |sql: &str| -> SqlResult<Vec<(rusqlite::types::Value, i64)>> {
            let mut stmt = self.conn.prepare(sql)?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect()
        };
        let id = |v: rusqlite::types::Value| match v {
            rusqlite::types::Value::Integer(i) => i,
            _ => 0,
        };
        for (k, n) in grouped(
            "SELECT pd.project_id, COUNT(*) FROM project_docs pd
             JOIN documents d ON d.id = pd.doc_id
             WHERE d.deleted = 0 GROUP BY pd.project_id",
        )? {
            c.projects.insert(id(k), n);
        }
        for (k, n) in grouped(
            "SELECT dl.label_id, COUNT(*) FROM doc_labels dl
             JOIN documents d ON d.id = dl.doc_id
             WHERE d.archived = 0 AND d.deleted = 0 GROUP BY dl.label_id",
        )? {
            c.labels.insert(id(k), n);
        }
        for (k, n) in grouped(
            "SELECT da.author_id, COUNT(*) FROM doc_authors da
             JOIN documents d ON d.id = da.doc_id
             WHERE d.archived = 0 AND d.deleted = 0 GROUP BY da.author_id",
        )? {
            c.authors.insert(id(k), n);
        }
        Ok(c)
    }

    pub fn set_title(&mut self, doc_id: i64, title: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE documents SET title = ?1, title_custom = 1 WHERE id = ?2",
            params![title, doc_id],
        )?;
        self.reindex_row(doc_id, None)?;
        Ok(())
    }

    pub fn set_pinned(&mut self, doc_id: i64, pinned: bool) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE documents SET pinned=?1 WHERE id=?2",
            params![pinned as i64, doc_id],
        )?;
        Ok(())
    }

    pub fn move_to_trash(&mut self, doc_id: i64) -> Result<(), TrashError> {
        let doc = match self.doc_by_id(doc_id)? {
            Some(d) => d,
            None => return Ok(()),
        };
        let trash_dir = self.trash_dir.clone();
        std::fs::create_dir_all(&trash_dir)?;
        let ts = Utc::now().timestamp();
        // Prefix with doc_id (unique, from the primary key) rather than relying
        // on timestamp+basename alone, which can collide when two same-named
        // files are trashed within the same second.
        let filename = doc
            .path
            .file_name()
            .map(|n| format!("{}-{}-{}", ts, doc_id, n.to_string_lossy()))
            .unwrap_or_else(|| format!("{ts}-{doc_id}.typ"));
        let trash_path = trash_dir.join(&filename);

        // The filesystem move is authoritative: only mark the document
        // deleted in the database once the file has genuinely landed in the
        // trash directory. If rename fails (e.g. cross-device) and the copy
        // fallback also fails, or the copy succeeds but removing the
        // original doesn't, propagate the error and leave the database
        // untouched rather than recording a "trashed" file that's still
        // sitting at its original path.
        if std::fs::rename(&doc.path, &trash_path).is_err() {
            std::fs::copy(&doc.path, &trash_path)?;
            if let Err(e) = std::fs::remove_file(&doc.path) {
                // Don't leave an orphaned copy in Trash if the original
                // couldn't be removed — the document is still not deleted.
                let _ = std::fs::remove_file(&trash_path);
                return Err(e.into());
            }
        }

        let trash_str = trash_path.to_string_lossy().to_string();
        self.conn.execute(
            "UPDATE documents SET deleted=1, trash_path=?1 WHERE id=?2",
            params![trash_str, doc_id],
        )?;
        Ok(())
    }

    /// Recovers from the DB-write-fails-after-filesystem-succeeds cases that
    /// `move_to_trash`/`restore_from_trash`/`permanently_delete` can't fully
    /// rule out on their own: those methods make the filesystem authoritative
    /// and abort before touching the database if the filesystem step fails,
    /// but the reverse ordering (filesystem step succeeds, then the
    /// subsequent DB write itself fails — a locked/full/corrupt DB) can still
    /// leave the two disagreeing. Run once at startup, after `import_directory`,
    /// so a crash or DB error mid-operation self-heals on next launch instead
    /// of leaving a document stuck in a state the UI can't explain. Returns a
    /// human-readable note per fixup applied, for logging.
    pub fn reconcile_trash_state(&mut self) -> SqlResult<Vec<String>> {
        let mut notes = Vec::new();

        let trashed: Vec<(i64, Option<String>)> = {
            let mut stmt = self
                .conn
                .prepare("SELECT id, trash_path FROM documents WHERE deleted=1")?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<SqlResult<Vec<_>>>()?;
            rows
        };
        for (id, trash_path) in trashed {
            let trash_file_exists = trash_path.as_deref().is_some_and(|p| Path::new(p).exists());
            if trash_file_exists {
                continue;
            }
            let doc_path: Option<String> = self
                .conn
                .query_row("SELECT path FROM documents WHERE id=?1", params![id], |r| {
                    r.get(0)
                })
                .optional()?;
            if doc_path.as_deref().is_some_and(|p| Path::new(p).exists()) {
                // restore_from_trash's rename landed back at the original path
                // before the DB update that would have recorded it failed.
                self.conn.execute(
                    "UPDATE documents SET deleted=0, trash_path=NULL WHERE id=?1",
                    params![id],
                )?;
                notes.push(format!(
                    "Document {id} was already restored on disk but still listed as \
                     trashed; corrected the record"
                ));
            } else if let Some(found) = trash_file_for_doc(&self.trash_dir, id) {
                // move_to_trash's file move succeeded, but either the original
                // trash_path UPDATE failed, or the row's recorded path is stale.
                let found_str = found.to_string_lossy().to_string();
                self.conn.execute(
                    "UPDATE documents SET trash_path=?1 WHERE id=?2",
                    params![found_str, id],
                )?;
                notes.push(format!("Resynced trash path for document {id}"));
            } else {
                // permanently_delete's file removal succeeded but the row's
                // DELETE failed — nothing left to restore, so drop the row.
                self.conn
                    .execute("DELETE FROM documents WHERE id=?1", params![id])?;
                notes.push(format!(
                    "Document {id} was already permanently deleted; dropped its stale record"
                ));
            }
        }

        let active: Vec<(i64, String)> = {
            let mut stmt = self
                .conn
                .prepare("SELECT id, path FROM documents WHERE deleted=0")?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<SqlResult<Vec<_>>>()?;
            rows
        };
        for (id, path) in active {
            if Path::new(&path).exists() {
                continue;
            }
            if let Some(found) = trash_file_for_doc(&self.trash_dir, id) {
                // move_to_trash's file move succeeded but the deleted=1 UPDATE
                // failed, leaving an "active" row whose file is actually in Trash.
                let found_str = found.to_string_lossy().to_string();
                self.conn.execute(
                    "UPDATE documents SET deleted=1, trash_path=?1 WHERE id=?2",
                    params![found_str, id],
                )?;
                notes.push(format!(
                    "Document {id}'s file was found in Trash but the database still \
                     listed it as active; marked it trashed to match"
                ));
            }
            // Otherwise the file is simply missing (moved/deleted outside the
            // app) — not a case this method knows how to safely fix.
        }

        Ok(notes)
    }

    pub fn restore_from_trash(&mut self, doc_id: i64) -> SqlResult<()> {
        let trash_path: Option<String> = self
            .conn
            .query_row(
                "SELECT trash_path FROM documents WHERE id=?1",
                params![doc_id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();

        if let (Some(tpath), Some(doc)) = (trash_path, self.doc_by_id(doc_id)?) {
            // Don't clobber a file that now occupies the original path (e.g. a
            // new document created at the same path after this one was trashed).
            let dest = if doc.path.exists() {
                restore_collision_path(&doc.path)
            } else {
                doc.path.clone()
            };
            if std::fs::rename(&tpath, &dest).is_err() {
                return Ok(());
            }
            self.conn.execute(
                "UPDATE documents SET deleted=0, trash_path=NULL, path=?1 WHERE id=?2",
                params![dest.to_string_lossy().to_string(), doc_id],
            )?;
        }
        Ok(())
    }

    /// Removing the trashed file is authoritative, matching `move_to_trash`:
    /// if it can't be removed, the database keeps its reference to it rather
    /// than silently losing track of a file that's still on disk.
    pub fn permanently_delete(&mut self, doc_id: i64) -> Result<(), TrashError> {
        let trash_path: Option<String> = self
            .conn
            .query_row(
                "SELECT trash_path FROM documents WHERE id=?1",
                params![doc_id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        if let Some(tpath) = trash_path {
            if let Err(e) = std::fs::remove_file(&tpath) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(e.into());
                }
            }
        }
        self.conn
            .execute("DELETE FROM documents WHERE id=?1", params![doc_id])?;
        Ok(())
    }

    pub fn set_archived(&mut self, doc_id: i64, archived: bool) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE documents SET archived = ?1 WHERE id = ?2",
            params![archived as i64, doc_id],
        )?;
        Ok(())
    }

    pub fn set_notes(&mut self, doc_id: i64, notes: Option<&str>) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE documents SET notes=?1 WHERE id=?2",
            params![notes, doc_id],
        )?;
        self.reindex_row(doc_id, None)?;
        Ok(())
    }

    pub fn move_doc_in_project(
        &mut self,
        project_id: i64,
        doc_id: i64,
        new_position: i64,
    ) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE project_docs SET position = position + 1
             WHERE project_id=?1 AND doc_id != ?2 AND position >= ?3",
            params![project_id, doc_id, new_position],
        )?;
        self.conn.execute(
            "UPDATE project_docs SET position=?1 WHERE project_id=?2 AND doc_id=?3",
            params![new_position, project_id, doc_id],
        )?;
        Ok(())
    }

    pub fn position_in_project(&self, project_id: i64, doc_id: i64) -> SqlResult<Option<i64>> {
        self.conn
            .query_row(
                "SELECT position FROM project_docs WHERE project_id=?1 AND doc_id=?2",
                params![project_id, doc_id],
                |r| r.get(0),
            )
            .optional()
    }

    pub fn remove_document(&mut self, doc_id: i64) -> SqlResult<()> {
        self.conn
            .execute("DELETE FROM documents WHERE id = ?1", params![doc_id])?;
        Ok(())
    }

    // ── folder export ────────────────────────────────────────────────────

    /// Writes a complete, consistent copy of the library to `dest` — safe on a
    /// library that is open and being written to.
    pub fn backup_to(&self, dest: &Path) -> SqlResult<()> {
        self.conn
            .execute("VACUUM INTO ?1", params![dest.to_string_lossy()])?;
        Ok(())
    }

    /// This library's name among the machines that share the folder — made
    /// once, kept in the library, and never reused. Each machine writes only
    /// into its own folder named by it, so no two ever edit the same file.
    pub fn machine_id(&self) -> SqlResult<String> {
        if let Some(id) = self
            .conn
            .query_row("SELECT value FROM meta WHERE key = 'machine_id'", [], |r| {
                r.get::<_, String>(0)
            })
            .optional()?
        {
            return Ok(id);
        }
        use std::hash::{BuildHasher, Hasher};
        // Random enough to tell a handful of machines apart: the hasher's keys
        // are random per process, and the time and process make each run differ.
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        );
        h.write_u32(std::process::id());
        let id = format!("m-{:012x}", h.finish() & 0xffff_ffff_ffff);
        self.conn.execute(
            "INSERT OR IGNORE INTO meta (key, value) VALUES ('machine_id', ?1)",
            params![id],
        )?;
        self.conn
            .query_row("SELECT value FROM meta WHERE key = 'machine_id'", [], |r| {
                r.get(0)
            })
    }

    /// A small note left for the next time the Library window opens.
    pub fn set_note(&self, key: &str, value: &str) -> SqlResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Reads and clears a note left with `set_note`.
    pub fn take_note(&self, key: &str) -> Option<String> {
        let value: Option<String> = self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| {
                r.get(0)
            })
            .optional()
            .ok()
            .flatten();
        if value.is_some() {
            self.conn
                .execute("DELETE FROM meta WHERE key = ?1", params![key])
                .ok();
        }
        value
    }

    /// A count that moves whenever anything in the library is written, however
    /// small — cheap to poll to learn "something may have changed".
    pub fn change_stamp(&self) -> i64 {
        self.conn.total_changes() as i64
    }

    /// The library's own data, ready to write out. Documents in the Trash are
    /// left out.
    pub fn export_snapshot(&self) -> SqlResult<Snapshot> {
        let mut labels_of = self.labels_by_doc()?;
        let mut stmt = self.conn.prepare(
            "SELECT id, path, title, title_custom, pinned, archived, notes
             FROM documents WHERE deleted = 0 ORDER BY path",
        )?;
        let docs = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    DocState {
                        id: r.get::<_, i64>(0)?,
                        path: PathBuf::from(r.get::<_, String>(1)?),
                        title: if r.get::<_, i64>(3)? != 0 {
                            Some(r.get::<_, String>(2)?)
                        } else {
                            None
                        },
                        pinned: r.get::<_, i64>(4)? != 0,
                        archived: r.get::<_, i64>(5)? != 0,
                        notes: r
                            .get::<_, Option<String>>(6)?
                            .filter(|n| !n.trim().is_empty()),
                        labels: Vec::new(),
                    },
                ))
            })?
            .collect::<SqlResult<Vec<_>>>()?;
        let docs = docs
            .into_iter()
            .map(|(id, mut d)| {
                d.labels = labels_of
                    .remove(&id)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|l| l.name)
                    .collect();
                d
            })
            .collect();

        let mut projects = Vec::new();
        for p in self.all_projects()? {
            let root = match p.root_doc_id {
                Some(id) => self.doc_by_id(id)?.map(|d| d.path),
                None => None,
            };
            let mut stmt = self.conn.prepare(
                "SELECT d.path FROM project_docs pd JOIN documents d ON d.id = pd.doc_id
                 WHERE pd.project_id = ?1 AND d.deleted = 0 ORDER BY pd.position, d.title",
            )?;
            let documents = stmt
                .query_map(params![p.id], |r| Ok(PathBuf::from(r.get::<_, String>(0)?)))?
                .collect::<SqlResult<Vec<_>>>()?;
            projects.push(ProjectState {
                name: p.name,
                root,
                documents,
            });
        }
        Ok(Snapshot {
            docs,
            labels: self.all_labels()?,
            projects,
        })
    }

    /// What the export last wrote to each of its files, by path under the
    /// export folder.
    pub fn export_records(&self) -> SqlResult<std::collections::HashMap<String, String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, content FROM export_files")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect()
    }

    /// Forgets what the export has written — as if the library had been lost
    /// and rebuilt over the same folder.
    #[cfg(test)]
    pub fn forget_exports(&self) {
        self.conn.execute("DELETE FROM export_files", []).unwrap();
    }

    pub fn set_export_record(&self, path: &str, content: &str) -> SqlResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO export_files (path, content) VALUES (?1, ?2)",
            params![path, content],
        )?;
        Ok(())
    }

    // ── labels ───────────────────────────────────────────────────────────

    pub fn all_labels(&self) -> SqlResult<Vec<Label>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, color_hex FROM labels ORDER BY name COLLATE NOCASE")?;
        let rows = stmt.query_map([], label_from_row)?;
        rows.collect()
    }

    /// Every document's labels in one query, for drawing the list.
    pub fn labels_by_doc(&self) -> SqlResult<std::collections::HashMap<i64, Vec<Label>>> {
        let mut stmt = self.conn.prepare(
            "SELECT dl.doc_id, l.id, l.name, l.color_hex FROM doc_labels dl
             JOIN labels l ON l.id = dl.label_id
             ORDER BY l.name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                Label {
                    id: r.get(1)?,
                    name: r.get(2)?,
                    color_hex: r.get(3)?,
                },
            ))
        })?;
        let mut map: std::collections::HashMap<i64, Vec<Label>> = Default::default();
        for r in rows {
            let (doc, label) = r?;
            map.entry(doc).or_default().push(label);
        }
        Ok(map)
    }

    /// Makes a label, or returns the one that already has that name (names
    /// ignore case, so "sermons" is "Sermons").
    pub fn create_label(&mut self, name: &str) -> SqlResult<i64> {
        let name = name.trim();
        self.conn.execute(
            "INSERT OR IGNORE INTO labels (name) VALUES (?1)",
            params![name],
        )?;
        self.conn.query_row(
            "SELECT id FROM labels WHERE name = ?1",
            params![name],
            |r| r.get(0),
        )
    }

    pub fn set_label_color(&mut self, label_id: i64, color: Option<&str>) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE labels SET color_hex = ?1 WHERE id = ?2",
            params![color, label_id],
        )?;
        Ok(())
    }

    /// Renames a label. Fails if another label already has the name.
    pub fn rename_label(&mut self, label_id: i64, name: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE labels SET name = ?1 WHERE id = ?2",
            params![name.trim(), label_id],
        )?;
        Ok(())
    }

    /// Deletes a label; the documents that carried it stay, unlabelled by it.
    pub fn delete_label(&mut self, label_id: i64) -> SqlResult<()> {
        self.conn
            .execute("DELETE FROM labels WHERE id = ?1", params![label_id])?;
        Ok(())
    }

    /// Gives each document each label, leaving what they already carry alone.
    pub fn add_labels(&mut self, doc_ids: &[i64], label_ids: &[i64]) -> SqlResult<()> {
        let tx = self.conn.unchecked_transaction()?;
        for doc in doc_ids {
            for label in label_ids {
                tx.execute(
                    "INSERT OR IGNORE INTO doc_labels (doc_id, label_id) VALUES (?1, ?2)",
                    params![doc, label],
                )?;
            }
        }
        tx.commit()
    }

    /// Takes each label off each document.
    pub fn remove_labels(&mut self, doc_ids: &[i64], label_ids: &[i64]) -> SqlResult<()> {
        let tx = self.conn.unchecked_transaction()?;
        for doc in doc_ids {
            for label in label_ids {
                tx.execute(
                    "DELETE FROM doc_labels WHERE doc_id = ?1 AND label_id = ?2",
                    params![doc, label],
                )?;
            }
        }
        tx.commit()
    }

    /// For each label carried by any of `doc_ids`, how many of them carry it —
    /// what the Organize popover needs to show "all", "some" or "none".
    pub fn label_memberships(
        &self,
        doc_ids: &[i64],
    ) -> SqlResult<std::collections::HashMap<i64, usize>> {
        self.memberships("doc_labels", "label_id", doc_ids)
    }

    /// The same for projects.
    pub fn project_memberships(
        &self,
        doc_ids: &[i64],
    ) -> SqlResult<std::collections::HashMap<i64, usize>> {
        self.memberships("project_docs", "project_id", doc_ids)
    }

    fn memberships(
        &self,
        table: &str,
        column: &str,
        doc_ids: &[i64],
    ) -> SqlResult<std::collections::HashMap<i64, usize>> {
        if doc_ids.is_empty() {
            return Ok(Default::default());
        }
        let marks = vec!["?"; doc_ids.len()].join(",");
        let sql = format!(
            "SELECT {column}, COUNT(*) FROM {table} WHERE doc_id IN ({marks}) GROUP BY {column}"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(doc_ids.iter()), |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)? as usize))
        })?;
        rows.collect()
    }

    pub fn all_projects(&self) -> SqlResult<Vec<Project>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, root_doc_id, created_at FROM projects ORDER BY name")?;
        let rows = stmt.query_map([], |r| {
            Ok(Project {
                id: r.get(0)?,
                name: r.get(1)?,
                root_doc_id: r.get(2)?,
                created_at: r.get(3)?,
            })
        })?;
        let mut projects = Vec::new();
        for r in rows {
            projects.push(r?);
        }
        Ok(projects)
    }

    pub fn create_project(&mut self, name: &str) -> SqlResult<i64> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO projects (name, created_at) VALUES (?1, ?2)",
            params![name, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn rename_project(&mut self, project_id: i64, name: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE projects SET name = ?1 WHERE id = ?2",
            params![name, project_id],
        )?;
        Ok(())
    }

    pub fn delete_project(&mut self, project_id: i64) -> SqlResult<()> {
        self.conn
            .execute("DELETE FROM projects WHERE id = ?1", params![project_id])?;
        Ok(())
    }

    pub fn add_doc_to_project(&mut self, project_id: i64, doc_id: i64) -> SqlResult<()> {
        let next_pos: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM project_docs WHERE project_id = ?1",
            params![project_id],
            |r| r.get(0),
        )?;
        self.conn.execute(
            "INSERT OR IGNORE INTO project_docs (project_id, doc_id, position)
             VALUES (?1, ?2, ?3)",
            params![project_id, doc_id, next_pos],
        )?;
        Ok(())
    }

    // CRUD symmetry for the multi-project-membership feature (v0.29.0, "Documents can belong to
    // multiple categories") — `add_doc_to_project`/`set_project_root`/`project_root_path` are
    // live; these three (remove/lookup-by-path/reverse-lookup) aren't wired to any UI action yet.
    // Recent, active feature area — kept rather than deleted.
    #[allow(dead_code)]
    pub fn remove_doc_from_project(&mut self, project_id: i64, doc_id: i64) -> SqlResult<()> {
        self.conn.execute(
            "DELETE FROM project_docs WHERE project_id = ?1 AND doc_id = ?2",
            params![project_id, doc_id],
        )?;
        Ok(())
    }

    pub fn set_project_root(&mut self, project_id: i64, doc_id: Option<i64>) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE projects SET root_doc_id = ?1 WHERE id = ?2",
            params![doc_id, project_id],
        )?;
        Ok(())
    }

    pub fn project_root_path(&self, project_id: i64) -> SqlResult<Option<PathBuf>> {
        let res: Option<String> = self
            .conn
            .query_row(
                "SELECT d.path FROM documents d
                 JOIN projects p ON p.root_doc_id = d.id
                 WHERE p.id = ?1",
                params![project_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(res.map(PathBuf::from))
    }

    // See `remove_doc_from_project`'s comment above.
    #[allow(dead_code)]
    pub fn doc_by_path(&self, path: &Path) -> SqlResult<Option<Document>> {
        let path_str = path.to_string_lossy().to_string();
        let sql = format!("SELECT {DOC_COLS} FROM documents WHERE path = ?1{PATH_COLLATE}");
        self.conn
            .query_row(&sql, params![path_str], row_to_doc)
            .optional()
    }

    pub fn doc_by_id(&self, doc_id: i64) -> SqlResult<Option<Document>> {
        let sql = format!("SELECT {DOC_COLS} FROM documents WHERE id = ?1");
        self.conn
            .query_row(&sql, params![doc_id], row_to_doc)
            .optional()
    }

    // See `remove_doc_from_project`'s comment above.
    #[allow(dead_code)]
    pub fn project_of_doc(&self, doc_id: i64) -> SqlResult<Option<(i64, String)>> {
        self.conn
            .query_row(
                "SELECT p.id, p.name FROM projects p
                 JOIN project_docs pd ON pd.project_id = p.id
                 WHERE pd.doc_id = ?1 LIMIT 1",
                params![doc_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
    }

    /// Registers every `.typ` file under `dir`, skipping hidden folders and the
    /// `Templates` folder at the top — those are starting points for new
    /// documents, not documents, and listing them buries the real ones.
    pub fn import_directory(&mut self, dir: &Path) -> SqlResult<usize> {
        self.import_tree(dir, dir)
    }

    fn import_tree(&mut self, dir: &Path, root: &Path) -> SqlResult<usize> {
        let mut count = 0;
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Ok(0);
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let is_templates = dir == root && name == "Templates";
                if !name.starts_with('.') && !is_templates {
                    count += self.import_tree(&path, root)?;
                }
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("typ"))
            {
                self.upsert_document(&path)?;
                count += 1;
            }
        }
        Ok(count)
    }
}

/// `param` binds the LIKE pattern for names (title, labels, authors).
/// With `with_fts`, `param + 1` also binds the full-text query for what's inside
/// the document — its body, notes and file name. A search with no words in it
/// has no such query, and the clause leaves that part out: SQLite raises an
/// error for `MATCH NULL`.
fn search_clause(prefix: &str, param: usize, with_fts: bool) -> String {
    let fts = param + 1;
    let full_text = if with_fts {
        format!(" OR {prefix}id IN (SELECT rowid FROM doc_text WHERE doc_text MATCH ?{fts})")
    } else {
        String::new()
    };
    format!(
        "({prefix}title LIKE ?{param} ESCAPE '\\' \
         OR {prefix}id IN (SELECT doc_id FROM doc_labels _dl \
                           JOIN labels _l ON _l.id = _dl.label_id \
                           WHERE _l.name LIKE ?{param} ESCAPE '\\') \
         OR {prefix}id IN (SELECT doc_id FROM doc_authors _da \
                           JOIN authors _a ON _a.id = _da.author_id \
                           WHERE _a.name LIKE ?{param} ESCAPE '\\'){full_text})"
    )
}

/// What the search box means as a substring: `%` and `_` typed by the user are
/// ordinary characters, not wildcards.
fn like_pattern(search: &str) -> String {
    let mut out = String::from("%");
    for c in search.chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    out
}

/// The search box as an FTS5 query: each word, matched as a prefix so results
/// narrow as you type, all required. `None` when there are no words (an empty
/// box, or only punctuation).
fn fts_query(search: &str) -> Option<String> {
    let words: Vec<String> = search
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\"*"))
        .collect();
    if words.is_empty() {
        None
    } else {
        Some(words.join(" "))
    }
}

/// Wraps what `snippets` matched; chosen so they can't occur in real text.
pub const MARK_START: &str = "\u{e000}";
pub const MARK_END: &str = "\u{e001}";

/// A snippet as Pango markup: text escaped, matches in bold.
pub fn snippet_to_markup(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 16);
    for c in raw.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\u{e000}' => out.push_str("<b>"),
            '\u{e001}' => out.push_str("</b>"),
            c => out.push(c),
        }
    }
    out
}

/// The words of a document that count as prose: not code blocks, not lines of
/// Typst code or comments, and not citation keys, labels or inline code.
pub fn prose_words(content: &str) -> Vec<&str> {
    let mut in_code_block = false;
    let mut words = Vec::new();
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block || t.starts_with("//") || t.starts_with('#') {
            continue;
        }
        words.extend(t.split_whitespace().filter(|w| {
            // Not citation keys, labels or inline code — nor the `==` that
            // marks a heading, which is syntax, not a word.
            !w.starts_with('@')
                && !w.starts_with('<')
                && !w.starts_with('`')
                && !w.chars().all(|c| c == '=')
        }));
    }
    words
}

/// A file's modification time (nanoseconds) and length — the stamp that says
/// whether the index is still describing it.
fn file_stamp(path: &Path) -> Option<(i64, i64)> {
    let meta = std::fs::metadata(path).ok()?;
    let nanos = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some((nanos as i64, meta.len() as i64))
}

/// Looks for a file in `trash_dir` matching `move_to_trash`'s
/// `{timestamp}-{doc_id}-{original name}` naming, by doc id. Used by
/// `reconcile_trash_state` to relocate a trashed file whose `trash_path`
/// column didn't get recorded (or got recorded, then went stale).
fn trash_file_for_doc(trash_dir: &Path, doc_id: i64) -> Option<PathBuf> {
    let needle = format!("-{doc_id}-");
    let entries = std::fs::read_dir(trash_dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(idx) = name.find(needle.as_str()) {
            if name[..idx].chars().all(|c| c.is_ascii_digit()) {
                return Some(entry.path());
            }
        }
    }
    None
}

/// Finds a free path near `path` (e.g. `essay.typ` -> `essay (restored).typ`,
/// then `essay (restored 2).typ`, ...) for use when the original path is
/// already occupied by a different file.
fn restore_collision_path(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = path.extension().map(|e| e.to_string_lossy().to_string());
    let dir = path.parent().map(PathBuf::from).unwrap_or_default();
    let mut n = 1;
    loop {
        let label = if n == 1 {
            "restored".to_string()
        } else {
            format!("restored {n}")
        };
        let name = match &ext {
            Some(e) => format!("{stem} ({label}).{e}"),
            None => format!("{stem} ({label})"),
        };
        let candidate = dir.join(name);
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

/// Reads the first `#let doc-title = "..."` line from a Typst file.
/// Falls back to `#let title = "..."` if doc-title isn't found.
fn extract_typst_title(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut fallback: Option<String> = None;
    for line in content.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("#let doc-title") {
            if let Some(value) = rest.trim().strip_prefix('=') {
                if let Some(val) = parse_typst_string_value(value.trim()) {
                    return Some(val);
                }
            }
        }
        if fallback.is_none() {
            if let Some(rest) = t.strip_prefix("#let title") {
                if let Some(value) = rest.trim().strip_prefix('=') {
                    if let Some(val) = parse_typst_string_value(value.trim()) {
                        fallback = Some(val);
                    }
                }
            }
        }
    }
    fallback
}

fn parse_typst_string_value(s: &str) -> Option<String> {
    if let Some(inner) = s.strip_prefix('"') {
        let end = inner.find('"')?;
        let val = inner[..end].trim().to_string();
        if !val.is_empty() {
            return Some(val);
        }
    } else if let Some(inner) = s.strip_prefix('[') {
        let end = inner.find(']')?;
        let val = inner[..end].trim().to_string();
        if !val.is_empty() {
            return Some(val);
        }
    }
    None
}

fn label_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Label> {
    Ok(Label {
        id: row.get(0)?,
        name: row.get(1)?,
        color_hex: row.get(2)?,
    })
}

fn row_to_doc(row: &rusqlite::Row<'_>) -> rusqlite::Result<Document> {
    Ok(Document {
        id: row.get(0)?,
        path: PathBuf::from(row.get::<_, String>(1)?),
        title: row.get(2)?,
        archived: row.get::<_, i64>(3)? != 0,
        pinned: row.get::<_, i64>(4)? != 0,
        notes: row.get(5)?,
        created_at: row.get(6)?,
        modified_at: row.get(7)?,
        last_opened_at: row.get(8)?,
        words: row.get::<_, Option<i64>>(9)?.map(|w| w.max(0) as usize),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// A library whose trash lives inside `work`, plus a scratch dir to create
    /// real document files in — the trash/restore paths move files for real, so
    /// they need somewhere on disk that isn't Cal's data dir.
    fn fixture() -> (Library, TempDir) {
        let work = TempDir::new().expect("temp dir");
        let lib = Library::in_memory_with_trash_dir(work.path().join("trash"));
        (lib, work)
    }

    fn write_doc(work: &TempDir, name: &str, body: &str) -> PathBuf {
        let path = work.path().join(name);
        std::fs::write(&path, body).expect("write doc");
        path
    }

    fn add_doc(lib: &mut Library, work: &TempDir, name: &str) -> (i64, PathBuf) {
        let path = write_doc(work, name, "= Heading\n");
        let id = lib.upsert_document(&path).expect("upsert");
        (id, path)
    }

    fn titles(docs: &[Document]) -> Vec<String> {
        docs.iter().map(|d| d.title.clone()).collect()
    }

    fn ids(docs: &[Document]) -> Vec<i64> {
        docs.iter().map(|d| d.id).collect()
    }

    // ── Trash lifecycle ──────────────────────────────────────────────────────

    #[test]
    fn move_to_trash_flags_the_row_and_records_where_the_file_went() {
        let (mut lib, work) = fixture();
        let (id, path) = add_doc(&mut lib, &work, "essay.typ");

        lib.move_to_trash(id).expect("trash");

        let doc = lib
            .doc_by_id(id)
            .expect("query")
            .expect("row still present");
        assert!(!path.exists(), "original file should have been moved away");
        let trashed = lib
            .documents(LibraryFilter::Trash, "", SortOrder::Modified)
            .expect("list");
        assert_eq!(ids(&trashed), vec![doc.id]);
    }

    #[test]
    fn move_to_trash_moves_the_file_into_the_trash_dir_with_its_contents_intact() {
        let (mut lib, work) = fixture();
        let path = write_doc(&work, "essay.typ", "= Original body\n");
        let id = lib.upsert_document(&path).expect("upsert");

        lib.move_to_trash(id).expect("trash");

        let trash_dir = work.path().join("trash");
        let entries: Vec<_> = std::fs::read_dir(&trash_dir)
            .expect("trash dir created")
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(entries.len(), 1, "exactly one file should be in the trash");
        let contents = std::fs::read_to_string(entries[0].path()).expect("read trashed file");
        assert_eq!(contents, "= Original body\n");
    }

    #[test]
    fn move_to_trash_leaves_the_row_untouched_if_the_source_file_is_already_gone() {
        let (mut lib, work) = fixture();
        let (id, path) = add_doc(&mut lib, &work, "essay.typ");
        std::fs::remove_file(&path).expect("remove out from under the library");

        let err = lib
            .move_to_trash(id)
            .expect_err("rename and copy both fail");
        assert!(matches!(err, TrashError::Io(_)));

        let docs = lib
            .documents(LibraryFilter::All, "", SortOrder::Modified)
            .expect("list");
        assert_eq!(
            ids(&docs),
            vec![id],
            "must not be flagged deleted if the file never moved"
        );
    }

    #[test]
    fn move_to_trash_leaves_the_row_untouched_if_the_trash_dir_cannot_be_created() {
        let (mut lib, work) = fixture();
        let (id, path) = add_doc(&mut lib, &work, "essay.typ");
        // A plain file sitting at the trash dir's path makes `create_dir_all`
        // fail with "not a directory" — stands in for a real-world case like
        // a full disk or a permissions error preventing trash dir creation.
        std::fs::write(work.path().join("trash"), b"not a directory").expect("blocker file");

        let err = lib
            .move_to_trash(id)
            .expect_err("trash dir cannot be created");
        assert!(matches!(err, TrashError::Io(_)));

        assert!(path.exists(), "original file must be left in place");
        let docs = lib
            .documents(LibraryFilter::All, "", SortOrder::Modified)
            .expect("list");
        assert_eq!(
            ids(&docs),
            vec![id],
            "must not be flagged deleted if the file never moved"
        );
    }

    #[cfg(unix)]
    #[test]
    fn move_to_trash_leaves_the_row_untouched_if_the_source_cannot_be_removed_after_copying() {
        use std::os::unix::fs::PermissionsExt;

        let (mut lib, work) = fixture();
        let src_dir = work.path().join("src");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        let path = src_dir.join("essay.typ");
        std::fs::write(&path, "= Heading\n").expect("write doc");
        let id = lib.upsert_document(&path).expect("upsert");

        // Read-only source directory: `rename` needs write permission on the
        // source dir to unlink the entry, so it falls back to `copy` (which
        // only needs read on the file itself, so it succeeds) — but the
        // subsequent `remove_file` needs the same write permission `rename`
        // was missing, so it fails too. This reproduces "copy succeeded but
        // removing the original didn't" without needing a real cross-device
        // filesystem boundary to force `rename`'s EXDEV path.
        let original_mode = std::fs::metadata(&src_dir).expect("stat").permissions();
        std::fs::set_permissions(&src_dir, std::fs::Permissions::from_mode(0o555))
            .expect("make src dir read-only");

        let result = lib.move_to_trash(id);

        std::fs::set_permissions(&src_dir, original_mode).expect("restore permissions for cleanup");

        let err = result.expect_err("copy succeeds but removing the original fails");
        assert!(matches!(err, TrashError::Io(_)));
        assert!(path.exists(), "original file must still be present");
        let docs = lib
            .documents(LibraryFilter::All, "", SortOrder::Modified)
            .expect("list");
        assert_eq!(
            ids(&docs),
            vec![id],
            "must not be flagged deleted if the original wasn't removed"
        );
    }

    /// The `{ts}-{doc_id}-{name}` scheme exists because timestamp+basename alone
    /// collides for two same-named files trashed within the same second.
    #[test]
    fn two_same_named_files_trashed_in_the_same_second_do_not_overwrite_each_other() {
        let (mut lib, work) = fixture();
        let dir_a = work.path().join("a");
        let dir_b = work.path().join("b");
        std::fs::create_dir_all(&dir_a).unwrap();
        std::fs::create_dir_all(&dir_b).unwrap();
        std::fs::write(dir_a.join("notes.typ"), "= From A\n").unwrap();
        std::fs::write(dir_b.join("notes.typ"), "= From B\n").unwrap();

        let id_a = lib
            .upsert_document(&dir_a.join("notes.typ"))
            .expect("upsert a");
        let id_b = lib
            .upsert_document(&dir_b.join("notes.typ"))
            .expect("upsert b");
        lib.move_to_trash(id_a).expect("trash a");
        lib.move_to_trash(id_b).expect("trash b");

        let mut bodies: Vec<String> = std::fs::read_dir(work.path().join("trash"))
            .expect("trash dir")
            .filter_map(|e| e.ok())
            .map(|e| std::fs::read_to_string(e.path()).expect("read"))
            .collect();
        bodies.sort();
        assert_eq!(
            bodies,
            vec!["= From A\n".to_string(), "= From B\n".to_string()]
        );
    }

    #[test]
    fn restore_from_trash_puts_the_file_back_and_clears_the_deleted_flag() {
        let (mut lib, work) = fixture();
        let path = write_doc(&work, "essay.typ", "= Body\n");
        let id = lib.upsert_document(&path).expect("upsert");

        lib.move_to_trash(id).expect("trash");
        lib.restore_from_trash(id).expect("restore");

        assert!(path.exists(), "file should be back at its original path");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "= Body\n");
        let trashed = lib
            .documents(LibraryFilter::Trash, "", SortOrder::Modified)
            .expect("list");
        assert!(trashed.is_empty(), "doc should no longer be in the trash");
        let all = lib
            .documents(LibraryFilter::All, "", SortOrder::Modified)
            .expect("list");
        assert_eq!(ids(&all), vec![id]);
    }

    /// The data-loss case: something new occupies the original path by the time
    /// the old document is restored. The restore must go beside it, not over it.
    #[test]
    fn restoring_onto_an_occupied_path_does_not_clobber_the_new_file() {
        let (mut lib, work) = fixture();
        let path = write_doc(&work, "essay.typ", "= The old one\n");
        let id = lib.upsert_document(&path).expect("upsert");
        lib.move_to_trash(id).expect("trash");

        std::fs::write(&path, "= A different, newer file\n").unwrap();
        lib.restore_from_trash(id).expect("restore");

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "= A different, newer file\n",
            "the newer file must survive untouched"
        );
        let restored = work.path().join("essay (restored).typ");
        assert!(restored.exists(), "restored copy should land beside it");
        assert_eq!(
            std::fs::read_to_string(&restored).unwrap(),
            "= The old one\n"
        );

        let doc = lib.doc_by_id(id).expect("query").expect("row");
        assert_eq!(
            doc.path, restored,
            "DB path should follow the file to its new home"
        );
    }

    #[test]
    fn restore_from_trash_on_a_doc_that_was_never_trashed_is_a_no_op() {
        let (mut lib, work) = fixture();
        let (id, path) = add_doc(&mut lib, &work, "essay.typ");

        lib.restore_from_trash(id)
            .expect("restore should not error");

        assert!(path.exists());
        let all = lib
            .documents(LibraryFilter::All, "", SortOrder::Modified)
            .expect("list");
        assert_eq!(ids(&all), vec![id]);
    }

    #[test]
    fn permanently_delete_removes_both_the_row_and_the_trashed_file() {
        let (mut lib, work) = fixture();
        let (id, _) = add_doc(&mut lib, &work, "essay.typ");
        lib.move_to_trash(id).expect("trash");

        lib.permanently_delete(id).expect("delete");

        assert!(
            lib.doc_by_id(id).expect("query").is_none(),
            "row should be gone"
        );
        let remaining = std::fs::read_dir(work.path().join("trash"))
            .expect("trash dir")
            .filter_map(|e| e.ok())
            .count();
        assert_eq!(remaining, 0, "trashed file should be gone from disk");
    }

    #[test]
    fn reconcile_marks_trashed_a_doc_whose_file_moved_but_whose_row_was_never_updated() {
        let (mut lib, work) = fixture();
        let (id, _) = add_doc(&mut lib, &work, "essay.typ");
        lib.move_to_trash(id).expect("trash");
        // Simulate the DB UPDATE having failed right after the fs move
        // succeeded: put the row back as if it were still active.
        lib.conn
            .execute(
                "UPDATE documents SET deleted=0, trash_path=NULL WHERE id=?1",
                params![id],
            )
            .expect("simulate stale row");

        let notes = lib.reconcile_trash_state().expect("reconcile");

        assert_eq!(notes.len(), 1);
        assert!(
            lib.doc_by_id(id).expect("query").is_some(),
            "row still exists"
        );
        let deleted: i64 = lib
            .conn
            .query_row(
                "SELECT deleted FROM documents WHERE id=?1",
                params![id],
                |r| r.get(0),
            )
            .expect("read deleted flag");
        assert_eq!(deleted, 1, "should be corrected back to trashed");
    }

    #[test]
    fn reconcile_restores_a_doc_whose_file_was_put_back_but_whose_row_still_says_trashed() {
        let (mut lib, work) = fixture();
        let (id, path) = add_doc(&mut lib, &work, "essay.typ");
        lib.move_to_trash(id).expect("trash");
        let trash_path: String = lib
            .conn
            .query_row(
                "SELECT trash_path FROM documents WHERE id=?1",
                params![id],
                |r| r.get(0),
            )
            .expect("read trash_path");
        // Simulate restore_from_trash's rename having succeeded, followed by
        // its DB UPDATE failing.
        std::fs::rename(&trash_path, &path).expect("simulate fs-only restore");

        let notes = lib.reconcile_trash_state().expect("reconcile");

        assert_eq!(notes.len(), 1);
        let deleted: i64 = lib
            .conn
            .query_row(
                "SELECT deleted FROM documents WHERE id=?1",
                params![id],
                |r| r.get(0),
            )
            .expect("read deleted flag");
        assert_eq!(deleted, 0, "should be corrected back to active");
    }

    #[test]
    fn reconcile_drops_a_row_whose_trashed_file_was_already_permanently_removed() {
        let (mut lib, work) = fixture();
        let (id, _) = add_doc(&mut lib, &work, "essay.typ");
        lib.move_to_trash(id).expect("trash");
        let trash_path: String = lib
            .conn
            .query_row(
                "SELECT trash_path FROM documents WHERE id=?1",
                params![id],
                |r| r.get(0),
            )
            .expect("read trash_path");
        // Simulate permanently_delete's remove_file having succeeded, followed
        // by its DB DELETE failing.
        std::fs::remove_file(&trash_path).expect("simulate fs-only permanent delete");

        let notes = lib.reconcile_trash_state().expect("reconcile");

        assert_eq!(notes.len(), 1);
        assert!(
            lib.doc_by_id(id).expect("query").is_none(),
            "stale row should be dropped"
        );
    }

    #[test]
    fn reconcile_is_a_no_op_when_db_and_filesystem_already_agree() {
        let (mut lib, work) = fixture();
        let (id, _) = add_doc(&mut lib, &work, "essay.typ");
        add_doc(&mut lib, &work, "other.typ");
        lib.move_to_trash(id).expect("trash");

        let notes = lib.reconcile_trash_state().expect("reconcile");

        assert!(notes.is_empty(), "nothing should need fixing: {notes:?}");
    }

    #[cfg(unix)]
    #[test]
    fn move_to_trash_removes_the_orphaned_copy_if_removing_the_original_fails() {
        use std::os::unix::fs::PermissionsExt;

        let (mut lib, work) = fixture();
        let src_dir = work.path().join("src");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        let path = src_dir.join("essay.typ");
        std::fs::write(&path, "= Heading\n").expect("write doc");
        let id = lib.upsert_document(&path).expect("upsert");

        let original_mode = std::fs::metadata(&src_dir).expect("stat").permissions();
        std::fs::set_permissions(&src_dir, std::fs::Permissions::from_mode(0o555))
            .expect("make src dir read-only");

        let result = lib.move_to_trash(id);

        std::fs::set_permissions(&src_dir, original_mode).expect("restore permissions for cleanup");

        assert!(result.is_err());
        let leftover = std::fs::read_dir(work.path().join("trash"))
            .expect("trash dir")
            .filter_map(|e| e.ok())
            .count();
        assert_eq!(
            leftover, 0,
            "the copy landed in trash must be cleaned up, not orphaned"
        );
    }

    #[cfg(unix)]
    #[test]
    fn permanently_delete_keeps_the_row_if_the_file_cannot_be_removed() {
        use std::os::unix::fs::PermissionsExt;

        let (mut lib, work) = fixture();
        let (id, _) = add_doc(&mut lib, &work, "essay.typ");
        lib.move_to_trash(id).expect("trash");

        let trash_dir = work.path().join("trash");
        let original_mode = std::fs::metadata(&trash_dir).expect("stat").permissions();
        std::fs::set_permissions(&trash_dir, std::fs::Permissions::from_mode(0o555))
            .expect("make trash dir read-only");

        let result = lib.permanently_delete(id);

        std::fs::set_permissions(&trash_dir, original_mode)
            .expect("restore permissions for cleanup");

        assert!(
            result.is_err(),
            "should not silently succeed if the file can't be removed"
        );
        assert!(
            lib.doc_by_id(id).expect("query").is_some(),
            "row must survive a failed filesystem delete"
        );
    }

    #[test]
    fn move_to_trash_on_an_unknown_id_is_a_no_op() {
        let (mut lib, _work) = fixture();
        lib.move_to_trash(4242).expect("should not error");
    }

    #[test]
    fn trashed_documents_disappear_from_every_other_filter() {
        let (mut lib, work) = fixture();
        let (id, _) = add_doc(&mut lib, &work, "essay.typ");
        let label = lib.create_label("Essays").expect("label");
        lib.add_labels(&[id], &[label]).expect("label it");
        lib.touch_opened(&work.path().join("essay.typ"))
            .expect("open");

        lib.move_to_trash(id).expect("trash");

        for filter in [
            LibraryFilter::All,
            LibraryFilter::Label(label),
            LibraryFilter::Recent,
        ] {
            let docs = lib
                .documents(filter.clone(), "", SortOrder::Modified)
                .expect("list");
            assert!(docs.is_empty(), "{filter:?} should not show trashed docs");
            assert_eq!(
                lib.doc_count(&filter).expect("count"),
                0,
                "{filter:?} count"
            );
        }
    }

    // ── Filters ──────────────────────────────────────────────────────────────

    #[test]
    fn all_filter_excludes_archived_and_deleted() {
        let (mut lib, work) = fixture();
        let (keep, _) = add_doc(&mut lib, &work, "keep.typ");
        let (archived, _) = add_doc(&mut lib, &work, "archived.typ");
        let (trashed, _) = add_doc(&mut lib, &work, "trashed.typ");
        lib.set_archived(archived, true).expect("archive");
        lib.move_to_trash(trashed).expect("trash");

        let docs = lib
            .documents(LibraryFilter::All, "", SortOrder::Modified)
            .expect("list");
        assert_eq!(ids(&docs), vec![keep]);
        assert_eq!(lib.doc_count(&LibraryFilter::All).expect("count"), 1);
    }

    #[test]
    fn archive_filter_shows_only_archived_and_not_deleted() {
        let (mut lib, work) = fixture();
        let (a, _) = add_doc(&mut lib, &work, "a.typ");
        let (b, _) = add_doc(&mut lib, &work, "b.typ");
        lib.set_archived(a, true).expect("archive a");
        lib.set_archived(b, true).expect("archive b");
        lib.move_to_trash(b).expect("trash b");

        let docs = lib
            .documents(LibraryFilter::Archive, "", SortOrder::Modified)
            .expect("list");
        assert_eq!(ids(&docs), vec![a]);
        assert_eq!(lib.doc_count(&LibraryFilter::Archive).expect("count"), 1);
    }

    #[test]
    fn recent_filter_shows_only_documents_that_have_been_opened() {
        let (mut lib, work) = fixture();
        let (opened, opened_path) = add_doc(&mut lib, &work, "opened.typ");
        add_doc(&mut lib, &work, "never-opened.typ");
        lib.touch_opened(&opened_path).expect("open");

        let docs = lib
            .documents(LibraryFilter::Recent, "", SortOrder::Opened)
            .expect("list");
        assert_eq!(ids(&docs), vec![opened]);
        assert_eq!(lib.doc_count(&LibraryFilter::Recent).expect("count"), 1);
    }

    #[test]
    fn project_filter_returns_documents_in_stored_position_order() {
        let (mut lib, work) = fixture();
        let (first, _) = add_doc(&mut lib, &work, "first.typ");
        let (second, _) = add_doc(&mut lib, &work, "second.typ");
        let (third, _) = add_doc(&mut lib, &work, "third.typ");
        let project = lib.create_project("Thesis").expect("project");
        for id in [first, second, third] {
            lib.add_doc_to_project(project, id).expect("add");
        }

        let docs = lib
            .documents(LibraryFilter::Project(project), "", SortOrder::Title)
            .expect("list");
        assert_eq!(ids(&docs), vec![first, second, third]);
        assert_eq!(
            lib.doc_count(&LibraryFilter::Project(project))
                .expect("count"),
            3
        );
    }

    // ── Search ───────────────────────────────────────────────────────────────

    #[test]
    fn empty_search_matches_everything() {
        let (mut lib, work) = fixture();
        add_doc(&mut lib, &work, "alpha.typ");
        add_doc(&mut lib, &work, "beta.typ");

        let docs = lib
            .documents(LibraryFilter::All, "", SortOrder::Title)
            .expect("list");
        assert_eq!(docs.len(), 2);
    }

    #[test]
    fn search_matches_a_substring_of_the_title_case_insensitively() {
        let (mut lib, work) = fixture();
        let (id, _) = add_doc(&mut lib, &work, "Reformation.typ");
        add_doc(&mut lib, &work, "unrelated.typ");

        for query in ["Reform", "reform", "format"] {
            let docs = lib
                .documents(LibraryFilter::All, query, SortOrder::Title)
                .expect("list");
            assert_eq!(ids(&docs), vec![id], "query {query:?}");
        }
    }

    #[test]
    fn search_that_matches_nothing_returns_empty() {
        let (mut lib, work) = fixture();
        add_doc(&mut lib, &work, "alpha.typ");

        let docs = lib
            .documents(LibraryFilter::All, "no-such-document", SortOrder::Title)
            .expect("list");
        assert!(docs.is_empty());
    }

    // ── Sorting ──────────────────────────────────────────────────────────────

    #[test]
    fn title_sort_is_alphabetical_and_case_insensitive() {
        let (mut lib, work) = fixture();
        for (name, title) in [("c.typ", "cherry"), ("a.typ", "Apple"), ("b.typ", "banana")] {
            let path = write_doc(&work, name, &format!("#let doc-title = \"{title}\"\n"));
            lib.upsert_document(&path).expect("upsert");
        }

        let docs = lib
            .documents(LibraryFilter::All, "", SortOrder::Title)
            .expect("list");
        assert_eq!(titles(&docs), vec!["Apple", "banana", "cherry"]);
    }

    #[test]
    fn opened_sort_puts_never_opened_documents_last() {
        let (mut lib, work) = fixture();
        let (opened, opened_path) = add_doc(&mut lib, &work, "opened.typ");
        let (never, _) = add_doc(&mut lib, &work, "never.typ");
        lib.touch_opened(&opened_path).expect("open");

        let docs = lib
            .documents(LibraryFilter::All, "", SortOrder::Opened)
            .expect("list");
        assert_eq!(ids(&docs), vec![opened, never]);
    }

    #[test]
    fn pinned_documents_sort_ahead_of_unpinned_ones_regardless_of_sort_order() {
        let (mut lib, work) = fixture();
        let path_a = write_doc(&work, "a.typ", "#let doc-title = \"Aaa\"\n");
        let path_z = write_doc(&work, "z.typ", "#let doc-title = \"Zzz\"\n");
        lib.upsert_document(&path_a).expect("upsert");
        let zzz = lib.upsert_document(&path_z).expect("upsert");
        lib.set_pinned(zzz, true).expect("pin");

        let docs = lib
            .documents(LibraryFilter::All, "", SortOrder::Title)
            .expect("list");
        assert_eq!(
            titles(&docs),
            vec!["Zzz", "Aaa"],
            "pinned wins even though Title sort would put Aaa first"
        );
    }

    #[test]
    fn doc_count_agrees_with_the_number_of_documents_returned() {
        let (mut lib, work) = fixture();
        for name in ["a.typ", "b.typ", "c.typ"] {
            add_doc(&mut lib, &work, name);
        }
        let archived = lib
            .upsert_document(&write_doc(&work, "d.typ", "x"))
            .expect("upsert");
        lib.set_archived(archived, true).expect("archive");

        for filter in [
            LibraryFilter::All,
            LibraryFilter::Archive,
            LibraryFilter::Unlabelled,
        ] {
            let listed = lib
                .documents(filter.clone(), "", SortOrder::Modified)
                .expect("list")
                .len();
            assert_eq!(
                listed as i64,
                lib.doc_count(&filter).expect("count"),
                "{filter:?}"
            );
        }
    }

    // ── Upsert & timestamps ──────────────────────────────────────────────────

    #[test]
    fn upserting_the_same_path_twice_returns_the_same_id_and_does_not_duplicate() {
        let (mut lib, work) = fixture();
        let path = write_doc(&work, "essay.typ", "= One\n");

        let first = lib.upsert_document(&path).expect("first");
        let second = lib.upsert_document(&path).expect("second");

        assert_eq!(first, second);
        let docs = lib
            .documents(LibraryFilter::All, "", SortOrder::Title)
            .expect("list");
        assert_eq!(docs.len(), 1);
    }

    #[test]
    fn upsert_picks_up_a_retitled_document() {
        let (mut lib, work) = fixture();
        let path = write_doc(&work, "essay.typ", "#let doc-title = \"First Title\"\n");
        let id = lib.upsert_document(&path).expect("upsert");
        assert_eq!(lib.doc_by_id(id).unwrap().unwrap().title, "First Title");

        std::fs::write(&path, "#let doc-title = \"Second Title\"\n").unwrap();
        lib.upsert_document(&path).expect("re-upsert");

        assert_eq!(lib.doc_by_id(id).unwrap().unwrap().title, "Second Title");
    }

    #[test]
    fn a_document_with_no_title_declaration_falls_back_to_its_filename() {
        let (mut lib, work) = fixture();
        let path = write_doc(&work, "my-essay.typ", "= Just a heading\n");

        let id = lib.upsert_document(&path).expect("upsert");

        assert_eq!(lib.doc_by_id(id).unwrap().unwrap().title, "my-essay");
    }

    #[test]
    fn touch_opened_sets_last_opened_without_touching_modified() {
        let (mut lib, work) = fixture();
        let (id, path) = add_doc(&mut lib, &work, "essay.typ");
        let before = lib.doc_by_id(id).unwrap().unwrap();
        assert!(before.last_opened_at.is_none());

        lib.touch_opened(&path).expect("open");

        let after = lib.doc_by_id(id).unwrap().unwrap();
        assert!(after.last_opened_at.is_some());
        assert_eq!(after.modified_at, before.modified_at);
    }

    #[test]
    fn touch_saved_advances_modified_without_setting_last_opened() {
        let (mut lib, work) = fixture();
        let (id, path) = add_doc(&mut lib, &work, "essay.typ");
        let before = lib.doc_by_id(id).unwrap().unwrap();

        lib.touch_saved(&path).expect("save");

        let after = lib.doc_by_id(id).unwrap().unwrap();
        assert!(after.modified_at >= before.modified_at);
        assert!(after.last_opened_at.is_none());
    }

    // ── Tags ─────────────────────────────────────────────────────────────────

    #[test]
    fn documents_added_to_a_project_get_sequential_positions() {
        let (mut lib, work) = fixture();
        let project = lib.create_project("Thesis").expect("project");
        let (a, _) = add_doc(&mut lib, &work, "a.typ");
        let (b, _) = add_doc(&mut lib, &work, "b.typ");
        lib.add_doc_to_project(project, a).expect("add");
        lib.add_doc_to_project(project, b).expect("add");

        assert_eq!(lib.position_in_project(project, a).expect("pos"), Some(0));
        assert_eq!(lib.position_in_project(project, b).expect("pos"), Some(1));
    }

    #[test]
    fn moving_a_document_to_the_front_of_a_project_reorders_the_rest() {
        let (mut lib, work) = fixture();
        let project = lib.create_project("Thesis").expect("project");
        let (a, _) = add_doc(&mut lib, &work, "a.typ");
        let (b, _) = add_doc(&mut lib, &work, "b.typ");
        let (c, _) = add_doc(&mut lib, &work, "c.typ");
        for id in [a, b, c] {
            lib.add_doc_to_project(project, id).expect("add");
        }

        lib.move_doc_in_project(project, c, 0).expect("move");

        let docs = lib
            .documents(LibraryFilter::Project(project), "", SortOrder::Title)
            .expect("list");
        assert_eq!(ids(&docs), vec![c, a, b]);
    }

    #[test]
    fn deleting_a_project_leaves_its_documents_alone() {
        let (mut lib, work) = fixture();
        let project = lib.create_project("Thesis").expect("project");
        let (id, _) = add_doc(&mut lib, &work, "essay.typ");
        lib.add_doc_to_project(project, id).expect("add");

        lib.delete_project(project).expect("delete");

        assert!(lib.all_projects().expect("projects").is_empty());
        let docs = lib
            .documents(LibraryFilter::All, "", SortOrder::Title)
            .expect("list");
        assert_eq!(ids(&docs), vec![id], "the document itself should survive");
    }

    #[test]
    fn deleting_a_document_that_is_a_project_root_clears_the_root_reference() {
        let (mut lib, work) = fixture();
        let project = lib.create_project("Thesis").expect("project");
        let (id, path) = add_doc(&mut lib, &work, "main.typ");
        lib.add_doc_to_project(project, id).expect("add");
        lib.set_project_root(project, Some(id)).expect("set root");
        assert_eq!(lib.project_root_path(project).expect("root"), Some(path));

        lib.remove_document(id).expect("remove");

        assert_eq!(lib.project_root_path(project).expect("root"), None);
        assert_eq!(lib.all_projects().expect("projects").len(), 1);
    }

    // ── Import ───────────────────────────────────────────────────────────────

    #[test]
    fn import_directory_recurses_but_skips_hidden_dirs_and_non_typst_files() {
        let (mut lib, work) = fixture();
        std::fs::create_dir_all(work.path().join("nested")).unwrap();
        std::fs::create_dir_all(work.path().join(".hidden")).unwrap();
        std::fs::write(work.path().join("top.typ"), "= Top\n").unwrap();
        std::fs::write(work.path().join("nested/deep.typ"), "= Deep\n").unwrap();
        std::fs::write(work.path().join("notes.md"), "not typst").unwrap();
        std::fs::write(work.path().join(".hidden/secret.typ"), "= Secret\n").unwrap();

        let count = lib.import_directory(work.path()).expect("import");

        assert_eq!(count, 2);
        let found: Vec<String> = lib
            .documents(LibraryFilter::All, "", SortOrder::Title)
            .expect("list")
            .into_iter()
            .map(|d| d.title)
            .collect();
        assert_eq!(found, vec!["deep", "top"]);
    }

    #[test]
    fn a_title_chosen_in_the_library_survives_a_rescan() {
        let (mut lib, work) = fixture();
        let path = write_doc(&work, "essay.typ", "#let doc-title = \"From The File\"\n");
        let id = lib.upsert_document(&path).expect("upsert");
        lib.set_title(id, "My Name For It").unwrap();

        lib.upsert_document(&path).expect("rescan");
        lib.touch_saved(&path).expect("save");

        assert_eq!(lib.doc_by_id(id).unwrap().unwrap().title, "My Name For It");
    }

    const BIB: &str = "@book{gender, author = {Butler, Judith}}\n\
                       @book{black, author = {Cone, James H.}}\n";

    fn lib_with_bib() -> (Library, TempDir, PathBuf) {
        let (mut lib, work) = fixture();
        let bib = write_doc(&work, "refs.bib", BIB);
        lib.set_bibliography(Some(bib.clone()));
        (lib, work, bib)
    }

    fn author_names(lib: &Library) -> Vec<String> {
        lib.all_authors_with_counts()
            .unwrap()
            .into_iter()
            .map(|(a, _)| a.name)
            .collect()
    }

    #[test]
    fn scanning_a_document_tags_it_with_the_authors_it_cites() {
        let (mut lib, work, _) = lib_with_bib();
        let path = write_doc(&work, "paper.typ", "See @gender and @black.\n");
        let id = lib.upsert_document(&path).unwrap();

        assert_eq!(author_names(&lib), vec!["Butler, J.", "Cone, J."]);
        let butler = lib.all_authors_with_counts().unwrap()[0].0.id;
        let docs = lib
            .documents(LibraryFilter::Author(butler), "", SortOrder::Title)
            .unwrap();
        assert_eq!(ids(&docs), vec![id]);
        assert_eq!(lib.doc_count(&LibraryFilter::Author(butler)).unwrap(), 1);
    }

    #[test]
    fn the_author_filter_lists_every_document_citing_that_author() {
        let (mut lib, work, _) = lib_with_bib();
        let a = write_doc(&work, "a.typ", "@gender @black\n");
        let b = write_doc(&work, "b.typ", "@black\n");
        let c = write_doc(&work, "c.typ", "@gender\n");
        let (ia, ib) = (
            lib.upsert_document(&a).unwrap(),
            lib.upsert_document(&b).unwrap(),
        );
        lib.upsert_document(&c).unwrap();

        let cone = lib
            .all_authors_with_counts()
            .unwrap()
            .into_iter()
            .find(|(a, _)| a.name == "Cone, J.")
            .unwrap();
        assert_eq!(cone.1, 2);
        let docs = lib
            .documents(LibraryFilter::Author(cone.0.id), "", SortOrder::Title)
            .unwrap();
        let mut got = ids(&docs);
        got.sort();
        assert_eq!(got, vec![ia, ib]);
    }

    #[test]
    fn searching_finds_a_document_by_a_cited_author() {
        let (mut lib, work, _) = lib_with_bib();
        let path = write_doc(&work, "paper.typ", "@gender\n");
        let id = lib.upsert_document(&path).unwrap();
        write_doc(&work, "other.typ", "No citations.\n");
        lib.upsert_document(&work.path().join("other.typ")).unwrap();

        for q in ["Butler", "butler, j", "Butler, J."] {
            let found = lib
                .documents(LibraryFilter::All, q, SortOrder::Title)
                .unwrap();
            assert_eq!(ids(&found), vec![id], "query {q:?}");
        }
    }

    #[test]
    fn dropping_a_citation_drops_the_author_tag_and_an_uncited_author_disappears() {
        let (mut lib, work, _) = lib_with_bib();
        let path = write_doc(&work, "paper.typ", "@gender @black\n");
        lib.upsert_document(&path).unwrap();
        assert_eq!(author_names(&lib), vec!["Butler, J.", "Cone, J."]);

        std::fs::write(&path, "@gender\n").unwrap();
        lib.touch_saved(&path).unwrap();
        assert_eq!(author_names(&lib), vec!["Butler, J."]);

        std::fs::write(&path, "no citations now\n").unwrap();
        lib.touch_saved(&path).unwrap();
        assert!(author_names(&lib).is_empty());
    }

    #[test]
    fn archived_and_trashed_documents_do_not_count_toward_an_author() {
        let (mut lib, work, _) = lib_with_bib();
        let a = lib
            .upsert_document(&write_doc(&work, "a.typ", "@gender\n"))
            .unwrap();
        let b = lib
            .upsert_document(&write_doc(&work, "b.typ", "@gender\n"))
            .unwrap();
        assert_eq!(lib.all_authors_with_counts().unwrap()[0].1, 2);

        lib.set_archived(a, true).unwrap();
        assert_eq!(lib.all_authors_with_counts().unwrap()[0].1, 1);
        lib.move_to_trash(b).unwrap();
        assert!(author_names(&lib).is_empty());
    }

    #[test]
    fn removing_a_document_clears_its_author_links() {
        let (mut lib, work, _) = lib_with_bib();
        let id = lib
            .upsert_document(&write_doc(&work, "a.typ", "@gender\n"))
            .unwrap();
        lib.remove_document(id).unwrap();
        // The link rows cascade; the author itself goes on the next sync.
        assert!(author_names(&lib).is_empty());
    }

    #[test]
    fn changing_the_bibliography_and_resyncing_updates_every_document() {
        let (mut lib, work, bib) = lib_with_bib();
        lib.upsert_document(&write_doc(&work, "a.typ", "@gender\n"))
            .unwrap();
        assert_eq!(author_names(&lib), vec!["Butler, J."]);

        let other = write_doc(
            &work,
            "other.bib",
            "@book{gender, author = {Arendt, Hannah}}",
        );
        assert!(lib.set_bibliography(Some(other.clone())));
        assert!(
            !lib.set_bibliography(Some(other)),
            "unchanged is not a change"
        );
        lib.resync_authors();
        assert_eq!(author_names(&lib), vec!["Arendt, H."]);
        let _ = bib;
    }

    #[test]
    fn a_document_with_no_bibliography_available_simply_has_no_authors() {
        let (mut lib, work) = fixture();
        lib.upsert_document(&write_doc(&work, "a.typ", "@gender\n"))
            .unwrap();
        assert!(author_names(&lib).is_empty());
    }

    #[test]
    fn authors_by_doc_groups_names_per_document() {
        let (mut lib, work, _) = lib_with_bib();
        let id = lib
            .upsert_document(&write_doc(&work, "a.typ", "@gender @black\n"))
            .unwrap();
        let map = lib.authors_by_doc().unwrap();
        assert_eq!(map[&id], vec!["Butler, J.", "Cone, J."]);
    }

    // ── text index and search ────────────────────────────────────────────

    fn search(lib: &Library, q: &str) -> Vec<i64> {
        ids(&lib
            .documents(LibraryFilter::Everywhere, q, SortOrder::Title)
            .unwrap())
    }

    #[test]
    fn search_finds_words_inside_a_document_including_as_you_type_prefixes() {
        let (mut lib, work) = fixture();
        let id = lib
            .upsert_document(&write_doc(
                &work,
                "advent.typ",
                "The Magnificat is sung here.\n",
            ))
            .unwrap();
        lib.upsert_document(&write_doc(&work, "other.typ", "Nothing relevant.\n"))
            .unwrap();
        assert_eq!(search(&lib, "magnificat"), vec![id]);
        assert_eq!(search(&lib, "magnif"), vec![id]);
        assert_eq!(
            search(&lib, "sung magnificat"),
            vec![id],
            "all words required"
        );
        assert!(search(&lib, "magnificat vespers").is_empty());
    }

    #[test]
    fn heading_markers_are_not_words() {
        assert_eq!(
            prose_words("== Second heading\nBody text\n"),
            vec!["Second", "heading", "Body", "text"]
        );
    }

    #[test]
    fn typst_code_and_comments_are_not_searchable_prose() {
        let (mut lib, work) = fixture();
        lib.upsert_document(&write_doc(
            &work,
            "a.typ",
            "#let secretvariable = 1\n// hiddencomment\nVisible words.\n```\ncodeblockword\n```\n",
        ))
        .unwrap();
        for hidden in ["secretvariable", "hiddencomment", "codeblockword"] {
            assert!(search(&lib, hidden).is_empty(), "{hidden}");
        }
        assert_eq!(search(&lib, "visible").len(), 1);
    }

    #[test]
    fn search_ignores_accents_in_either_direction() {
        let (mut lib, work) = fixture();
        let id = lib
            .upsert_document(&write_doc(&work, "a.typ", "Slavoj Žižek writes.\n"))
            .unwrap();
        assert_eq!(search(&lib, "zizek"), vec![id]);
        assert_eq!(search(&lib, "Žižek"), vec![id]);
    }

    #[test]
    fn search_covers_notes_titles_and_file_names_and_follows_their_edits() {
        let (mut lib, work) = fixture();
        let id = lib
            .upsert_document(&write_doc(&work, "draftsermon.typ", "Body.\n"))
            .unwrap();
        assert_eq!(search(&lib, "draftsermon"), vec![id], "file name");

        assert!(search(&lib, "isaiah").is_empty());
        lib.set_notes(id, Some("Check the Isaiah reading")).unwrap();
        assert_eq!(search(&lib, "isaiah"), vec![id], "notes");
        lib.set_notes(id, None).unwrap();
        assert!(
            search(&lib, "isaiah").is_empty(),
            "cleared notes stop matching"
        );

        lib.set_title(id, "Advent Lament").unwrap();
        assert_eq!(search(&lib, "lament"), vec![id], "renamed title");
        assert_eq!(
            search(&lib, "body"),
            vec![id],
            "the body survives a title change"
        );
    }

    #[test]
    fn folder_names_are_not_searchable() {
        let (mut lib, work) = fixture();
        std::fs::create_dir_all(work.path().join("quirkyfolder")).unwrap();
        let path = work.path().join("quirkyfolder").join("a.typ");
        std::fs::write(&path, "Text.\n").unwrap();
        lib.upsert_document(&path).unwrap();
        assert!(search(&lib, "quirkyfolder").is_empty());
        assert_eq!(search(&lib, "a.typ").len(), 1);
    }

    #[test]
    fn editing_the_file_updates_what_search_finds_and_the_word_count() {
        let (mut lib, work) = fixture();
        let path = write_doc(&work, "a.typ", "alpha beta gamma\n");
        let id = lib.upsert_document(&path).unwrap();
        assert_eq!(lib.doc_by_id(id).unwrap().unwrap().words, Some(3));
        assert_eq!(search(&lib, "alpha"), vec![id]);

        std::fs::write(&path, "delta epsilon\n").unwrap();
        lib.touch_saved(&path).unwrap();
        assert_eq!(lib.doc_by_id(id).unwrap().unwrap().words, Some(2));
        assert!(search(&lib, "alpha").is_empty());
        assert_eq!(search(&lib, "delta"), vec![id]);
    }

    #[test]
    fn an_unchanged_file_is_not_read_again_but_a_changed_one_is() {
        let (mut lib, work) = fixture();
        let path = write_doc(&work, "a.typ", "one two three\n");
        let id = lib.upsert_document(&path).unwrap();
        // Poison the stored stamp's words; an unchanged file must leave them.
        lib.conn
            .execute("UPDATE documents SET words = 99 WHERE id = ?1", params![id])
            .unwrap();
        lib.upsert_document(&path).unwrap();
        assert_eq!(lib.doc_by_id(id).unwrap().unwrap().words, Some(99));

        std::fs::write(&path, "one two\n").unwrap();
        lib.upsert_document(&path).unwrap();
        assert_eq!(lib.doc_by_id(id).unwrap().unwrap().words, Some(2));
    }

    #[test]
    fn a_missing_file_keeps_its_row_and_its_last_index() {
        let (mut lib, work) = fixture();
        let path = write_doc(&work, "a.typ", "remembered words\n");
        let id = lib.upsert_document(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        lib.upsert_document(&path).unwrap();
        assert_eq!(search(&lib, "remembered"), vec![id]);
        assert_eq!(lib.doc_by_id(id).unwrap().unwrap().words, Some(2));
    }

    #[test]
    fn moving_a_document_updates_its_file_name_in_the_index() {
        let (mut lib, work) = fixture();
        let id = lib
            .upsert_document(&write_doc(&work, "oldname.typ", "Body.\n"))
            .unwrap();
        // The title is its own thing; only the file name is under test.
        lib.set_title(id, "Sermon").unwrap();
        let new = write_doc(&work, "newname.typ", "Body.\n");
        lib.update_path(id, &new).unwrap();
        assert!(search(&lib, "oldname").is_empty());
        assert_eq!(search(&lib, "newname"), vec![id]);
    }

    #[test]
    fn removing_a_document_removes_it_from_the_index() {
        let (mut lib, work) = fixture();
        let id = lib
            .upsert_document(&write_doc(&work, "a.typ", "findable\n"))
            .unwrap();
        lib.remove_document(id).unwrap();
        let left: i64 = lib
            .conn
            .query_row("SELECT COUNT(*) FROM doc_text", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0);
    }

    #[test]
    fn prune_index_drops_rows_whose_document_is_gone() {
        let (mut lib, work) = fixture();
        lib.upsert_document(&write_doc(&work, "a.typ", "x\n"))
            .unwrap();
        lib.conn
            .execute(
                "INSERT INTO doc_text (rowid, title, notes, path, body) VALUES (999, 't', '', 'p', 'b')",
                [],
            )
            .unwrap();
        lib.prune_index();
        let n: i64 = lib
            .conn
            .query_row("SELECT COUNT(*) FROM doc_text", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn awkward_search_text_never_errors_and_percent_is_literal() {
        let (mut lib, work) = fixture();
        let plain = lib
            .upsert_document(&write_doc(&work, "plain.typ", "Just words.\n"))
            .unwrap();
        let pct = lib
            .upsert_document(&write_doc(&work, "pct.typ", "= Grew 100% in a year\n"))
            .unwrap();
        lib.set_title(pct, "Grew 100% in a year").unwrap();
        for q in [
            "\"",
            "(",
            "*",
            "OR",
            "NEAR(",
            "a AND",
            "'; DROP TABLE documents; --",
            ":",
            "-x",
        ] {
            lib.documents(LibraryFilter::All, q, SortOrder::Title)
                .unwrap_or_else(|e| panic!("search {q:?} failed: {e}"));
        }
        assert_eq!(search(&lib, "100%"), vec![pct]);
        assert!(
            !search(&lib, "%").contains(&plain),
            "a lone % is not a wildcard"
        );
        assert!(search(&lib, "_").is_empty());
    }

    #[test]
    fn snippets_exist_only_for_body_matches_and_mark_the_match() {
        let (mut lib, work) = fixture();
        let body = lib
            .upsert_document(&write_doc(
                &work,
                "a.typ",
                "Long before the dawn the Magnificat rose from the choir.\n",
            ))
            .unwrap();
        let titled = lib
            .upsert_document(&write_doc(&work, "b.typ", "Unrelated prose.\n"))
            .unwrap();
        lib.set_title(titled, "Magnificat in title only").unwrap();
        let snips = lib.snippets("magnificat");
        assert_eq!(snips.keys().copied().collect::<Vec<_>>(), vec![body]);
        let text = &snips[&body];
        assert!(
            text.contains(&format!("{MARK_START}Magnificat{MARK_END}")),
            "{text:?}"
        );
        assert!(lib.snippets("   ").is_empty());
    }

    #[test]
    fn snippet_markup_escapes_text_and_bolds_the_match() {
        let raw = format!("a < b & {MARK_START}c{MARK_END} > d");
        assert_eq!(snippet_to_markup(&raw), "a &lt; b &amp; <b>c</b> &gt; d");
    }

    #[test]
    fn the_search_box_becomes_prefix_terms_or_nothing() {
        assert_eq!(
            fts_query("gender trouble").as_deref(),
            Some("\"gender\"* \"trouble\"*")
        );
        assert_eq!(
            fts_query("Butler, J.").as_deref(),
            Some("\"Butler\"* \"J\"*")
        );
        assert_eq!(fts_query("  ...  "), None);
        assert_eq!(fts_query(""), None);
        assert_eq!(like_pattern("50%_\\"), "%50\\%\\_\\\\%");
    }

    #[test]
    fn everywhere_includes_archived_documents_but_not_trashed_ones() {
        let (mut lib, work) = fixture();
        let (live, _) = add_doc(&mut lib, &work, "live.typ");
        let (old, _) = add_doc(&mut lib, &work, "old.typ");
        let (gone, _) = add_doc(&mut lib, &work, "gone.typ");
        lib.set_archived(old, true).unwrap();
        lib.move_to_trash(gone).unwrap();
        let mut got = search(&lib, "");
        got.sort();
        assert_eq!(got, vec![live, old]);
    }

    #[test]
    fn import_skips_the_templates_folder_at_the_top_but_not_deeper_ones() {
        let (mut lib, work) = fixture();
        std::fs::create_dir_all(work.path().join("Templates")).unwrap();
        std::fs::create_dir_all(work.path().join("Thesis/Templates")).unwrap();
        std::fs::write(work.path().join("Templates/starter.typ"), "x").unwrap();
        std::fs::write(work.path().join("Thesis/Templates/mine.typ"), "x").unwrap();
        std::fs::write(work.path().join("real.typ"), "x").unwrap();
        lib.import_directory(work.path()).unwrap();
        let titles = titles(
            &lib.documents(LibraryFilter::All, "", SortOrder::Title)
                .unwrap(),
        );
        assert_eq!(titles, vec!["mine", "real"]);
    }

    // ── counts and bulk lookups ──────────────────────────────────────────

    #[test]
    fn counts_of_an_empty_library_are_zero_not_an_error() {
        let (lib, _work) = fixture();
        let c = lib.counts().unwrap();
        assert_eq!(
            (c.all, c.archive, c.trash, c.recent, c.unlabelled),
            (0, 0, 0, 0, 0)
        );
        assert!(c.projects.is_empty());
    }

    #[test]
    fn recent_is_capped_at_thirty_in_both_the_list_and_its_count() {
        let (mut lib, work) = fixture();
        for i in 0..35 {
            let p = write_doc(&work, &format!("r{i}.typ"), "x\n");
            lib.touch_opened(&p).unwrap();
        }
        let listed = lib
            .documents(LibraryFilter::Recent, "", SortOrder::Opened)
            .unwrap()
            .len();
        assert_eq!(listed, 30);
        assert_eq!(lib.doc_count(&LibraryFilter::Recent).unwrap(), 30);
        assert_eq!(lib.counts().unwrap().recent, 30);
    }

    // ── labels ───────────────────────────────────────────────────────────

    fn label_names(lib: &Library, doc: i64) -> Vec<String> {
        lib.labels_by_doc()
            .unwrap()
            .remove(&doc)
            .unwrap_or_default()
            .into_iter()
            .map(|l| l.name)
            .collect()
    }

    #[test]
    fn a_label_is_made_once_whatever_its_case() {
        let (mut lib, _work) = fixture();
        let a = lib.create_label("Sermons").unwrap();
        let b = lib.create_label("  sermons ").unwrap();
        assert_eq!(a, b);
        assert_eq!(lib.all_labels().unwrap().len(), 1);
        assert_eq!(
            lib.all_labels().unwrap()[0].name,
            "Sermons",
            "first spelling wins"
        );
    }

    #[test]
    fn labels_are_added_and_removed_across_many_documents_without_disturbing_others() {
        let (mut lib, work) = fixture();
        let (a, _) = add_doc(&mut lib, &work, "a.typ");
        let (b, _) = add_doc(&mut lib, &work, "b.typ");
        let (c, _) = add_doc(&mut lib, &work, "c.typ");
        let draft = lib.create_label("draft").unwrap();
        let advent = lib.create_label("Advent").unwrap();
        lib.add_labels(&[a], &[advent]).unwrap();

        lib.add_labels(&[a, b], &[draft]).unwrap();
        lib.add_labels(&[a, b], &[draft]).unwrap(); // idempotent
        assert_eq!(label_names(&lib, a), vec!["Advent", "draft"]);
        assert_eq!(label_names(&lib, b), vec!["draft"]);
        assert!(label_names(&lib, c).is_empty());

        lib.remove_labels(&[a, c], &[draft]).unwrap();
        assert_eq!(label_names(&lib, a), vec!["Advent"]);
        assert_eq!(
            label_names(&lib, b),
            vec!["draft"],
            "b was not in the removal"
        );
    }

    #[test]
    fn membership_counts_say_how_many_of_the_chosen_documents_have_each() {
        let (mut lib, work) = fixture();
        let (a, _) = add_doc(&mut lib, &work, "a.typ");
        let (b, _) = add_doc(&mut lib, &work, "b.typ");
        let (c, _) = add_doc(&mut lib, &work, "c.typ");
        let all = lib.create_label("all").unwrap();
        let some = lib.create_label("some").unwrap();
        lib.add_labels(&[a, b], &[all]).unwrap();
        lib.add_labels(&[a], &[some]).unwrap();
        lib.add_labels(&[c], &[some]).unwrap(); // not among the chosen
        let m = lib.label_memberships(&[a, b]).unwrap();
        assert_eq!(m[&all], 2);
        assert_eq!(m[&some], 1);
        assert!(lib.label_memberships(&[]).unwrap().is_empty());
        let project = lib.create_project("P").unwrap();
        lib.add_doc_to_project(project, a).unwrap();
        assert_eq!(lib.project_memberships(&[a, b]).unwrap()[&project], 1);
    }

    #[test]
    fn deleting_a_label_takes_it_off_every_document_but_keeps_the_documents() {
        let (mut lib, work) = fixture();
        let (a, _) = add_doc(&mut lib, &work, "a.typ");
        let l = lib.create_label("temp").unwrap();
        lib.add_labels(&[a], &[l]).unwrap();
        lib.delete_label(l).unwrap();
        assert!(label_names(&lib, a).is_empty());
        assert!(lib.doc_by_id(a).unwrap().is_some());
    }

    #[test]
    fn renaming_a_label_keeps_its_documents_and_refuses_a_name_already_taken() {
        let (mut lib, work) = fixture();
        let (a, _) = add_doc(&mut lib, &work, "a.typ");
        let one = lib.create_label("one").unwrap();
        let two = lib.create_label("two").unwrap();
        lib.add_labels(&[a], &[one]).unwrap();
        lib.rename_label(one, "Uno").unwrap();
        assert_eq!(label_names(&lib, a), vec!["Uno"]);
        assert!(lib.rename_label(two, "uno").is_err(), "names ignore case");
    }

    #[test]
    fn a_label_colour_can_be_set_and_handed_back_to_automatic() {
        let (mut lib, _work) = fixture();
        let l = lib.create_label("x").unwrap();
        assert_eq!(lib.all_labels().unwrap()[0].color_hex, None);
        lib.set_label_color(l, Some("#e01b24")).unwrap();
        assert_eq!(
            lib.all_labels().unwrap()[0].color_hex.as_deref(),
            Some("#e01b24")
        );
        lib.set_label_color(l, None).unwrap();
        assert_eq!(lib.all_labels().unwrap()[0].color_hex, None);
    }

    #[test]
    fn the_label_filter_unlabelled_view_and_search_all_agree_with_the_labels() {
        let (mut lib, work) = fixture();
        let (a, _) = add_doc(&mut lib, &work, "a.typ");
        let (b, _) = add_doc(&mut lib, &work, "b.typ");
        let l = lib.create_label("Sermons").unwrap();
        lib.add_labels(&[a], &[l]).unwrap();

        let labelled = lib
            .documents(LibraryFilter::Label(l), "", SortOrder::Title)
            .unwrap();
        assert_eq!(ids(&labelled), vec![a]);
        let unlabelled = lib
            .documents(LibraryFilter::Unlabelled, "", SortOrder::Title)
            .unwrap();
        assert_eq!(ids(&unlabelled), vec![b]);
        assert_eq!(
            search(&lib, "sermons"),
            vec![a],
            "a label's name is searchable"
        );

        let c = lib.counts().unwrap();
        assert_eq!(
            c.labels[&l],
            lib.doc_count(&LibraryFilter::Label(l)).unwrap()
        );
        assert_eq!(
            c.unlabelled,
            lib.doc_count(&LibraryFilter::Unlabelled).unwrap()
        );
    }

    /// The sidebar's grouped counts and `doc_count` (and so the list a row
    /// opens) must always agree.
    #[test]
    fn grouped_counts_agree_with_the_count_of_each_view() {
        let (mut lib, work, _) = lib_with_bib();
        let mut docs = Vec::new();
        for (i, body) in [
            "@gender",
            "@black @gender",
            "plain",
            "@black",
            "plain",
            "plain",
        ]
        .iter()
        .enumerate()
        {
            let path = write_doc(&work, &format!("d{i}.typ"), &format!("{body}\n"));
            docs.push(lib.upsert_document(&path).unwrap());
            lib.touch_opened(&path).ok();
        }
        let project = lib.create_project("Thesis").unwrap();
        lib.add_doc_to_project(project, docs[0]).unwrap();
        lib.add_doc_to_project(project, docs[2]).unwrap();
        let draft = lib.create_label("draft").unwrap();
        let advent = lib.create_label("Advent").unwrap();
        lib.add_labels(&[docs[1], docs[3]], &[draft]).unwrap();
        lib.add_labels(&[docs[0], docs[1]], &[advent]).unwrap();
        lib.set_archived(docs[4], true).unwrap();
        lib.move_to_trash(docs[5]).unwrap();

        let c = lib.counts().unwrap();
        for (f, n) in [
            (LibraryFilter::All, c.all),
            (LibraryFilter::Archive, c.archive),
            (LibraryFilter::Trash, c.trash),
            (LibraryFilter::Recent, c.recent),
            (LibraryFilter::Unlabelled, c.unlabelled),
            (LibraryFilter::Project(project), c.projects[&project]),
            (LibraryFilter::Label(draft), c.labels[&draft]),
            (LibraryFilter::Label(advent), c.labels[&advent]),
        ] {
            assert_eq!(n, lib.doc_count(&f).unwrap(), "{f:?}");
        }
        for (a, n) in lib.all_authors_with_counts().unwrap() {
            assert_eq!(c.authors[&a.id], n);
            assert_eq!(n, lib.doc_count(&LibraryFilter::Author(a.id)).unwrap());
        }
    }

    #[test]
    fn bulk_label_lookup_matches_the_per_document_view() {
        let (mut lib, work) = fixture();
        let (a, _) = add_doc(&mut lib, &work, "a.typ");
        let (b, _) = add_doc(&mut lib, &work, "b.typ");
        let z = lib.create_label("zeta").unwrap();
        let y = lib.create_label("alpha").unwrap();
        lib.add_labels(&[a], &[z, y]).unwrap();
        let by = lib.labels_by_doc().unwrap();
        assert_eq!(label_names(&lib, a), vec!["alpha", "zeta"]);
        assert!(!by.contains_key(&b));
    }

    // ── tags and categories becoming labels ──────────────────────────────

    /// An old-style library: documents, tags and categories, with the labels
    /// tables not yet filled in. Built by hand so the shapes are exactly what
    /// earlier versions wrote.
    fn legacy_library(work: &TempDir) -> Library {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        conn.execute_batch(
            "CREATE TABLE documents (
                 id INTEGER PRIMARY KEY AUTOINCREMENT, path TEXT NOT NULL UNIQUE,
                 title TEXT NOT NULL, category TEXT, archived INTEGER NOT NULL DEFAULT 0,
                 notes TEXT, created_at TEXT NOT NULL, modified_at TEXT NOT NULL,
                 last_opened_at TEXT);
             CREATE TABLE tags (
                 id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE,
                 color_hex TEXT NOT NULL DEFAULT '#3584e4');
             CREATE TABLE doc_tags (
                 doc_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                 tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
                 PRIMARY KEY (doc_id, tag_id));
             CREATE TABLE categories (
                 name TEXT NOT NULL PRIMARY KEY,
                 color_hex TEXT NOT NULL DEFAULT '#3584e4',
                 parent TEXT REFERENCES categories(name));
             CREATE TABLE doc_categories (
                 doc_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                 category TEXT NOT NULL REFERENCES categories(name) ON DELETE CASCADE,
                 PRIMARY KEY (doc_id, category));",
        )
        .unwrap();
        for (i, name) in ["a", "b", "c", "d"].iter().enumerate() {
            conn.execute(
                "INSERT INTO documents (path, title, created_at, modified_at) VALUES (?1, ?2, 't', 't')",
                params![work.path().join(format!("{name}.typ")).to_string_lossy(), name],
            )
            .unwrap();
            let _ = i;
        }
        Library {
            conn,
            trash_dir: work.path().join("trash"),
            authors: AuthorSync::default(),
        }
    }

    fn labels_of(lib: &Library, doc: i64) -> Vec<String> {
        label_names(lib, doc)
    }

    #[test]
    fn tags_and_categories_become_labels_carrying_the_same_documents() {
        let work = TempDir::new().unwrap();
        let lib = legacy_library(&work);
        lib.conn.execute_batch(
            "INSERT INTO tags (name, color_hex) VALUES ('draft', '#e01b24'), ('plain', '#3584e4');
             INSERT INTO doc_tags VALUES (1, 1), (2, 1), (2, 2);
             INSERT INTO categories (name, color_hex, parent) VALUES
                 ('Liturgy', '#3584e4', NULL), ('Advent', '#33d17a', 'Liturgy'),
                 ('Papers', '#3584e4', NULL);
             INSERT INTO doc_categories VALUES (1, 'Advent'), (3, 'Papers');",
        ).unwrap();
        lib.migrate().unwrap();

        assert_eq!(labels_of(&lib, 1), vec!["draft", "Liturgy › Advent"]);
        assert_eq!(labels_of(&lib, 2), vec!["draft", "plain"]);
        assert_eq!(labels_of(&lib, 3), vec!["Papers"]);
        assert!(labels_of(&lib, 4).is_empty());

        let by_name: std::collections::HashMap<String, Option<String>> = lib
            .all_labels()
            .unwrap()
            .into_iter()
            .map(|l| (l.name, l.color_hex))
            .collect();
        assert_eq!(
            by_name["draft"].as_deref(),
            Some("#e01b24"),
            "a chosen tag colour is kept"
        );
        assert_eq!(
            by_name["plain"], None,
            "the old default blue was never chosen"
        );
        assert_eq!(by_name["Papers"], None, "nor was a category's");
        assert_eq!(by_name["Liturgy › Advent"].as_deref(), Some("#33d17a"));
    }

    #[test]
    fn a_parent_category_that_only_grouped_its_children_is_not_a_label_of_its_own() {
        let work = TempDir::new().unwrap();
        let lib = legacy_library(&work);
        lib.conn.execute_batch(
            "INSERT INTO categories (name, parent) VALUES ('Liturgy', NULL), ('Advent', 'Liturgy'),
                 ('Lent', 'Liturgy'), ('Empty', NULL);
             INSERT INTO doc_categories VALUES (1, 'Advent');",
        ).unwrap();
        lib.migrate().unwrap();
        let names: Vec<String> = lib
            .all_labels()
            .unwrap()
            .into_iter()
            .map(|l| l.name)
            .collect();
        assert_eq!(names, vec!["Empty", "Liturgy › Advent", "Liturgy › Lent"]);
    }

    #[test]
    fn a_parent_that_also_held_documents_keeps_its_own_label_beside_its_children() {
        let work = TempDir::new().unwrap();
        let lib = legacy_library(&work);
        lib.conn.execute_batch(
            "INSERT INTO categories (name, parent) VALUES ('Liturgy', NULL), ('Advent', 'Liturgy');
             INSERT INTO doc_categories VALUES (1, 'Liturgy'), (1, 'Advent');",
        ).unwrap();
        lib.migrate().unwrap();
        assert_eq!(labels_of(&lib, 1), vec!["Liturgy", "Liturgy › Advent"]);
    }

    #[test]
    fn a_tag_and_a_category_with_one_name_become_one_label_with_the_chosen_colour() {
        let work = TempDir::new().unwrap();
        let lib = legacy_library(&work);
        lib.conn
            .execute_batch(
                "INSERT INTO tags (name) VALUES ('Sermons');
             INSERT INTO doc_tags VALUES (1, 1);
             INSERT INTO categories (name, color_hex) VALUES ('sermons', '#9141ac');
             INSERT INTO doc_categories VALUES (2, 'sermons');",
            )
            .unwrap();
        lib.migrate().unwrap();
        let labels = lib.all_labels().unwrap();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].color_hex.as_deref(), Some("#9141ac"));
        assert_eq!(labels_of(&lib, 1), vec!["Sermons"]);
        assert_eq!(labels_of(&lib, 2), vec!["Sermons"]);
    }

    #[test]
    fn the_migration_runs_once_and_leaves_the_old_tables_alone() {
        let work = TempDir::new().unwrap();
        let mut lib = legacy_library(&work);
        lib.conn
            .execute_batch(
                "INSERT INTO tags (name) VALUES ('draft'); INSERT INTO doc_tags VALUES (1, 1);",
            )
            .unwrap();
        lib.migrate().unwrap();
        // The user then renames the label; migrating again must not undo that
        // or bring the old tag back.
        let id = lib.all_labels().unwrap()[0].id;
        lib.rename_label(id, "Draft v2").unwrap();
        lib.migrate().unwrap();
        lib.migrate().unwrap();
        assert_eq!(lib.all_labels().unwrap().len(), 1);
        assert_eq!(lib.all_labels().unwrap()[0].name, "Draft v2");
        let old: i64 = lib
            .conn
            .query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0))
            .unwrap();
        assert_eq!(old, 1, "the old table is kept as it was");
        assert!(labels_migrated(&lib.conn));
    }

    #[test]
    fn a_title_chosen_before_titles_could_be_protected_survives_the_upgrade() {
        // An old library: no title_custom column. One document has a title that
        // differs from its file's (the user renamed it), one has the file's own.
        let work = TempDir::new().unwrap();
        let renamed = work.path().join("ch1.typ");
        let plain = work.path().join("ch2.typ");
        std::fs::write(&renamed, "= One\n").unwrap();
        std::fs::write(&plain, "= Two\n").unwrap();
        let mut lib = legacy_library(&work);
        lib.conn.execute_batch("DELETE FROM documents;").unwrap();
        for (path, title) in [(&renamed, "My Chapter One"), (&plain, "ch2")] {
            lib.conn
                .execute(
                    "INSERT INTO documents (path, title, created_at, modified_at) VALUES (?1, ?2, 't', 't')",
                    params![path.to_string_lossy(), title],
                )
                .unwrap();
        }
        lib.migrate().unwrap();
        // The first scan after upgrading — which used to reset the title.
        lib.upsert_document(&renamed).unwrap();
        lib.upsert_document(&plain).unwrap();
        fn title(lib: &Library, p: &Path) -> String {
            lib.doc_by_path(p).unwrap().unwrap().title
        }
        assert_eq!(title(&lib, &renamed), "My Chapter One");
        assert_eq!(title(&lib, &plain), "ch2");
        // A title from the file still follows the file.
        std::fs::write(&plain, "#let doc-title = \"Retitled\"\n").unwrap();
        lib.upsert_document(&plain).unwrap();
        assert_eq!(title(&lib, &plain), "Retitled");
    }

    #[test]
    fn a_new_library_starts_with_the_migration_done_and_no_labels() {
        let (lib, _work) = fixture();
        assert!(labels_migrated(&lib.conn));
        assert!(lib.all_labels().unwrap().is_empty());
    }

    #[test]
    fn the_pending_check_sees_an_old_library_and_not_a_new_or_finished_one() {
        let work = TempDir::new().unwrap();
        let lib = legacy_library(&work);
        assert!(labels_migration_pending(&lib.conn));
        lib.migrate().unwrap();
        assert!(!labels_migration_pending(&lib.conn));
        let (fresh, _w) = fixture();
        assert!(!labels_migration_pending(&fresh.conn));
    }

    #[test]
    fn migrating_a_real_file_database_with_wal_keeps_everything() {
        let work = TempDir::new().unwrap();
        let path = work.path().join("library.sqlite");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
                .unwrap();
            conn.execute_batch(
                "CREATE TABLE documents (
                     id INTEGER PRIMARY KEY AUTOINCREMENT, path TEXT NOT NULL UNIQUE,
                     title TEXT NOT NULL, category TEXT, archived INTEGER NOT NULL DEFAULT 0,
                     notes TEXT, created_at TEXT NOT NULL, modified_at TEXT NOT NULL,
                     last_opened_at TEXT);
                 CREATE TABLE tags (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE,
                     color_hex TEXT NOT NULL DEFAULT '#3584e4');
                 CREATE TABLE doc_tags (doc_id INTEGER NOT NULL, tag_id INTEGER NOT NULL,
                     PRIMARY KEY (doc_id, tag_id));
                 INSERT INTO documents (path, title, created_at, modified_at) VALUES ('/x.typ', 'x', 't', 't');
                 INSERT INTO tags (name) VALUES ('keep');
                 INSERT INTO doc_tags VALUES (1, 1);",
            ).unwrap();
        }
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")
            .unwrap();
        let lib = Library {
            conn,
            trash_dir: work.path().join("t"),
            authors: AuthorSync::default(),
        };
        lib.migrate().unwrap();
        assert_eq!(label_names(&lib, 1), vec!["keep"]);
    }
}

#[cfg(test)]
mod sql_shape {
    use super::*;

    fn sql_for(filter: LibraryFilter, sort: SortOrder) -> String {
        let q = filter.query();
        let idx = if q.param.is_some() { 2 } else { 1 };
        format!(
            "SELECT {} FROM {} WHERE {} AND {} ORDER BY {}",
            q.select,
            q.from,
            q.conditions,
            search_clause(q.prefix, idx, true),
            q.order(&sort)
        ) + &q.limit_clause()
    }

    /// Filters that take a leading parameter must bind the search pattern to
    /// `?2`; the rest to `?1`. Getting this wrong silently searches for the
    /// label/project id instead of the user's text.
    #[test]
    fn search_pattern_takes_the_slot_after_any_leading_parameter() {
        for f in [
            LibraryFilter::All,
            LibraryFilter::Archive,
            LibraryFilter::Unlabelled,
            LibraryFilter::Recent,
            LibraryFilter::Trash,
        ] {
            let sql = sql_for(f.clone(), SortOrder::Modified);
            assert!(
                sql.contains("title LIKE ?1"),
                "{f:?} should bind search to ?1"
            );
        }
        for f in [
            LibraryFilter::Label(3),
            LibraryFilter::Author(5),
            LibraryFilter::Project(7),
        ] {
            let sql = sql_for(f.clone(), SortOrder::Modified);
            assert!(sql.contains("LIKE ?2"), "{f:?} should bind search to ?2");
        }
    }

    #[test]
    fn joined_filters_prefix_every_column_with_the_table_alias() {
        for f in [LibraryFilter::Project(7), LibraryFilter::Label(9)] {
            let sql = sql_for(f.clone(), SortOrder::Modified);
            assert!(sql.contains("FROM documents d JOIN"), "{f:?}");
            assert!(
                sql.contains("SELECT d.id,"),
                "{f:?} must select prefixed columns"
            );
            assert!(
                sql.contains("d.title LIKE ?2"),
                "{f:?} must prefix the search clause"
            );
            assert!(sql.contains("ORDER BY d.pinned DESC"), "{f:?}");
        }
    }

    /// Three filters ignore the caller's sort entirely.
    #[test]
    fn fixed_orderings_override_the_requested_sort() {
        let project = sql_for(LibraryFilter::Project(7), SortOrder::Title);
        assert!(project.ends_with("ORDER BY d.pinned DESC, pd.position, d.title"));

        let recent = sql_for(LibraryFilter::Recent, SortOrder::Title);
        assert!(recent.ends_with("ORDER BY pinned DESC, last_opened_at DESC LIMIT 30"));

        let trash = sql_for(LibraryFilter::Trash, SortOrder::Title);
        assert!(trash.ends_with("ORDER BY modified_at DESC"));
    }

    #[test]
    fn the_other_filters_honour_the_requested_sort_behind_pinned() {
        for f in [
            LibraryFilter::All,
            LibraryFilter::Archive,
            LibraryFilter::Unlabelled,
            LibraryFilter::Label(9),
        ] {
            for (sort, tail) in [
                (SortOrder::Title, "title COLLATE NOCASE ASC"),
                (SortOrder::Created, "created_at DESC"),
                (SortOrder::Opened, "last_opened_at DESC NULLS LAST"),
            ] {
                let sql = sql_for(f.clone(), sort.clone());
                assert!(
                    sql.contains("pinned DESC, "),
                    "{f:?} must keep pinned first"
                );
                assert!(sql.ends_with(tail), "{f:?} with {sort:?} should end {tail}");
            }
        }
    }

    /// Each filter's defining condition, so a mis-shuffled descriptor is caught.
    #[test]
    fn each_filter_keeps_its_own_conditions() {
        let cases = [
            (LibraryFilter::All, "archived = 0 AND deleted = 0"),
            (LibraryFilter::Archive, "archived = 1 AND deleted = 0"),
            (LibraryFilter::Trash, "deleted = 1"),
            (
                LibraryFilter::Unlabelled,
                "id NOT IN (SELECT DISTINCT doc_id FROM doc_labels)",
            ),
            (LibraryFilter::Recent, "last_opened_at IS NOT NULL"),
            (
                LibraryFilter::Project(7),
                "pd.project_id = ?1 AND d.deleted = 0",
            ),
            (LibraryFilter::Label(9), "dl.label_id = ?1"),
        ];
        for (f, needle) in cases {
            let sql = sql_for(f.clone(), SortOrder::Modified);
            assert!(
                sql.contains(needle),
                "{f:?} should contain {needle:?}\n{sql}"
            );
        }
    }
}
