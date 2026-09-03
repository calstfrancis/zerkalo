use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};

pub struct Library {
    conn: Connection,
    /// Where `move_to_trash` parks deleted files. A field rather than a direct
    /// `glib::user_data_dir()` call so tests can point it at a temp dir instead
    /// of the real data dir.
    trash_dir: PathBuf,
}

fn default_trash_dir() -> PathBuf {
    crate::config::zerkalo_data_dir().join("trash")
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
}

#[derive(Clone, Debug)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color_hex: String,
}

#[derive(Clone, Debug)]
#[allow(dead_code)] // mirrors the projects table; not every column is read yet
pub struct Project {
    pub id: i64,
    pub name: String,
    pub root_doc_id: Option<i64>,
    pub created_at: String,
}
#[derive(Clone, Debug)]
pub struct Category {
    pub name: String,
    /// `None` when no colour has been explicitly chosen — callers substitute a
    /// per-name palette colour so distinct categories stay distinct.
    pub color_hex: Option<String>,
    pub parent: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LibraryFilter {
    All,
    Project(i64),
    Tag(i64),
    Category(String),
    CategoryGroup(String),
    Archive,
    Recent,
    Untagged,
    Trash,
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
    param: Option<rusqlite::types::Value>,
}

impl FilterSpec {
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
            param: None,
        };
        match self {
            LibraryFilter::All => plain("archived = 0 AND deleted = 0"),
            LibraryFilter::Archive => plain("archived = 1 AND deleted = 0"),
            LibraryFilter::Untagged => plain(
                "archived = 0 AND deleted = 0 \
                 AND id NOT IN (SELECT DISTINCT doc_id FROM doc_tags)",
            ),
            LibraryFilter::Recent => FilterSpec {
                order_override: Some("pinned DESC, last_opened_at DESC LIMIT 30"),
                ..plain("last_opened_at IS NOT NULL AND archived = 0 AND deleted = 0")
            },
            LibraryFilter::Trash => FilterSpec {
                order_override: Some("modified_at DESC"),
                ..plain("deleted = 1")
            },
            // `EXISTS` rather than a `doc_categories` join: a document can now
            // carry more than one category (including a parent and one of its
            // own children at once), so a join could match the same document
            // through more than one row and duplicate it in the results.
            LibraryFilter::Category(cat) => FilterSpec {
                param: Some(cat.into()),
                ..plain(
                    "EXISTS (SELECT 1 FROM doc_categories dc \
                         WHERE dc.doc_id = id AND dc.category = ?1) \
                     AND archived = 0 AND deleted = 0",
                )
            },
            LibraryFilter::CategoryGroup(parent) => FilterSpec {
                param: Some(parent.into()),
                ..plain(
                    "EXISTS (SELECT 1 FROM doc_categories dc \
                         WHERE dc.doc_id = id AND dc.category IN (\
                             SELECT name FROM categories WHERE name = ?1 OR parent = ?1\
                         )) AND archived = 0 AND deleted = 0",
                )
            },
            LibraryFilter::Project(pid) => FilterSpec {
                prefix: "d.",
                select: doc_cols_prefixed("d"),
                from: "documents d JOIN project_docs pd ON pd.doc_id = d.id".to_string(),
                conditions: "pd.project_id = ?1 AND d.deleted = 0".to_string(),
                order_override: Some("d.pinned DESC, pd.position, d.title"),
                param: Some(pid.into()),
            },
            LibraryFilter::Tag(tid) => FilterSpec {
                prefix: "d.",
                select: doc_cols_prefixed("d"),
                from: "documents d JOIN doc_tags dt ON dt.doc_id = d.id".to_string(),
                conditions: "dt.tag_id = ?1 AND d.archived = 0 AND d.deleted = 0".to_string(),
                order_override: None,
                param: Some(tid.into()),
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
    "id, path, title, archived, pinned, notes, created_at, modified_at, last_opened_at";

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
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        let lib = Self {
            conn,
            trash_dir: default_trash_dir(),
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
        let lib = Self { conn, trash_dir };
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

        let existing: Option<i64> = self
            .conn
            .query_row(
                &format!("SELECT id FROM documents WHERE path = ?1{PATH_COLLATE}"),
                params![path_str],
                |r| r.get(0),
            )
            .optional()?;

        if let Some(id) = existing {
            self.conn.execute(
                "UPDATE documents SET modified_at = ?1, title = ?2 WHERE id = ?3",
                params![fs_modified, title, id],
            )?;
            Ok(id)
        } else {
            self.conn.execute(
                "INSERT INTO documents (path, title, created_at, modified_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![path_str, title, fs_created, fs_modified],
            )?;
            Ok(self.conn.last_insert_rowid())
        }
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
        let search_pat = if search.is_empty() {
            "%".to_string()
        } else {
            format!("%{}%", search)
        };

        // Every filter is the same query with five slots swapped: which columns
        // to select, what to join, the conditions before the search clause, the
        // ordering, and an optional leading parameter. This used to be nine
        // near-identical arms, each preparing and draining its own statement.
        let q = filter.query();
        let search_idx = if q.param.is_some() { 2 } else { 1 };
        let sql = format!(
            "SELECT {} FROM {} WHERE {} AND {} ORDER BY {}",
            q.select,
            q.from,
            q.conditions,
            search_clause(q.prefix, search_idx),
            q.order(&sort),
        );

        let mut args: Vec<rusqlite::types::Value> = Vec::new();
        if let Some(v) = q.param {
            args.push(v);
        }
        args.push(search_pat.into());

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(args), row_to_doc)?;
        let mut docs = Vec::new();
        for r in rows {
            docs.push(r?);
        }
        Ok(docs)
    }

    pub fn doc_count(&self, filter: &LibraryFilter) -> SqlResult<i64> {
        match filter {
            LibraryFilter::All => self.conn.query_row(
                "SELECT COUNT(*) FROM documents WHERE archived=0 AND deleted=0",
                [],
                |r| r.get(0),
            ),
            LibraryFilter::Archive => self.conn.query_row(
                "SELECT COUNT(*) FROM documents WHERE archived=1 AND deleted=0",
                [],
                |r| r.get(0),
            ),
            LibraryFilter::Project(pid) => self.conn.query_row(
                "SELECT COUNT(*) FROM project_docs pd JOIN documents d ON d.id=pd.doc_id WHERE pd.project_id=?1 AND d.deleted=0",
                params![pid],
                |r| r.get(0),
            ),
            LibraryFilter::Tag(tid) => self.conn.query_row(
                "SELECT COUNT(*) FROM doc_tags JOIN documents d ON d.id=doc_id WHERE tag_id=?1 AND d.archived=0 AND d.deleted=0",
                params![tid],
                |r| r.get(0),
            ),
            LibraryFilter::Category(cat) => self.conn.query_row(
                "SELECT COUNT(*) FROM documents WHERE archived=0 AND deleted=0
                 AND EXISTS (SELECT 1 FROM doc_categories dc WHERE dc.doc_id=documents.id AND dc.category=?1)",
                params![cat],
                |r| r.get(0),
            ),
            LibraryFilter::CategoryGroup(parent) => self.conn.query_row(
                "SELECT COUNT(*) FROM documents WHERE archived=0 AND deleted=0
                 AND EXISTS (
                     SELECT 1 FROM doc_categories dc WHERE dc.doc_id=documents.id
                     AND dc.category IN (SELECT name FROM categories WHERE name=?1 OR parent=?1)
                 )",
                params![parent],
                |r| r.get(0),
            ),
            LibraryFilter::Recent => self.conn.query_row(
                "SELECT COUNT(*) FROM (SELECT 1 FROM documents WHERE last_opened_at IS NOT NULL AND archived=0 AND deleted=0 ORDER BY last_opened_at DESC LIMIT 30)",
                [],
                |r| r.get(0),
            ),
            LibraryFilter::Untagged => self.conn.query_row(
                "SELECT COUNT(*) FROM documents WHERE archived=0 AND deleted=0 AND id NOT IN (SELECT DISTINCT doc_id FROM doc_tags)",
                [],
                |r| r.get(0),
            ),
            LibraryFilter::Trash => self.conn.query_row(
                "SELECT COUNT(*) FROM documents WHERE deleted=1",
                [],
                |r| r.get(0),
            ),
        }
    }

    pub fn doc_tags(&self, doc_id: i64) -> SqlResult<Vec<Tag>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.name, t.color_hex FROM tags t
             JOIN doc_tags dt ON dt.tag_id = t.id
             WHERE dt.doc_id = ?1 ORDER BY t.name",
        )?;
        let rows = stmt.query_map(params![doc_id], |r| {
            Ok(Tag {
                id: r.get(0)?,
                name: r.get(1)?,
                color_hex: r.get(2)?,
            })
        })?;
        let mut tags = Vec::new();
        for r in rows {
            tags.push(r?);
        }
        Ok(tags)
    }

    pub fn set_title(&mut self, doc_id: i64, title: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE documents SET title = ?1 WHERE id = ?2",
            params![title, doc_id],
        )?;
        Ok(())
    }

    /// Categories currently assigned to a document, ordered like
    /// `all_categories_structured` (parents before children, then by name).
    pub fn doc_categories(&self, doc_id: i64) -> SqlResult<Vec<Category>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.name, c.color_hex, c.parent FROM categories c
             JOIN doc_categories dc ON dc.category = c.name
             WHERE dc.doc_id = ?1 ORDER BY c.parent NULLS FIRST, c.name",
        )?;
        let rows = stmt.query_map(params![doc_id], |r| {
            Ok(Category {
                name: r.get(0)?,
                color_hex: r.get(1)?,
                parent: r.get(2)?,
            })
        })?;
        let mut cats = Vec::new();
        for r in rows {
            cats.push(r?);
        }
        Ok(cats)
    }

    /// Replaces a document's full set of categories (mirrors `set_doc_tags`).
    pub fn set_doc_categories(&mut self, doc_id: i64, names: &[String]) -> SqlResult<()> {
        self.conn.execute(
            "DELETE FROM doc_categories WHERE doc_id = ?1",
            params![doc_id],
        )?;
        self.add_doc_categories(doc_id, names)
    }

    /// Adds categories to a document without disturbing any it already has
    /// (mirrors `add_doc_tags`) — used by drag-and-drop-to-category and bulk
    /// assignment, where dropping onto a category should add it, not replace
    /// whatever the document already belongs to.
    pub fn add_doc_categories(&mut self, doc_id: i64, names: &[String]) -> SqlResult<()> {
        for name in names {
            self.ensure_category(name)?;
            self.conn.execute(
                "INSERT OR IGNORE INTO doc_categories (doc_id, category) VALUES (?1, ?2)",
                params![doc_id, name],
            )?;
        }
        Ok(())
    }

    pub fn set_pinned(&mut self, doc_id: i64, pinned: bool) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE documents SET pinned=?1 WHERE id=?2",
            params![pinned as i64, doc_id],
        )?;
        Ok(())
    }

    /// Returns the saved color for a category, or `None` if it has never had
    /// one assigned. Callers decide the fallback (e.g. a palette color keyed
    /// off the category name) rather than baking in a single fixed color.
    pub fn get_category_color(&self, name: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT color_hex FROM categories WHERE name=?1",
                params![name],
                |r| r.get::<_, Option<String>>(0),
            )
            .ok()
            .flatten()
    }

    pub fn set_category_color(&mut self, name: &str, color: &str) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO categories (name, color_hex) VALUES (?1, ?2)
             ON CONFLICT(name) DO UPDATE SET color_hex=excluded.color_hex",
            params![name, color],
        )?;
        Ok(())
    }

    pub fn ensure_category(&mut self, name: &str) -> SqlResult<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO categories (name) VALUES (?1)",
            params![name],
        )?;
        Ok(())
    }

    /// Categories that appear on at least one document, paired with their saved
    /// color if one has been assigned. `None` means the caller should pick a
    /// fallback (e.g. a palette color keyed off the category name), so that
    /// distinct uncolored categories don't all render identically.
    pub fn all_categories_with_colors(&self) -> SqlResult<Vec<(String, Option<String>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT dc.category, c.color_hex
             FROM doc_categories dc
             JOIN documents d ON d.id = dc.doc_id
             LEFT JOIN categories c ON c.name = dc.category
             WHERE d.deleted = 0
             ORDER BY dc.category",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
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

    pub fn all_tags(&self) -> SqlResult<Vec<Tag>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, color_hex FROM tags ORDER BY name")?;
        let rows = stmt.query_map([], |r| {
            Ok(Tag {
                id: r.get(0)?,
                name: r.get(1)?,
                color_hex: r.get(2)?,
            })
        })?;
        let mut tags = Vec::new();
        for r in rows {
            tags.push(r?);
        }
        Ok(tags)
    }

    pub fn all_tags_with_counts(&self) -> SqlResult<Vec<(Tag, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.name, t.color_hex, COUNT(dt.doc_id) AS cnt
             FROM tags t
             LEFT JOIN doc_tags dt ON dt.tag_id = t.id
             GROUP BY t.id
             ORDER BY cnt DESC, t.name",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                Tag {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    color_hex: r.get(2)?,
                },
                r.get::<_, i64>(3)?,
            ))
        })?;
        let mut result = Vec::new();
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }

    pub fn create_tag(&mut self, name: &str, color: &str) -> SqlResult<i64> {
        self.conn.execute(
            "INSERT OR IGNORE INTO tags (name, color_hex) VALUES (?1, ?2)",
            params![name, color],
        )?;
        self.conn
            .query_row("SELECT id FROM tags WHERE name = ?1", params![name], |r| {
                r.get(0)
            })
    }

    pub fn delete_tag(&mut self, tag_id: i64) -> SqlResult<()> {
        self.conn
            .execute("DELETE FROM tags WHERE id = ?1", params![tag_id])?;
        Ok(())
    }

    pub fn rename_tag(&mut self, tag_id: i64, name: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE tags SET name = ?1 WHERE id = ?2",
            params![name, tag_id],
        )?;
        Ok(())
    }

    pub fn set_doc_tags(&mut self, doc_id: i64, tag_ids: &[i64]) -> SqlResult<()> {
        self.conn
            .execute("DELETE FROM doc_tags WHERE doc_id = ?1", params![doc_id])?;
        for tid in tag_ids {
            self.conn.execute(
                "INSERT OR IGNORE INTO doc_tags (doc_id, tag_id) VALUES (?1, ?2)",
                params![doc_id, tid],
            )?;
        }
        Ok(())
    }

    pub fn add_doc_tags(&mut self, doc_id: i64, tag_ids: &[i64]) -> SqlResult<()> {
        for tid in tag_ids {
            self.conn.execute(
                "INSERT OR IGNORE INTO doc_tags (doc_id, tag_id) VALUES (?1, ?2)",
                params![doc_id, tid],
            )?;
        }
        Ok(())
    }

    #[allow(dead_code)] // rounds out the CRUD surface over the library DB; exercised by tests
    pub fn all_categories(&self) -> SqlResult<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT category FROM doc_categories ORDER BY category")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut cats = Vec::new();
        for r in rows {
            cats.push(r?);
        }
        Ok(cats)
    }

    pub fn create_category(&mut self, name: &str, parent: Option<&str>) -> SqlResult<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO categories (name, parent) VALUES (?1, ?2)",
            params![name, parent],
        )?;
        Ok(())
    }

    #[allow(dead_code)] // rounds out the CRUD surface over the library DB
    pub fn delete_category(&mut self, name: &str) -> SqlResult<()> {
        self.conn.execute(
            "DELETE FROM doc_categories WHERE category = ?1",
            params![name],
        )?;
        Ok(())
    }

    pub fn rename_category(&mut self, old_name: &str, new_name: &str) -> SqlResult<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO categories (name, color_hex, parent)
             SELECT ?1, color_hex, parent FROM categories WHERE name = ?2",
            params![new_name, old_name],
        )?;
        tx.execute(
            "UPDATE categories SET parent = ?1 WHERE parent = ?2",
            params![new_name, old_name],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO doc_categories (doc_id, category)
             SELECT doc_id, ?1 FROM doc_categories WHERE category = ?2",
            params![new_name, old_name],
        )?;
        tx.execute("DELETE FROM categories WHERE name = ?1", params![old_name])?;
        tx.commit()?;
        Ok(())
    }

    pub fn all_categories_structured(&self) -> SqlResult<Vec<Category>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, color_hex, parent FROM categories ORDER BY parent NULLS FIRST, name",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Category {
                name: r.get(0)?,
                color_hex: r.get(1)?,
                parent: r.get(2)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn category_has_children(&self, name: &str) -> SqlResult<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM categories WHERE parent = ?1",
            params![name],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn force_delete_category_if_no_children(&mut self, name: &str) -> SqlResult<bool> {
        if self.category_has_children(name)? {
            return Ok(false);
        }
        // Relies on `doc_categories.category`'s `ON DELETE CASCADE` to clear
        // the category from every document that had it, same as
        // `delete_tag` relies on `doc_tags`'s cascade.
        self.conn
            .execute("DELETE FROM categories WHERE name = ?1", params![name])?;
        Ok(true)
    }

    pub fn set_category_parent(&mut self, name: &str, parent: Option<&str>) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE categories SET parent = ?1 WHERE name = ?2",
            params![parent, name],
        )?;
        Ok(())
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

    pub fn import_directory(&mut self, dir: &Path) -> SqlResult<usize> {
        let mut count = 0;
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Ok(0);
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let hidden = path
                    .file_name()
                    .map(|n| n.to_string_lossy().starts_with('.'))
                    .unwrap_or(false);
                if !hidden {
                    count += self.import_directory(&path)?;
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

fn search_clause(prefix: &str, param: usize) -> String {
    format!(
        "({prefix}title LIKE ?{param} \
         OR {prefix}id IN (SELECT doc_id FROM doc_categories \
                           WHERE category LIKE ?{param}) \
         OR {prefix}id IN (SELECT doc_id FROM doc_tags _dt \
                           JOIN tags _t ON _t.id = _dt.tag_id \
                           WHERE _t.name LIKE ?{param}))"
    )
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
        let tag = lib.create_tag("draft", "#ff0000").expect("tag");
        lib.set_doc_tags(id, &[tag]).expect("set tags");
        lib.add_doc_categories(id, &["Essays".to_string()])
            .expect("category");
        lib.touch_opened(&work.path().join("essay.typ"))
            .expect("open");

        lib.move_to_trash(id).expect("trash");

        for filter in [
            LibraryFilter::All,
            LibraryFilter::Tag(tag),
            LibraryFilter::Category("Essays".into()),
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
    fn untagged_filter_shows_only_documents_with_no_tags() {
        let (mut lib, work) = fixture();
        let (tagged, _) = add_doc(&mut lib, &work, "tagged.typ");
        let (bare, _) = add_doc(&mut lib, &work, "bare.typ");
        let tag = lib.create_tag("draft", "#ff0000").expect("tag");
        lib.set_doc_tags(tagged, &[tag]).expect("set tags");

        let docs = lib
            .documents(LibraryFilter::Untagged, "", SortOrder::Modified)
            .expect("list");
        assert_eq!(ids(&docs), vec![bare]);
        assert_eq!(lib.doc_count(&LibraryFilter::Untagged).expect("count"), 1);
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
    fn category_group_filter_includes_the_parent_and_its_children() {
        let (mut lib, work) = fixture();
        let (parent_doc, _) = add_doc(&mut lib, &work, "parent.typ");
        let (child_doc, _) = add_doc(&mut lib, &work, "child.typ");
        let (other_doc, _) = add_doc(&mut lib, &work, "other.typ");
        lib.create_category("Academic", None).expect("parent cat");
        lib.create_category("Essays", Some("Academic"))
            .expect("child cat");
        lib.add_doc_categories(parent_doc, &["Academic".to_string()])
            .expect("set");
        lib.add_doc_categories(child_doc, &["Essays".to_string()])
            .expect("set");
        lib.add_doc_categories(other_doc, &["Sermons".to_string()])
            .expect("set");

        let docs = lib
            .documents(
                LibraryFilter::CategoryGroup("Academic".into()),
                "",
                SortOrder::Title,
            )
            .expect("list");
        let mut got = ids(&docs);
        got.sort();
        let mut want = vec![parent_doc, child_doc];
        want.sort();
        assert_eq!(got, want);
        assert_eq!(
            lib.doc_count(&LibraryFilter::CategoryGroup("Academic".into()))
                .expect("count"),
            2
        );

        let narrow = lib
            .documents(
                LibraryFilter::Category("Essays".into()),
                "",
                SortOrder::Title,
            )
            .expect("list");
        assert_eq!(ids(&narrow), vec![child_doc]);
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
    fn search_also_matches_category_and_tag_names() {
        let (mut lib, work) = fixture();
        let (by_category, _) = add_doc(&mut lib, &work, "one.typ");
        let (by_tag, _) = add_doc(&mut lib, &work, "two.typ");
        lib.add_doc_categories(by_category, &["Homiletics".to_string()])
            .expect("category");
        let tag = lib.create_tag("patristics", "#ff0000").expect("tag");
        lib.set_doc_tags(by_tag, &[tag]).expect("set tags");

        let by_cat = lib
            .documents(LibraryFilter::All, "Homile", SortOrder::Title)
            .expect("list");
        assert_eq!(ids(&by_cat), vec![by_category]);
        let by_tag_name = lib
            .documents(LibraryFilter::All, "patris", SortOrder::Title)
            .expect("list");
        assert_eq!(ids(&by_tag_name), vec![by_tag]);
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
            LibraryFilter::Untagged,
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
    fn set_doc_tags_replaces_the_existing_set_rather_than_appending() {
        let (mut lib, work) = fixture();
        let (id, _) = add_doc(&mut lib, &work, "essay.typ");
        let draft = lib.create_tag("draft", "#ff0000").expect("tag");
        let final_tag = lib.create_tag("final", "#00ff00").expect("tag");

        lib.set_doc_tags(id, &[draft]).expect("set");
        lib.set_doc_tags(id, &[final_tag]).expect("replace");

        let names: Vec<String> = lib
            .doc_tags(id)
            .expect("tags")
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(names, vec!["final"]);
    }

    #[test]
    fn add_doc_tags_appends_and_ignores_duplicates() {
        let (mut lib, work) = fixture();
        let (id, _) = add_doc(&mut lib, &work, "essay.typ");
        let draft = lib.create_tag("draft", "#ff0000").expect("tag");
        let review = lib.create_tag("review", "#00ff00").expect("tag");

        lib.set_doc_tags(id, &[draft]).expect("set");
        lib.add_doc_tags(id, &[review]).expect("add");
        lib.add_doc_tags(id, &[review]).expect("add again");

        let names: Vec<String> = lib
            .doc_tags(id)
            .expect("tags")
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(names, vec!["draft", "review"]);
    }

    #[test]
    fn creating_a_tag_that_already_exists_returns_the_original_id() {
        let (mut lib, _work) = fixture();
        let first = lib.create_tag("draft", "#ff0000").expect("tag");
        let second = lib.create_tag("draft", "#00ff00").expect("tag again");

        assert_eq!(first, second);
        assert_eq!(lib.all_tags().expect("tags").len(), 1);
    }

    #[test]
    fn deleting_a_tag_detaches_it_from_every_document() {
        let (mut lib, work) = fixture();
        let (a, _) = add_doc(&mut lib, &work, "a.typ");
        let (b, _) = add_doc(&mut lib, &work, "b.typ");
        let tag = lib.create_tag("draft", "#ff0000").expect("tag");
        lib.set_doc_tags(a, &[tag]).expect("set");
        lib.set_doc_tags(b, &[tag]).expect("set");

        lib.delete_tag(tag).expect("delete");

        assert!(lib.doc_tags(a).expect("tags").is_empty());
        assert!(lib.doc_tags(b).expect("tags").is_empty());
        let untagged = lib
            .documents(LibraryFilter::Untagged, "", SortOrder::Title)
            .expect("list");
        assert_eq!(untagged.len(), 2);
    }

    #[test]
    fn renaming_a_tag_keeps_its_document_associations() {
        let (mut lib, work) = fixture();
        let (id, _) = add_doc(&mut lib, &work, "essay.typ");
        let tag = lib.create_tag("draft", "#ff0000").expect("tag");
        lib.set_doc_tags(id, &[tag]).expect("set");

        lib.rename_tag(tag, "in-progress").expect("rename");

        let names: Vec<String> = lib
            .doc_tags(id)
            .expect("tags")
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(names, vec!["in-progress"]);
        let docs = lib
            .documents(LibraryFilter::Tag(tag), "", SortOrder::Title)
            .expect("list");
        assert_eq!(ids(&docs), vec![id]);
    }

    #[test]
    fn tag_counts_include_zero_for_tags_nobody_uses() {
        let (mut lib, work) = fixture();
        let (id, _) = add_doc(&mut lib, &work, "essay.typ");
        let used = lib.create_tag("used", "#ff0000").expect("tag");
        lib.create_tag("unused", "#00ff00").expect("tag");
        lib.set_doc_tags(id, &[used]).expect("set");

        let counts: Vec<(String, i64)> = lib
            .all_tags_with_counts()
            .expect("counts")
            .into_iter()
            .map(|(t, n)| (t.name, n))
            .collect();
        assert_eq!(
            counts,
            vec![("used".to_string(), 1), ("unused".to_string(), 0)]
        );
    }

    // ── Categories ───────────────────────────────────────────────────────────

    #[test]
    fn ensure_category_is_idempotent() {
        let (mut lib, _work) = fixture();
        lib.ensure_category("Essays").expect("first");
        lib.ensure_category("Essays").expect("second");

        let names: Vec<String> = lib
            .all_categories_structured()
            .expect("cats")
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(names, vec!["Essays"]);
    }

    #[test]
    fn setting_a_category_on_a_document_registers_the_category() {
        let (mut lib, work) = fixture();
        let (id, _) = add_doc(&mut lib, &work, "essay.typ");

        lib.add_doc_categories(id, &["Sermons".to_string()])
            .expect("set");

        assert_eq!(lib.all_categories().expect("cats"), vec!["Sermons"]);
        assert!(lib
            .all_categories_structured()
            .expect("structured")
            .iter()
            .any(|c| c.name == "Sermons"));
    }

    /// The literal reported bug: a document must be able to hold more than
    /// one category at once, including two siblings under the same parent —
    /// not just one category total, and not just one-per-parent.
    #[test]
    fn a_document_can_hold_two_categories_under_the_same_parent() {
        let (mut lib, work) = fixture();
        let (id, _) = add_doc(&mut lib, &work, "essay.typ");
        lib.create_category("Academic", None).expect("parent");
        lib.create_category("Essays", Some("Academic"))
            .expect("child 1");
        lib.create_category("Lectures", Some("Academic"))
            .expect("child 2");

        lib.add_doc_categories(id, &["Essays".to_string(), "Lectures".to_string()])
            .expect("set both");

        let mut names: Vec<String> = lib
            .doc_categories(id)
            .expect("doc cats")
            .into_iter()
            .map(|c| c.name)
            .collect();
        names.sort();
        assert_eq!(names, vec!["Essays".to_string(), "Lectures".to_string()]);

        for cat in ["Essays", "Lectures"] {
            let docs = lib
                .documents(LibraryFilter::Category(cat.into()), "", SortOrder::Title)
                .expect("list");
            assert_eq!(ids(&docs), vec![id], "filter on {cat}");
        }
        let group = lib
            .documents(
                LibraryFilter::CategoryGroup("Academic".into()),
                "",
                SortOrder::Title,
            )
            .expect("list");
        assert_eq!(
            ids(&group),
            vec![id],
            "doc matching two children of the group must not be duplicated"
        );
    }

    #[test]
    fn add_doc_categories_is_additive_set_doc_categories_replaces() {
        let (mut lib, work) = fixture();
        let (id, _) = add_doc(&mut lib, &work, "essay.typ");

        lib.add_doc_categories(id, &["Sermons".to_string()])
            .expect("add first");
        lib.add_doc_categories(id, &["Essays".to_string()])
            .expect("add second");
        let mut names: Vec<String> = lib
            .doc_categories(id)
            .expect("doc cats")
            .into_iter()
            .map(|c| c.name)
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec!["Essays".to_string(), "Sermons".to_string()],
            "add_doc_categories must not clobber an existing category"
        );

        lib.set_doc_categories(id, &["Lectures".to_string()])
            .expect("replace");
        let names: Vec<String> = lib
            .doc_categories(id)
            .expect("doc cats")
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(
            names,
            vec!["Lectures".to_string()],
            "set_doc_categories must replace the full set"
        );
    }

    #[test]
    fn category_colors_round_trip() {
        let (mut lib, _work) = fixture();
        lib.ensure_category("Essays").expect("ensure");

        lib.set_category_color("Essays", "#aabbcc")
            .expect("set color");

        assert_eq!(
            lib.get_category_color("Essays"),
            Some("#aabbcc".to_string())
        );
    }

    /// A category with no colour of its own must report `None`, so callers can
    /// substitute a per-name palette colour and distinct categories stay
    /// visually distinct. Guards the regression where the column's
    /// `NOT NULL DEFAULT '#3584e4'` made `None` unreachable.
    #[test]
    fn a_category_with_no_chosen_color_reports_none() {
        let (mut lib, work) = fixture();
        lib.ensure_category("Essays").expect("ensure");
        lib.create_category("Homilies", None).expect("create");
        let (id, _) = add_doc(&mut lib, &work, "essay.typ");
        lib.add_doc_categories(id, &["Sermons".to_string()])
            .expect("set");

        assert_eq!(lib.get_category_color("Essays"), None);
        assert_eq!(lib.get_category_color("Homilies"), None);
        assert_eq!(
            lib.all_categories_with_colors().expect("colors"),
            vec![("Sermons".to_string(), None)]
        );
    }

    #[test]
    fn an_explicitly_chosen_color_survives_alongside_uncolored_categories() {
        let (mut lib, _work) = fixture();
        lib.ensure_category("Essays").expect("ensure");
        lib.ensure_category("Sermons").expect("ensure");

        lib.set_category_color("Essays", "#e01b24")
            .expect("set color");

        assert_eq!(
            lib.get_category_color("Essays"),
            Some("#e01b24".to_string())
        );
        assert_eq!(lib.get_category_color("Sermons"), None);
    }

    /// Someone may have deliberately picked the blue that used to be the schema
    /// default; once set explicitly it must round-trip like any other choice.
    #[test]
    fn the_old_default_blue_can_still_be_chosen_deliberately() {
        let (mut lib, _work) = fixture();
        lib.ensure_category("Essays").expect("ensure");

        lib.set_category_color("Essays", "#3584e4")
            .expect("set color");

        assert_eq!(
            lib.get_category_color("Essays"),
            Some("#3584e4".to_string())
        );
    }

    // ── Schema migration ─────────────────────────────────────────────────────

    /// Builds the pre-fix `categories` shape, then runs the migration over it.
    fn library_with_legacy_categories(rows: &[(&str, &str, Option<&str>)]) -> Library {
        let conn = Connection::open_in_memory().expect("in-memory DB");
        conn.execute_batch(
            "CREATE TABLE categories (
                 name TEXT NOT NULL PRIMARY KEY,
                 color_hex TEXT NOT NULL DEFAULT '#3584e4'
             );
             ALTER TABLE categories ADD COLUMN parent TEXT REFERENCES categories(name);",
        )
        .expect("legacy schema");
        for (name, color, parent) in rows {
            conn.execute(
                "INSERT INTO categories (name, color_hex, parent) VALUES (?1, ?2, ?3)",
                params![name, color, parent],
            )
            .expect("seed");
        }
        let lib = Library {
            conn,
            trash_dir: PathBuf::from("/nonexistent"),
        };
        lib.migrate().expect("migrate");
        lib
    }

    #[test]
    fn migration_treats_the_old_auto_applied_default_as_no_color_chosen() {
        let lib = library_with_legacy_categories(&[
            ("Essays", "#3584e4", None),
            ("Sermons", "#e01b24", None),
        ]);

        assert_eq!(
            lib.get_category_color("Essays"),
            None,
            "auto default becomes unset"
        );
        assert_eq!(
            lib.get_category_color("Sermons"),
            Some("#e01b24".to_string()),
            "a real choice is preserved"
        );
    }

    #[test]
    fn migration_preserves_category_parents() {
        let lib = library_with_legacy_categories(&[
            ("Academic", "#3584e4", None),
            ("Essays", "#33d17a", Some("Academic")),
        ]);

        let cats = lib.all_categories_structured().expect("cats");
        let essays = cats
            .iter()
            .find(|c| c.name == "Essays")
            .expect("child present");
        assert_eq!(essays.parent, Some("Academic".to_string()));
        assert!(lib.category_has_children("Academic").expect("children"));
    }

    #[test]
    fn migration_is_idempotent_and_leaves_the_column_nullable() {
        let lib = library_with_legacy_categories(&[("Essays", "#3584e4", None)]);
        lib.migrate().expect("second migrate");
        lib.migrate().expect("third migrate");

        assert_eq!(lib.get_category_color("Essays"), None);
        let not_null: i64 = lib
            .conn
            .query_row(
                "SELECT \"notnull\" FROM pragma_table_info('categories') WHERE name = 'color_hex'",
                [],
                |r| r.get(0),
            )
            .expect("pragma");
        assert_eq!(not_null, 0, "color_hex should be nullable after migrating");
    }

    /// The in-memory tests above run without WAL or foreign-key enforcement.
    /// A real library is a file with both switched on, and the migration drops
    /// and recreates a table that other rows reference — so exercise it there.
    #[test]
    fn migration_survives_a_real_file_database_with_wal_and_foreign_keys() {
        let work = TempDir::new().expect("temp dir");
        let db_path = work.path().join("library.sqlite");
        {
            let conn = Connection::open(&db_path).expect("create");
            conn.execute_batch(
                "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;
                 CREATE TABLE categories (
                     name TEXT NOT NULL PRIMARY KEY,
                     color_hex TEXT NOT NULL DEFAULT '#3584e4'
                 );
                 ALTER TABLE categories ADD COLUMN parent TEXT REFERENCES categories(name);
                 INSERT INTO categories (name, color_hex, parent) VALUES ('Academic', '#3584e4', NULL);
                 INSERT INTO categories (name, color_hex, parent) VALUES ('Essays', '#e01b24', 'Academic');",
            )
            .expect("legacy file schema");
        }

        let conn = Connection::open(&db_path).expect("reopen");
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")
            .expect("pragmas");
        let lib = Library {
            conn,
            trash_dir: work.path().join("trash"),
        };
        lib.migrate().expect("migrate");

        assert_eq!(lib.get_category_color("Academic"), None);
        assert_eq!(
            lib.get_category_color("Essays"),
            Some("#e01b24".to_string())
        );
        let essays = lib
            .all_categories_structured()
            .expect("cats")
            .into_iter()
            .find(|c| c.name == "Essays")
            .expect("child survived");
        assert_eq!(essays.parent, Some("Academic".to_string()));

        let fk_violations: i64 = lib
            .conn
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })
            .expect("fk check");
        assert_eq!(
            fk_violations, 0,
            "migration must not leave dangling references"
        );
        let fk_on: i64 = lib
            .conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .expect("pragma");
        assert_eq!(fk_on, 1, "foreign keys must be switched back on afterwards");
    }

    #[test]
    fn a_freshly_created_database_already_has_a_nullable_color_column() {
        let (lib, _work) = fixture();
        let not_null: i64 = lib
            .conn
            .query_row(
                "SELECT \"notnull\" FROM pragma_table_info('categories') WHERE name = 'color_hex'",
                [],
                |r| r.get(0),
            )
            .expect("pragma");
        assert_eq!(not_null, 0);
    }

    /// Pre-multi-category databases stored a document's one category in
    /// `documents.category` directly; `migrate()` must backfill that value
    /// into `doc_categories` so existing libraries don't lose their
    /// categorization the first time they're opened after the upgrade.
    #[test]
    fn migration_backfills_existing_document_categories_into_the_join_table() {
        let conn = Connection::open_in_memory().expect("in-memory DB");
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE documents (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 path TEXT NOT NULL UNIQUE,
                 title TEXT NOT NULL,
                 category TEXT,
                 archived INTEGER NOT NULL DEFAULT 0,
                 notes TEXT,
                 created_at TEXT NOT NULL,
                 modified_at TEXT NOT NULL,
                 last_opened_at TEXT
             );
             INSERT INTO documents (path, title, category, created_at, modified_at)
                 VALUES ('/tmp/essay.typ', 'Essay', 'Sermons', '2020-01-01', '2020-01-01');",
        )
        .expect("legacy documents schema");
        let lib = Library {
            conn,
            trash_dir: PathBuf::from("/nonexistent"),
        };
        lib.migrate().expect("migrate");

        let id: i64 = lib
            .conn
            .query_row(
                "SELECT id FROM documents WHERE path = '/tmp/essay.typ'",
                [],
                |r| r.get(0),
            )
            .expect("doc id");
        let names: Vec<String> = lib
            .doc_categories(id)
            .expect("doc cats")
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(names, vec!["Sermons".to_string()]);
    }

    #[test]
    fn renaming_a_category_moves_its_documents_and_reparents_its_children() {
        let (mut lib, work) = fixture();
        let (id, _) = add_doc(&mut lib, &work, "essay.typ");
        lib.create_category("Academic", None).expect("parent");
        lib.create_category("Essays", Some("Academic"))
            .expect("child");
        lib.add_doc_categories(id, &["Academic".to_string()])
            .expect("set");

        lib.rename_category("Academic", "Scholarly")
            .expect("rename");

        let doc_cat_names: Vec<String> = lib
            .doc_categories(id)
            .expect("doc cats")
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(doc_cat_names, vec!["Scholarly".to_string()]);
        let child = lib
            .all_categories_structured()
            .expect("cats")
            .into_iter()
            .find(|c| c.name == "Essays")
            .expect("child still present");
        assert_eq!(child.parent, Some("Scholarly".to_string()));
        assert!(!lib
            .all_categories_structured()
            .expect("cats")
            .iter()
            .any(|c| c.name == "Academic"));
    }

    #[test]
    fn a_category_with_children_refuses_to_be_force_deleted() {
        let (mut lib, _work) = fixture();
        lib.create_category("Academic", None).expect("parent");
        lib.create_category("Essays", Some("Academic"))
            .expect("child");

        assert!(lib.category_has_children("Academic").expect("check"));
        assert!(!lib
            .force_delete_category_if_no_children("Academic")
            .expect("delete"));
        assert!(lib
            .all_categories_structured()
            .expect("cats")
            .iter()
            .any(|c| c.name == "Academic"));
    }

    #[test]
    fn force_deleting_a_childless_category_clears_it_from_its_documents() {
        let (mut lib, work) = fixture();
        let (id, _) = add_doc(&mut lib, &work, "essay.typ");
        lib.add_doc_categories(id, &["Essays".to_string()])
            .expect("set");

        assert!(lib
            .force_delete_category_if_no_children("Essays")
            .expect("delete"));

        assert!(lib.doc_categories(id).expect("doc cats").is_empty());
        assert!(lib.all_categories().expect("cats").is_empty());
    }

    // ── Projects ─────────────────────────────────────────────────────────────

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
            search_clause(q.prefix, idx),
            q.order(&sort)
        )
    }

    /// Filters that take a leading parameter must bind the search pattern to
    /// `?2`; the rest to `?1`. Getting this wrong silently searches for the
    /// category/tag id instead of the user's text.
    #[test]
    fn search_pattern_takes_the_slot_after_any_leading_parameter() {
        for f in [
            LibraryFilter::All,
            LibraryFilter::Archive,
            LibraryFilter::Untagged,
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
            LibraryFilter::Category("C".into()),
            LibraryFilter::CategoryGroup("G".into()),
            LibraryFilter::Project(7),
            LibraryFilter::Tag(9),
        ] {
            let sql = sql_for(f.clone(), SortOrder::Modified);
            assert!(sql.contains("LIKE ?2"), "{f:?} should bind search to ?2");
        }
    }

    #[test]
    fn joined_filters_prefix_every_column_with_the_table_alias() {
        for f in [LibraryFilter::Project(7), LibraryFilter::Tag(9)] {
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
            LibraryFilter::Untagged,
            LibraryFilter::Category("C".into()),
            LibraryFilter::Tag(9),
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
                LibraryFilter::Untagged,
                "id NOT IN (SELECT DISTINCT doc_id FROM doc_tags)",
            ),
            (LibraryFilter::Recent, "last_opened_at IS NOT NULL"),
            (
                LibraryFilter::Category("C".into()),
                "EXISTS (SELECT 1 FROM doc_categories dc WHERE dc.doc_id = id AND dc.category = ?1)",
            ),
            (
                LibraryFilter::CategoryGroup("G".into()),
                "name = ?1 OR parent = ?1",
            ),
            (
                LibraryFilter::Project(7),
                "pd.project_id = ?1 AND d.deleted = 0",
            ),
            (LibraryFilter::Tag(9), "dt.tag_id = ?1"),
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
