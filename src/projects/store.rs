use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Row};
use serde_json::Value;

use super::{Document, DocumentId, DocumentKind, Project, ProjectId};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Project document not found")]
    NotFound,
    #[error("Document changed since it was opened; reload before saving")]
    Conflict,
    #[error("Unsupported project database version: {0}")]
    UnsupportedVersion(u32),
    #[error("Name must not be empty")]
    EmptyName,
    #[error("Project database must be a regular file owned by the current user")]
    InvalidPath,
    #[error("Operating system random source unavailable")]
    Random,
    #[error("System clock unavailable")]
    Clock,
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

type Result<T> = std::result::Result<T, StoreError>;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS projects (
 id TEXT PRIMARY KEY, name TEXT NOT NULL,
 created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS documents (
 id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
 kind TEXT NOT NULL CHECK(kind IN ('collection','request','environment','scenario','finding','run','evidence')),
 title TEXT NOT NULL, payload TEXT NOT NULL, revision INTEGER NOT NULL CHECK(revision >= 1),
 created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS documents_by_project ON documents(project_id, kind, title);
PRAGMA user_version = 1;
";

const DOCUMENT_COLUMNS: &str =
    "id, project_id, kind, title, payload, revision, created_at_ms, updated_at_ms";

pub struct ProjectStore {
    connection: Mutex<Connection>,
}

impl ProjectStore {
    pub fn open(path: &Path) -> Result<Self> {
        prepare_directory(path)?;
        // macOS's /var and /tmp are system symlinks. Resolve the parent while
        // preserving SQLite's no-follow protection on the database itself.
        let filename = path.file_name().ok_or(StoreError::InvalidPath)?;
        let parent = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let path = parent.canonicalize()?.join(filename);
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_FULL_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW;
        let mut connection = Connection::open_with_flags(&path, flags)?;
        let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > 1 {
            return Err(StoreError::UnsupportedVersion(version));
        }
        make_private(&path)?;
        connection.busy_timeout(Duration::from_secs(3))?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        if version == 0 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(SCHEMA)?;
            transaction.commit()?;
        }
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn create_project(&self, name: &str) -> Result<Project> {
        let name = name.trim();
        validate_name(name)?;
        let timestamp = now()?;
        let project = Project {
            id: ProjectId(new_id()?),
            name: name.into(),
            created_at_ms: timestamp,
            updated_at_ms: timestamp,
        };
        self.connection.lock().execute(
            "INSERT INTO projects VALUES (?1,?2,?3,?4)",
            params![project.id.as_str(), project.name, timestamp, timestamp],
        )?;
        Ok(project)
    }

    pub fn projects(&self) -> Result<Vec<Project>> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "SELECT id, name, created_at_ms, updated_at_ms FROM projects ORDER BY name, id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(Project {
                id: ProjectId(row.get(0)?),
                name: row.get(1)?,
                created_at_ms: row.get(2)?,
                updated_at_ms: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn create_document(
        &self,
        project: &ProjectId,
        kind: DocumentKind,
        title: &str,
        payload: Value,
    ) -> Result<Document> {
        let title = title.trim();
        validate_name(title)?;
        let connection = self.connection.lock();
        if !project_exists(&connection, project)? {
            return Err(StoreError::NotFound);
        }
        let timestamp = now()?;
        let document = Document {
            id: DocumentId(new_id()?),
            project_id: project.clone(),
            kind,
            title: title.into(),
            payload,
            revision: 1,
            created_at_ms: timestamp,
            updated_at_ms: timestamp,
        };
        connection.execute(
            "INSERT INTO documents VALUES (?1,?2,?3,?4,?5,1,?6,?6)",
            params![
                document.id.as_str(),
                project.as_str(),
                kind.as_str(),
                title,
                document.payload.to_string(),
                timestamp
            ],
        )?;
        Ok(document)
    }

    pub fn document(&self, id: &DocumentId) -> Result<Document> {
        read_document(&self.connection.lock(), id)
    }

    pub fn documents(
        &self,
        project: &ProjectId,
        kind: Option<DocumentKind>,
    ) -> Result<Vec<Document>> {
        let connection = self.connection.lock();
        if !project_exists(&connection, project)? {
            return Err(StoreError::NotFound);
        }
        let sql = format!("SELECT {DOCUMENT_COLUMNS} FROM documents WHERE project_id=?1 AND (?2 IS NULL OR kind=?2) ORDER BY title, id");
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(
            params![project.as_str(), kind.map(DocumentKind::as_str)],
            RawDocument::read,
        )?;
        rows.map(|row| row.map_err(StoreError::from).and_then(RawDocument::decode))
            .collect()
    }

    pub fn update_document(
        &self,
        id: &DocumentId,
        revision: u64,
        title: &str,
        payload: Value,
    ) -> Result<Document> {
        let title = title.trim();
        validate_name(title)?;
        let connection = self.connection.lock();
        let changed = connection.execute("UPDATE documents SET title=?1,payload=?2,revision=revision+1,updated_at_ms=?3 WHERE id=?4 AND revision=?5", params![title, payload.to_string(), now()?, id.as_str(), revision])?;
        if changed == 0 {
            return match read_document(&connection, id) {
                Ok(_) => Err(StoreError::Conflict),
                Err(error) => Err(error),
            };
        }
        read_document(&connection, id)
    }

    pub fn delete_project(&self, id: &ProjectId) -> Result<()> {
        if self
            .connection
            .lock()
            .execute("DELETE FROM projects WHERE id=?1", [id.as_str()])?
            == 0
        {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }
}

fn project_exists(connection: &Connection, id: &ProjectId) -> rusqlite::Result<bool> {
    connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
        [id.as_str()],
        |row| row.get(0),
    )
}

fn read_document(connection: &Connection, id: &DocumentId) -> Result<Document> {
    let sql = format!("SELECT {DOCUMENT_COLUMNS} FROM documents WHERE id=?1");
    let row = connection
        .query_row(&sql, [id.as_str()], RawDocument::read)
        .optional()?
        .ok_or(StoreError::NotFound)?;
    row.decode()
}

struct RawDocument {
    id: String,
    project: String,
    kind: String,
    title: String,
    payload: String,
    revision: u64,
    created: u64,
    updated: u64,
}

impl RawDocument {
    fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            project: row.get(1)?,
            kind: row.get(2)?,
            title: row.get(3)?,
            payload: row.get(4)?,
            revision: row.get(5)?,
            created: row.get(6)?,
            updated: row.get(7)?,
        })
    }
    fn decode(self) -> Result<Document> {
        Ok(Document {
            id: DocumentId(self.id),
            project_id: ProjectId(self.project),
            kind: DocumentKind::parse(&self.kind)?,
            title: self.title,
            payload: serde_json::from_str(&self.payload)?,
            revision: self.revision,
            created_at_ms: self.created,
            updated_at_ms: self.updated,
        })
    }
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(StoreError::EmptyName);
    }
    Ok(())
}

fn new_id() -> Result<String> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|_| StoreError::Random)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn now() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StoreError::Clock)?
        .as_millis()
        .try_into()
        .map_err(|_| StoreError::Clock)
}

fn prepare_directory(path: &Path) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(parent)?;
    }
    Ok(())
}

fn make_private(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(StoreError::InvalidPath);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if metadata.uid() != unsafe { libc::geteuid() } {
            return Err(StoreError::InvalidPath);
        }
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}
