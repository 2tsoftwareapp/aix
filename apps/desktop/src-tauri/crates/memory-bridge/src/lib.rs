//! memory-bridge: embeds ai-memory (store + wiki) as the harness's native
//! long-term memory, scoped per project the desktop app works on.
//!
//! The embedded store is app-private: it lives in the app's own data dir and
//! does NOT share state with any machine-wide ai-memory service. Pages are
//! markdown-in-git (the wiki is the source of truth); SQLite is the derived
//! FTS index. Writes go through the wiki so the two never diverge.

use ai_memory_core::{ActorContext, PagePath, ProjectId, Tier, WorkspaceId};
use ai_memory_store::Store;
use ai_memory_wiki::{Wiki, WritePageRequest};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("store error: {0}")]
    Store(#[from] ai_memory_store::StoreError),
    #[error("wiki error: {0}")]
    Wiki(#[from] ai_memory_wiki::WikiError),
    #[error("core error: {0}")]
    Core(#[from] ai_memory_core::MemoryError),
    #[error("lock poisoned")]
    LockPoisoned,
}

#[derive(Debug)]
pub struct MemoryHit {
    pub path: String,
    pub title: String,
    pub snippet: String,
}

pub struct MemoryPage {
    pub title: String,
    pub body: String,
}

/// Embedded ai-memory instance. Cheap to clone-wrap in Arc — both the writer
/// handle and reader pool are cloneable by design.
pub struct MemoryBridge {
    wiki: Wiki,
    store: Store,
    scopes: Mutex<HashMap<String, (WorkspaceId, ProjectId)>>,
}

const WORKSPACE_NAME: &str = "aix-harness";

impl MemoryBridge {
    /// Open (or create) the embedded memory store under `data_dir`.
    /// Runs migrations synchronously at open; the wiki root is initialized
    /// with its git checkpoint layer.
    pub fn open(data_dir: &Path) -> Result<Self, BridgeError> {
        let store = Store::open(data_dir)?;
        let wiki = Wiki::new(data_dir, store.writer.clone())?;
        Ok(Self {
            wiki,
            store,
            scopes: Mutex::new(HashMap::new()),
        })
    }

    /// Resolve (creating on first use) the workspace/project pair for one
    /// project key — the desktop app's project root path. Resolution is
    /// cached; the store key is stable across runs.
    async fn scope(&self, project_key: &str) -> Result<(WorkspaceId, ProjectId), BridgeError> {
        if let Some(pair) = self
            .scopes
            .lock()
            .map_err(|_| BridgeError::LockPoisoned)?
            .get(project_key)
            .copied()
        {
            return Ok(pair);
        }
        let ws = self
            .store
            .writer
            .get_or_create_workspace(WORKSPACE_NAME)
            .await?;
        let name = Path::new(project_key)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(project_key)
            .to_string();
        let proj = self
            .store
            .writer
            .get_or_create_project(ws, name, Some(project_key.to_string()))
            .await?;
        self.scopes
            .lock()
            .map_err(|_| BridgeError::LockPoisoned)?
            .insert(project_key.to_string(), (ws, proj));
        Ok((ws, proj))
    }

    /// Write or replace a durable memory page. Goes through the wiki so the
    /// markdown file and the FTS index stay in lockstep.
    pub async fn write_page(
        &self,
        project_key: &str,
        path: &str,
        title: &str,
        body: &str,
    ) -> Result<(), BridgeError> {
        let (ws, proj) = self.scope(project_key).await?;
        self.wiki
            .write_page(WritePageRequest {
                workspace_id: ws,
                project_id: proj,
                path: PagePath::new(path)?,
                frontmatter: serde_json::Value::Null,
                body: body.to_string(),
                tier: Tier::Semantic,
                pinned: false,
                title: Some(title.to_string()),
                admission_ctx: None,
                author_id: None,
                actor: ActorContext {
                    agent: Some("aix-desktop".to_string()),
                    ..ActorContext::default()
                },
                evidence: Vec::new(),
                trust_class: None,
            })
            .await?;
        Ok(())
    }

    /// Full-text search over one project's memory pages.
    pub async fn search(
        &self,
        project_key: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryHit>, BridgeError> {
        let (ws, proj) = self.scope(project_key).await?;
        let hits = self
            .store
            .reader
            .search_pages_for_project(ws, proj, query.to_string(), limit, None)
            .await?;
        Ok(hits
            .into_iter()
            .map(|h| MemoryHit {
                path: h.path.as_str().to_string(),
                title: h.title,
                snippet: h.snippet,
            })
            .collect())
    }

    /// Read one page's full body back from the wiki tree.
    pub fn read_page(&self, project_key_scope: (WorkspaceId, ProjectId), path: &str) -> Result<Option<MemoryPage>, BridgeError> {
        let (ws, proj) = project_key_scope;
        let page_path = PagePath::new(path)?;
        match self.wiki.read_page(ws, proj, &page_path) {
            Ok(md) => Ok(Some(MemoryPage {
                title: ai_memory_wiki::derive_title(&md.frontmatter, &md.body, &page_path),
                body: md.body,
            })),
            Err(_) => Ok(None),
        }
    }

    /// Recall full page bodies relevant to a hint — the pre-run injection path.
    /// Returns at most `limit` pages; empty store yields nothing.
    pub async fn recall(
        &self,
        project_key: &str,
        hint: &str,
        limit: usize,
    ) -> Result<Vec<MemoryPage>, BridgeError> {
        let scope = self.scope(project_key).await?;
        let hits = self
            .store
            .reader
            .search_pages_for_project(scope.0, scope.1, hint.to_string(), limit, None)
            .await?;
        let mut pages = Vec::new();
        for hit in hits {
            if let Ok(Some(page)) = self.read_page(scope, hit.path.as_str()) {
                pages.push(page);
            }
        }
        Ok(pages)
    }

    /// Expose the scope resolution for callers that need repeated reads.
    pub async fn scope_for(&self, project_key: &str) -> Result<(WorkspaceId, ProjectId), BridgeError> {
        self.scope(project_key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("aix-membridge-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn write_search_read_roundtrip() {
        let dir = tempdir("roundtrip");
        let bridge = MemoryBridge::open(&dir).expect("open");
        bridge
            .write_page("/proj/a", "decisions/db", "Database choice", "Use SQLite for local state")
            .await
            .expect("write");
        let hits = bridge.search("/proj/a", "SQLite", 5).await.expect("search");
        assert_eq!(hits.len(), 1, "search must find the written page: {hits:?}");
        assert_eq!(hits[0].title, "Database choice");
        let scope = bridge.scope_for("/proj/a").await.unwrap();
        let page = bridge.read_page(scope, "decisions/db").unwrap().expect("page");
        assert!(page.body.contains("SQLite"));
    }

    #[tokio::test]
    async fn recall_returns_full_bodies() {
        let dir = tempdir("recall");
        let bridge = MemoryBridge::open(&dir).expect("open");
        bridge
            .write_page("/proj/a", "lessons/gates", "Gate lesson", "always evaluate gates before runs")
            .await
            .unwrap();
        let pages = bridge.recall("/proj/a", "gates", 3).await.unwrap();
        assert_eq!(pages.len(), 1);
        assert!(pages[0].body.contains("evaluate gates"));
    }

    #[tokio::test]
    async fn projects_are_isolated() {
        let dir = tempdir("isolation");
        let bridge = MemoryBridge::open(&dir).expect("open");
        bridge
            .write_page("/proj/a", "p", "T", "shared-token-zzz")
            .await
            .unwrap();
        assert!(bridge.search("/proj/b", "shared-token-zzz", 5).await.unwrap().is_empty());
        assert_eq!(bridge.search("/proj/a", "shared-token-zzz", 5).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn rewrite_replaces_not_duplicates() {
        let dir = tempdir("rewrite");
        let bridge = MemoryBridge::open(&dir).expect("open");
        bridge.write_page("/proj/a", "p", "v1", "alpha").await.unwrap();
        bridge.write_page("/proj/a", "p", "v2", "beta").await.unwrap();
        let hits = bridge.search("/proj/a", "beta", 5).await.unwrap();
        assert_eq!(hits.len(), 1);
    }
}
