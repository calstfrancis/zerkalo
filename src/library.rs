use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};

pub struct Library {
    conn: Connection,
}

#[derive(Clone, Debug)]
pub struct Document {
    pub id: i64,
    pub path: PathBuf,
    pub title: String,
    pub category: Option<String>,
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
pub struct Project {
    pub id: i64,
    pub name: String,
    pub root_doc_id: Option<i64>,
    pub created_at: String,
}
#[derive(Clone, Debug)]
pub struct Category {
    pub name: String,
    pub color_hex: String,
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
    "id, path, title, category, archived, pinned, notes, created_at, modified_at, last_opened_at";

fn doc_cols_prefixed(prefix: &str) -> String {
    DOC_COLS
        .split(", ")
        .map(|c| format!("{prefix}.{c}"))
        .collect::<Vec<_>>()
        .join(", ")
}

impl Library {
    pub fn open() -> SqlResult<Self> {
        let dir = glib::user_data_dir().join("zerkalo");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("library.sqlite");
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        let lib = Self { conn };
        lib.migrate()?;
        Ok(lib)
    }

    pub fn open_in_memory() -> Self {
        let conn = Connection::open_in_memory().expect("in-memory DB");
        conn.execute_batch("PRAGMA foreign_keys = ON;").ok();
        let lib = Self { conn };
        lib.migrate().ok();
        lib
    }

    fn migrate(&self) -> SqlResult<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS documents (
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
                    color_hex TEXT NOT NULL DEFAULT '#3584e4'
                );",
            )
            .ok();
        self.conn.execute_batch(
            "ALTER TABLE categories ADD COLUMN parent TEXT REFERENCES categories(name);"
        ).ok();
        self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_doc_category ON documents(category);
             CREATE INDEX IF NOT EXISTS idx_doc_archived ON documents(archived, deleted);
             CREATE INDEX IF NOT EXISTS idx_doc_last_opened ON documents(last_opened_at);
             CREATE INDEX IF NOT EXISTS idx_doc_modified ON documents(modified_at);
             CREATE INDEX IF NOT EXISTS idx_doc_tags_tag ON doc_tags(tag_id);
             CREATE INDEX IF NOT EXISTS idx_doc_tags_doc ON doc_tags(doc_id);"
        ).ok();
        self.conn.execute_batch(
            "INSERT OR IGNORE INTO categories (name)
             SELECT DISTINCT category FROM documents WHERE category IS NOT NULL;"
        ).ok();
        Ok(())
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
        let fs_modified = meta.as_ref()
            .and_then(|m| m.modified().ok())
            .map(|t| { let dt: chrono::DateTime<Utc> = t.into(); dt.to_rfc3339() })
            .unwrap_or_else(|| now.clone());
        let fs_created = meta.as_ref()
            .and_then(|m| m.created().ok())
            .map(|t| { let dt: chrono::DateTime<Utc> = t.into(); dt.to_rfc3339() })
            .unwrap_or_else(|| fs_modified.clone());

        let existing: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM documents WHERE path = ?1",
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
            "UPDATE documents SET last_opened_at = ?1 WHERE path = ?2",
            params![now, path_str],
        )?;
        Ok(())
    }

    pub fn touch_saved(&mut self, path: &Path) -> SqlResult<()> {
        self.upsert_document(path)?;
        let path_str = path.to_string_lossy().to_string();
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE documents SET modified_at = ?1 WHERE path = ?2",
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
                let fs_created = meta.created()
                    .or_else(|_| meta.modified())
                    .map(|t| { let dt: chrono::DateTime<Utc> = t.into(); dt.to_rfc3339() })
                    .ok();
                if let Some(created) = fs_created {
                    self.conn.execute(
                        "UPDATE documents SET created_at = ?1 WHERE id = ?2",
                        rusqlite::params![created, id],
                    ).ok();
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
        let mut docs = Vec::new();
        match filter {
            LibraryFilter::All => {
                let sql = format!(
                    "SELECT {DOC_COLS} FROM documents
                     WHERE archived = 0 AND deleted = 0 AND {}
                     ORDER BY pinned DESC, {}",
                    search_clause("", 1),
                    sort.clause("")
                );
                let mut stmt = self.conn.prepare(&sql)?;
                let rows = stmt.query_map(params![search_pat], row_to_doc)?;
                for r in rows { docs.push(r?); }
            }
            LibraryFilter::Project(pid) => {
                let sql = format!(
                    "SELECT {} FROM documents d
                     JOIN project_docs pd ON pd.doc_id = d.id
                     WHERE pd.project_id = ?1 AND d.deleted = 0 AND {}
                     ORDER BY d.pinned DESC, pd.position, d.title",
                    doc_cols_prefixed("d"),
                    search_clause("d.", 2)
                );
                let mut stmt = self.conn.prepare(&sql)?;
                let rows = stmt.query_map(params![pid, search_pat], row_to_doc)?;
                for r in rows { docs.push(r?); }
            }
            LibraryFilter::Tag(tid) => {
                let sql = format!(
                    "SELECT {} FROM documents d
                     JOIN doc_tags dt ON dt.doc_id = d.id
                     WHERE dt.tag_id = ?1 AND d.archived = 0 AND d.deleted = 0 AND {}
                     ORDER BY d.pinned DESC, {}",
                    doc_cols_prefixed("d"),
                    search_clause("d.", 2),
                    sort.clause("d.")
                );
                let mut stmt = self.conn.prepare(&sql)?;
                let rows = stmt.query_map(params![tid, search_pat], row_to_doc)?;
                for r in rows { docs.push(r?); }
            }
            LibraryFilter::Category(cat) => {
                let sql = format!(
                    "SELECT {DOC_COLS} FROM documents
                     WHERE category = ?1 AND archived = 0 AND deleted = 0 AND {}
                     ORDER BY pinned DESC, {}",
                    search_clause("", 2),
                    sort.clause("")
                );
                let mut stmt = self.conn.prepare(&sql)?;
                let rows = stmt.query_map(params![cat, search_pat], row_to_doc)?;
                for r in rows { docs.push(r?); }
            }
            LibraryFilter::CategoryGroup(ref parent) => {
                let sql = format!(
                    "SELECT {DOC_COLS} FROM documents
                     WHERE category IN (
                         SELECT name FROM categories WHERE name = ?1 OR parent = ?1
                     ) AND archived = 0 AND deleted = 0 AND {}
                     ORDER BY pinned DESC, {}",
                    search_clause("", 2),
                    sort.clause("")
                );
                let mut stmt = self.conn.prepare(&sql)?;
                let rows = stmt.query_map(params![parent, search_pat], row_to_doc)?;
                for r in rows { docs.push(r?); }
            }
            LibraryFilter::Archive => {
                let sql = format!(
                    "SELECT {DOC_COLS} FROM documents
                     WHERE archived = 1 AND deleted = 0 AND {}
                     ORDER BY pinned DESC, {}",
                    search_clause("", 1),
                    sort.clause("")
                );
                let mut stmt = self.conn.prepare(&sql)?;
                let rows = stmt.query_map(params![search_pat], row_to_doc)?;
                for r in rows { docs.push(r?); }
            }
            LibraryFilter::Recent => {
                let sql = format!(
                    "SELECT {DOC_COLS} FROM documents
                     WHERE last_opened_at IS NOT NULL AND archived = 0 AND deleted = 0 AND {}
                     ORDER BY pinned DESC, last_opened_at DESC LIMIT 30",
                    search_clause("", 1)
                );
                let mut stmt = self.conn.prepare(&sql)?;
                let rows = stmt.query_map(params![search_pat], row_to_doc)?;
                for r in rows { docs.push(r?); }
            }
            LibraryFilter::Untagged => {
                let sql = format!(
                    "SELECT {DOC_COLS} FROM documents
                     WHERE archived = 0 AND deleted = 0
                     AND id NOT IN (SELECT DISTINCT doc_id FROM doc_tags)
                     AND {}
                     ORDER BY pinned DESC, {}",
                    search_clause("", 1),
                    sort.clause("")
                );
                let mut stmt = self.conn.prepare(&sql)?;
                let rows = stmt.query_map(params![search_pat], row_to_doc)?;
                for r in rows { docs.push(r?); }
            }
            LibraryFilter::Trash => {
                let sql = format!(
                    "SELECT {DOC_COLS} FROM documents
                     WHERE deleted = 1 AND {}
                     ORDER BY modified_at DESC",
                    search_clause("", 1)
                );
                let mut stmt = self.conn.prepare(&sql)?;
                let rows = stmt.query_map(params![search_pat], row_to_doc)?;
                for r in rows { docs.push(r?); }
            }
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
                "SELECT COUNT(*) FROM documents WHERE category=?1 AND archived=0 AND deleted=0",
                params![cat],
                |r| r.get(0),
            ),
            LibraryFilter::CategoryGroup(parent) => self.conn.query_row(
                "SELECT COUNT(*) FROM documents
                 WHERE category IN (SELECT name FROM categories WHERE name=?1 OR parent=?1)
                 AND archived=0 AND deleted=0",
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

    pub fn set_category(&mut self, doc_id: i64, category: Option<&str>) -> SqlResult<()> {
        if let Some(name) = category {
            self.ensure_category(name)?;
        }
        self.conn.execute(
            "UPDATE documents SET category = ?1 WHERE id = ?2",
            params![category, doc_id],
        )?;
        Ok(())
    }

    pub fn set_pinned(&mut self, doc_id: i64, pinned: bool) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE documents SET pinned=?1 WHERE id=?2",
            params![pinned as i64, doc_id],
        )?;
        Ok(())
    }

    pub fn get_category_color(&self, name: &str) -> String {
        self.conn
            .query_row(
                "SELECT color_hex FROM categories WHERE name=?1",
                params![name],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| "#3584e4".to_string())
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

    pub fn all_categories_with_colors(&self) -> SqlResult<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT d.category, COALESCE(c.color_hex, '#3584e4')
             FROM documents d
             LEFT JOIN categories c ON c.name = d.category
             WHERE d.category IS NOT NULL AND d.deleted = 0
             ORDER BY d.category",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn move_to_trash(&mut self, doc_id: i64) -> SqlResult<()> {
        let doc = match self.doc_by_id(doc_id)? {
            Some(d) => d,
            None => return Ok(()),
        };
        let trash_dir = glib::user_data_dir().join("zerkalo").join("trash");
        std::fs::create_dir_all(&trash_dir).ok();
        let ts = Utc::now().timestamp();
        let filename = doc
            .path
            .file_name()
            .map(|n| format!("{}-{}", ts, n.to_string_lossy()))
            .unwrap_or_else(|| format!("{}.typ", ts));
        let trash_path = trash_dir.join(&filename);
        if std::fs::rename(&doc.path, &trash_path).is_err() {
            if std::fs::copy(&doc.path, &trash_path).is_ok() {
                std::fs::remove_file(&doc.path).ok();
            }
        }
        let trash_str = trash_path.to_string_lossy().to_string();
        self.conn.execute(
            "UPDATE documents SET deleted=1, trash_path=?1 WHERE id=?2",
            params![trash_str, doc_id],
        )?;
        Ok(())
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
            let _ = std::fs::rename(&tpath, &doc.path);
            self.conn.execute(
                "UPDATE documents SET deleted=0, trash_path=NULL WHERE id=?1",
                params![doc_id],
            )?;
        }
        Ok(())
    }

    pub fn permanently_delete(&mut self, doc_id: i64) -> SqlResult<()> {
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
            std::fs::remove_file(&tpath).ok();
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

    pub fn position_in_project(
        &self,
        project_id: i64,
        doc_id: i64,
    ) -> SqlResult<Option<i64>> {
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
                Tag { id: r.get(0)?, name: r.get(1)?, color_hex: r.get(2)? },
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
        self.conn.query_row(
            "SELECT id FROM tags WHERE name = ?1",
            params![name],
            |r| r.get(0),
        )
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

    pub fn all_categories(&self) -> SqlResult<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT category FROM documents
             WHERE category IS NOT NULL ORDER BY category",
        )?;
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

    pub fn delete_category(&mut self, name: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE documents SET category = NULL WHERE category = ?1",
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
        tx.execute("UPDATE categories SET parent = ?1 WHERE parent = ?2", params![new_name, old_name])?;
        tx.execute("UPDATE documents SET category = ?1 WHERE category = ?2", params![new_name, old_name])?;
        tx.execute("DELETE FROM categories WHERE name = ?1", params![old_name])?;
        tx.commit()?;
        Ok(())
    }


    pub fn all_categories_structured(&self) -> SqlResult<Vec<Category>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, color_hex, parent FROM categories ORDER BY parent NULLS FIRST, name"
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Category { name: r.get(0)?, color_hex: r.get(1)?, parent: r.get(2)? })
        })?;
        let mut out = Vec::new();
        for r in rows { out.push(r?); }
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
        self.conn.execute("UPDATE documents SET category = NULL WHERE category = ?1", params![name])?;
        self.conn.execute("DELETE FROM categories WHERE name = ?1", params![name])?;
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

    pub fn doc_by_path(&self, path: &Path) -> SqlResult<Option<Document>> {
        let path_str = path.to_string_lossy().to_string();
        let sql = format!("SELECT {DOC_COLS} FROM documents WHERE path = ?1");
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
            } else if path.extension().map(|e| e == "typ").unwrap_or(false) {
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
         OR {prefix}category LIKE ?{param} \
         OR {prefix}id IN (SELECT doc_id FROM doc_tags _dt \
                           JOIN tags _t ON _t.id = _dt.tag_id \
                           WHERE _t.name LIKE ?{param}))"
    )
}

/// Reads the first `#let doc-title = "..."` line from a Typst file.
/// Falls back to `#let title = "..."` if doc-title isn't found.
fn extract_typst_title(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut fallback: Option<String> = None;
    for line in content.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("#let doc-title") {
            let after = rest.trim();
            if after.starts_with('=') {
                if let Some(val) = parse_typst_string_value(after[1..].trim()) {
                    return Some(val);
                }
            }
        }
        if fallback.is_none() {
            if let Some(rest) = t.strip_prefix("#let title") {
                let after = rest.trim();
                if after.starts_with('=') {
                    if let Some(val) = parse_typst_string_value(after[1..].trim()) {
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
        if !val.is_empty() { return Some(val); }
    } else if let Some(inner) = s.strip_prefix('[') {
        let end = inner.find(']')?;
        let val = inner[..end].trim().to_string();
        if !val.is_empty() { return Some(val); }
    }
    None
}

fn row_to_doc(row: &rusqlite::Row<'_>) -> rusqlite::Result<Document> {
    Ok(Document {
        id: row.get(0)?,
        path: PathBuf::from(row.get::<_, String>(1)?),
        title: row.get(2)?,
        category: row.get(3)?,
        archived: row.get::<_, i64>(4)? != 0,
        pinned: row.get::<_, i64>(5)? != 0,
        notes: row.get(6)?,
        created_at: row.get(7)?,
        modified_at: row.get(8)?,
        last_opened_at: row.get(9)?,
    })
}
