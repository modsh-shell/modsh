//! Context graph — SQLite-based graph storage
//!
//! The context graph stores:
//! - `ProjectNode`: Detected project type, stack, git remote
//! - `CommandNode`: Command executions with metadata
//! - `PatternNode`: Recurring command sequences
//! - `ServerNode`: SSH hosts and their typical commands
//! - `ErrorNode`: Failed commands and recovery actions

use rusqlite::{Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use thiserror::Error;

/// Context graph errors
#[derive(Error, Debug)]
pub enum ContextError {
    /// Database error
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    /// Serialization error
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    /// Node not found
    #[error("node not found: {0}")]
    NotFound(String),
    /// Time error
    #[error("time error: {0}")]
    Time(String),
}

impl From<std::time::SystemTimeError> for ContextError {
    fn from(e: std::time::SystemTimeError) -> Self {
        ContextError::Time(e.to_string())
    }
}

/// Node types in the context graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Node {
    /// A project with detected type
    Project(ProjectNode),
    /// A command execution
    Command(CommandNode),
    /// A detected pattern
    Pattern(PatternNode),
    /// A remote server
    Server(ServerNode),
    /// An error with recovery
    Error(ErrorNode),
}

/// Project node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectNode {
    /// Node ID
    pub id: String,
    /// Project path
    pub path: PathBuf,
    /// Detected project type (rust, python, node, etc.)
    pub project_type: String,
    /// Stack/framework
    pub stack: Vec<String>,
    /// Git remote URL
    pub git_remote: Option<String>,
    /// Created timestamp
    pub created: SystemTime,
    /// Last accessed
    pub last_accessed: SystemTime,
    /// Access count
    pub access_count: u32,
}

/// Command node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandNode {
    /// Node ID
    pub id: String,
    /// Command string
    pub command: String,
    /// Arguments (serialized)
    pub args: Vec<String>,
    /// Working directory
    pub directory: PathBuf,
    /// Project ID if in a known project
    pub project_id: Option<String>,
    /// Exit code
    pub exit_code: u8,
    /// Duration
    pub duration: Duration,
    /// Timestamp
    pub timestamp: SystemTime,
    /// Weight (learned importance)
    pub weight: f32,
}

/// Pattern node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternNode {
    /// Node ID
    pub id: String,
    /// Sequence of command IDs
    pub sequence: Vec<String>,
    /// Pattern frequency
    pub frequency: u32,
    /// Last seen
    pub last_seen: SystemTime,
}

/// Server node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerNode {
    /// Node ID
    pub id: String,
    /// Hostname or IP
    pub host: String,
    /// User
    pub user: Option<String>,
    /// Port
    pub port: u16,
    /// Common commands on this server
    pub common_commands: Vec<String>,
}

/// Error node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorNode {
    /// Node ID
    pub id: String,
    /// Failed command
    pub command: String,
    /// Error message/output
    pub error: String,
    /// Recovery command that fixed it
    pub recovery: Option<String>,
    /// Timestamp
    pub timestamp: SystemTime,
}

/// Edge between nodes
#[derive(Debug, Clone)]
pub struct Edge {
    /// Source node ID
    pub from: String,
    /// Target node ID
    pub to: String,
    /// Edge type
    pub kind: EdgeKind,
    /// Edge weight
    pub weight: f32,
}

/// Edge types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EdgeKind {
    /// Command executed in project
    InProject,
    /// Commands in sequence
    Sequence,
    /// Similar commands
    Similar,
    /// Fixed by
    FixedBy,
}

/// Context graph database
pub struct ContextGraph {
    conn: Connection,
}

impl ContextGraph {
    /// Open or create a context graph at the given path
    ///
    /// # Errors
    /// Returns an error if the database cannot be opened or schema initialized
    pub fn open(path: &PathBuf) -> Result<Self, ContextError> {
        let conn = Connection::open(path)?;
        let graph = Self { conn };
        graph.init_schema()?;
        Ok(graph)
    }

    /// Create an in-memory context graph
    ///
    /// # Errors
    /// Returns an error if the in-memory database cannot be created
    pub fn open_in_memory() -> Result<Self, ContextError> {
        let conn = Connection::open_in_memory()?;
        let graph = Self { conn };
        graph.init_schema()?;
        Ok(graph)
    }

    fn init_schema(&self) -> SqlResult<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS nodes (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                data TEXT NOT NULL,
                created INTEGER NOT NULL,
                updated INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS edges (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                from_node TEXT NOT NULL,
                to_node TEXT NOT NULL,
                kind TEXT NOT NULL,
                weight REAL NOT NULL DEFAULT 1.0,
                FOREIGN KEY (from_node) REFERENCES nodes(id),
                FOREIGN KEY (to_node) REFERENCES nodes(id)
            );

            CREATE INDEX IF NOT EXISTS idx_edges_from ON edges(from_node);
            CREATE INDEX IF NOT EXISTS idx_edges_to ON edges(to_node);
            CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);
            ",
        )
    }

    /// Add a node to the graph
    ///
    /// # Errors
    /// Returns an error if the node cannot be serialized or inserted
    pub fn add_node(&mut self, node: &Node) -> Result<(), ContextError> {
        let (id, kind, data, created, updated) = match node {
            Node::Project(n) => (
                n.id.clone(),
                "project",
                serde_json::to_string(n)?,
                n.created,
                n.last_accessed,
            ),
            Node::Command(n) => (
                n.id.clone(),
                "command",
                serde_json::to_string(n)?,
                n.timestamp,
                n.timestamp,
            ),
            Node::Pattern(n) => (
                n.id.clone(),
                "pattern",
                serde_json::to_string(n)?,
                n.last_seen,
                n.last_seen,
            ),
            Node::Server(n) => (
                n.id.clone(),
                "server",
                serde_json::to_string(n)?,
                SystemTime::UNIX_EPOCH,
                SystemTime::UNIX_EPOCH,
            ),
            Node::Error(n) => (
                n.id.clone(),
                "error",
                serde_json::to_string(n)?,
                n.timestamp,
                n.timestamp,
            ),
        };

        let created_secs = i64::try_from(created.duration_since(SystemTime::UNIX_EPOCH)?.as_secs())
            .unwrap_or(i64::MAX);
        let updated_secs = i64::try_from(updated.duration_since(SystemTime::UNIX_EPOCH)?.as_secs())
            .unwrap_or(i64::MAX);

        self.conn.execute(
            "INSERT OR REPLACE INTO nodes (id, kind, data, created, updated) VALUES (?1, ?2, ?3, ?4, ?5)",
            (&id, kind, data, created_secs, updated_secs),
        )?;

        Ok(())
    }

    /// Get a node by ID
    ///
    /// # Errors
    /// Returns an error if the database query fails or deserialization fails
    pub fn get_node(&self, id: &str) -> Result<Option<Node>, ContextError> {
        let mut stmt = self
            .conn
            .prepare("SELECT kind, data FROM nodes WHERE id = ?1")?;

        let row = stmt.query_row([id], |row| {
            let kind: String = row.get(0)?;
            let data: String = row.get(1)?;
            Ok((kind, data))
        });

        match row {
            Ok((kind, data)) => {
                let node = match kind.as_str() {
                    "project" => Node::Project(serde_json::from_str(&data)?),
                    "command" => Node::Command(serde_json::from_str(&data)?),
                    "pattern" => Node::Pattern(serde_json::from_str(&data)?),
                    "server" => Node::Server(serde_json::from_str(&data)?),
                    "error" => Node::Error(serde_json::from_str(&data)?),
                    _ => return Err(ContextError::NotFound(format!("Unknown node kind: {kind}"))),
                };
                Ok(Some(node))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Add an edge
    ///
    /// # Errors
    /// Returns an error if the edge cannot be inserted
    pub fn add_edge(&mut self, edge: &Edge) -> Result<(), ContextError> {
        self.conn.execute(
            "INSERT INTO edges (from_node, to_node, kind, weight) VALUES (?1, ?2, ?3, ?4)",
            (
                &edge.from,
                &edge.to,
                format!("{:?}", edge.kind),
                edge.weight,
            ),
        )?;
        Ok(())
    }

    /// Get related nodes
    ///
    /// # Errors
    /// Returns an error if the database query fails
    #[allow(clippy::cast_possible_truncation)]
    pub fn get_related(&self, node_id: &str) -> Result<Vec<(Node, EdgeKind, f32)>, ContextError> {
        let mut stmt = self
            .conn
            .prepare("SELECT to_node, kind, weight FROM edges WHERE from_node = ?1")?;

        let rows = stmt.query_map([node_id], |row| {
            let to: String = row.get(0)?;
            let kind: String = row.get(1)?;
            let weight: f64 = row.get(2)?;
            Ok((to, kind, weight))
        })?;

        let mut results = Vec::new();
        for row in rows {
            let (to_id, kind_str, weight) = row?;
            if let Some(node) = self.get_node(&to_id)? {
                let kind = match kind_str.as_str() {
                    "InProject" => EdgeKind::InProject,
                    "Sequence" => EdgeKind::Sequence,
                    "FixedBy" => EdgeKind::FixedBy,
                    _ => EdgeKind::Similar,
                };
                results.push((node, kind, weight as f32));
            }
        }

        Ok(results)
    }

    /// Query commands by project
    ///
    /// # Errors
    /// Returns an error if the database query fails
    pub fn query_project_commands(
        &self,
        project_id: &str,
        limit: usize,
    ) -> Result<Vec<CommandNode>, ContextError> {
        let mut stmt = self.conn.prepare(
            "SELECT data FROM nodes WHERE kind = 'command' ORDER BY updated DESC LIMIT ?1",
        )?;

        let rows = stmt.query_map([i64::try_from(limit).unwrap_or(i64::MAX)], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        let mut results = Vec::new();
        for row in rows {
            let data: String = row?;
            if let Ok(Node::Command(cmd)) = serde_json::from_str::<Node>(&data) {
                if cmd.project_id.as_ref() == Some(&project_id.to_string()) {
                    results.push(cmd);
                }
            }
        }

        Ok(results)
    }

    /// Prune old nodes
    ///
    /// # Errors
    /// Returns an error if the database query fails
    pub fn prune(&mut self, before: SystemTime) -> Result<usize, ContextError> {
        let before_secs = i64::try_from(before.duration_since(SystemTime::UNIX_EPOCH)?.as_secs())
            .unwrap_or(i64::MAX);
        let count = self
            .conn
            .execute("DELETE FROM nodes WHERE updated < ?1", [before_secs])?;
        Ok(count)
    }

    /// Get graph statistics
    ///
    /// # Errors
    /// Returns an error if the database query fails
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn stats(&self) -> Result<GraphStats, ContextError> {
        let node_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))?;

        let edge_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM edges", [], |row| row.get(0))?;

        Ok(GraphStats {
            node_count: node_count as usize,
            edge_count: edge_count as usize,
        })
    }
}

/// Graph statistics
#[derive(Debug, Clone)]
pub struct GraphStats {
    /// Number of nodes
    pub node_count: usize,
    /// Number of edges
    pub edge_count: usize,
}
