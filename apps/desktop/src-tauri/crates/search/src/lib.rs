//! Native code search for a single project — tantivy BM25 lexical search.
//!
//! Index layout: <data_dir>/search/<sha256(canonical project root)[..16]>/
//! Documents are ~50-line overlapping chunks with path + start_line stored,
//! so hits map back to file positions. `meta.json` beside the index tracks
//! per-file mtimes for incremental reindexing.

use ignore::WalkBuilder;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument};

const CHUNK_LINES: usize = 50;
const CHUNK_OVERLAP: usize = 5;
const MAX_FILE_BYTES: u64 = 1_000_000;
const INDEX_WRITER_HEAP: usize = 50_000_000;

#[derive(thiserror::Error, Debug)]
pub enum SearchError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("tantivy: {0}")]
    Tantivy(#[from] tantivy::TantivyError),
    #[error("query: {0}")]
    Query(#[from] tantivy::query::QueryParserError),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("project root not found: {0}")]
    MissingRoot(String),
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Hit {
    pub path: String,
    pub start_line: u32,
    pub score: f32,
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingChunk {
    pub path: String,
    pub start_line: u32,
    pub content_hash: String,
    pub content: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct IndexStats {
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub files_unchanged: usize,
    pub chunks: usize,
}

pub struct ProjectIndex {
    root: PathBuf,
    db: rusqlite::Connection,
    index: Index,
    reader: IndexReader,
    f_path: Field,
    f_content: Field,
    f_start: Field,
    meta_path: PathBuf,
}

/// Derive the per-project index directory under the app-private data dir.
pub fn index_dir(data_dir: &Path, project_root: &Path) -> PathBuf {
    let canon = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    let hash = hex16(&Sha256::digest(canon.to_string_lossy().as_bytes()));
    data_dir.join("search").join(hash)
}

fn hex16(bytes: &[u8]) -> String {
    bytes.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

impl ProjectIndex {
    pub fn open_or_create(data_dir: &Path, project_root: &Path) -> Result<Self, SearchError> {
        if !project_root.exists() {
            return Err(SearchError::MissingRoot(
                project_root.display().to_string(),
            ));
        }
        let dir = index_dir(data_dir, project_root);
        fs::create_dir_all(&dir)?;

        let mut schema_builder = Schema::builder();
        let f_path = schema_builder.add_text_field("path", STRING | STORED);
        let f_content = schema_builder.add_text_field("content", TEXT);
        let f_start = schema_builder.add_u64_field("start_line", STORED | INDEXED);
        let schema = schema_builder.build();

        let index = if dir.join("meta.json").exists() && dir.join("index").exists() {
            Index::open_in_dir(dir.join("index"))?
        } else {
            fs::create_dir_all(dir.join("index"))?;
            Index::create_in_dir(dir.join("index"), schema)?
        };
        // Schema fields are positional; reopen from disk still matches field order.
        let schema = index.schema();
        let (f_path, f_content, f_start) = (
            schema.get_field("path").unwrap_or(f_path),
            schema.get_field("content").unwrap_or(f_content),
            schema.get_field("start_line").unwrap_or(f_start),
        );

        let reader: IndexReader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        let db = rusqlite::Connection::open(dir.join("vectors.db"))?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS chunks(
                path TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                content_hash TEXT NOT NULL,
                PRIMARY KEY(path, start_line)
            );
            CREATE TABLE IF NOT EXISTS embeddings(
                model TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                dims INTEGER NOT NULL,
                vector BLOB NOT NULL,
                PRIMARY KEY(model, content_hash)
            );",
        )?;

        Ok(Self {
            root: project_root.to_path_buf(),
            db,
            index,
            reader,
            f_path,
            f_content,
            f_start,
            meta_path: dir.join("meta.json"),
        })
    }

    fn load_meta(&self) -> HashMap<String, u64> {
        fs::read_to_string(&self.meta_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_meta(&self, meta: &HashMap<String, u64>) -> Result<(), SearchError> {
        fs::write(&self.meta_path, serde_json::to_string(meta)?)?;
        Ok(())
    }

    /// Incrementally index the project: files whose mtime matches the stored
    /// metadata are skipped; new/changed files replace their previous chunks.
    pub fn index_project(&mut self) -> Result<IndexStats, SearchError> {
        let mut stats = IndexStats::default();
        let mut meta = self.load_meta();
        let mut writer: IndexWriter = self.index.writer(INDEX_WRITER_HEAP)?;
        let mut seen = std::collections::HashSet::new();
        let mut chunk_updates: Vec<(String, Vec<(u64, String)>)> = Vec::new();
        let mut chunk_stmt = self.db.prepare("SELECT DISTINCT path FROM chunks")?;
        let chunk_paths = chunk_stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<std::collections::HashSet<_>, _>>()?;
        drop(chunk_stmt);

        let walker = WalkBuilder::new(&self.root)
            .hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build();

        for entry in walker.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let rel = match path.strip_prefix(&self.root) {
                Ok(r) => r.to_string_lossy().replace('\\', "/"),
                Err(_) => continue,
            };
            seen.insert(rel.clone());
            let Ok(md) = path.metadata() else {
                stats.files_skipped += 1;
                continue;
            };
            if md.len() > MAX_FILE_BYTES || md.len() == 0 {
                stats.files_skipped += 1;
                continue;
            }
            let mtime = md
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if meta.get(&rel) == Some(&mtime) && chunk_paths.contains(&rel) {
                stats.files_unchanged += 1;
                continue;
            }
            let Ok(content) = fs::read_to_string(path) else {
                stats.files_skipped += 1;
                continue;
            };
            if content.as_bytes().iter().take(8192).any(|&b| b == 0) {
                stats.files_skipped += 1; // binary
                continue;
            }

            // Replace previous chunks for this file, then add fresh ones.
            writer.delete_term(tantivy::Term::from_field_text(self.f_path, &rel));
            let lines: Vec<&str> = content.lines().collect();
            let mut file_chunks = Vec::new();
            let step = CHUNK_LINES - CHUNK_OVERLAP;
            let mut start = 0usize;
            loop {
                let end = (start + CHUNK_LINES).min(lines.len());
                let body = lines[start..end].join("\n");
                let mut doc = TantivyDocument::new();
                doc.add_text(self.f_path, &rel);
                doc.add_text(self.f_content, &body);
                doc.add_u64(self.f_start, (start + 1) as u64);
                writer.add_document(doc)?;
                file_chunks.push(((start + 1) as u64, sha256_hex(body.as_bytes())));
                stats.chunks += 1;
                if end >= lines.len() {
                    break;
                }
                start += step;
            }
            chunk_updates.push((rel.clone(), file_chunks));
            meta.insert(rel, mtime);
            stats.files_indexed += 1;
        }

        // Drop chunks of files that no longer exist.
        let stale: Vec<String> = meta
            .keys()
            .filter(|p| !seen.contains(*p))
            .cloned()
            .collect();
        for rel in &stale {
            writer.delete_term(tantivy::Term::from_field_text(self.f_path, rel));
            meta.remove(rel);
        }

        writer.commit()?;
        self.reader.reload()?;

        let tx = self.db.transaction()?;
        for (rel, chunks) in &chunk_updates {
            tx.execute("DELETE FROM chunks WHERE path = ?1", params![rel])?;
            for (start_line, hash) in chunks {
                tx.execute(
                    "INSERT INTO chunks(path, start_line, content_hash) VALUES (?1, ?2, ?3)",
                    params![rel, *start_line as i64, hash],
                )?;
            }
        }
        for rel in &stale {
            tx.execute("DELETE FROM chunks WHERE path = ?1", params![rel])?;
        }
        tx.commit()?;

        self.save_meta(&meta)?;
        Ok(stats)
    }

    /// BM25 lexical search. `path:` terms can be embedded in the query by the
    /// caller; plain queries match content and path alike.
    pub fn search(&self, query_text: &str, limit: usize) -> Result<Vec<Hit>, SearchError> {
        let searcher = self.reader.searcher();
        let parser = QueryParser::for_index(&self.index, vec![self.f_content, self.f_path]);
        let query = parser.parse_query(query_text)?;
        let top = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut hits = Vec::new();
        for (score, addr) in top {
            let doc: TantivyDocument = searcher.doc(addr)?;
            let path = doc
                .get_first(self.f_path)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let start_line = doc
                .get_first(self.f_start)
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            hits.push(Hit {
                path,
                start_line,
                score,
                snippet: String::new(),
            });
        }
        // Fill snippets by re-reading the file lines around the hit — cheap at
        // search-time volume and guarantees the snippet reflects real content.
        for hit in &mut hits {
            let full = self.root.join(&hit.path);
            if let Ok(text) = fs::read_to_string(&full) {
                let lines: Vec<&str> = text.lines().collect();
                let lo = hit.start_line.saturating_sub(1) as usize;
                let hi = (lo + 12).min(lines.len());
                if lo < lines.len() {
                    hit.snippet = lines[lo..hi].join("\n");
                }
            }
        }
        // Merge duplicate (path, start_line) that can arise from overlap.
        hits.dedup_by(|a, b| a.path == b.path && a.start_line.abs_diff(b.start_line) < CHUNK_OVERLAP as u32);
        Ok(hits)
    }
    /// Chunks whose content hash has no embedding for `model` yet. Chunk text
    /// is reconstructed from the file at query time and verified against the
    /// stored hash, so a partially-applied edit can never embed stale content.
    pub fn pending_embeddings(&self, model: &str) -> Result<Vec<PendingChunk>, SearchError> {
        let mut stmt = self.db.prepare(
            "SELECT c.path, c.start_line, c.content_hash
             FROM chunks c
             LEFT JOIN embeddings e
               ON e.content_hash = c.content_hash AND e.model = ?1
             WHERE e.content_hash IS NULL
             ORDER BY c.path, c.start_line",
        )?;
        let rows = stmt
            .query_map(params![model], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)? as u32,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        let mut pending = Vec::new();
        for (path, start_line, content_hash) in rows {
            let full = self.root.join(&path);
            let Ok(text) = fs::read_to_string(&full) else { continue };
            let lines: Vec<&str> = text.lines().collect();
            let lo = start_line.saturating_sub(1) as usize;
            if lo >= lines.len() {
                continue;
            }
            let hi = (lo + CHUNK_LINES).min(lines.len());
            let content = lines[lo..hi].join("\n");
            if sha256_hex(content.as_bytes()) == content_hash {
                pending.push(PendingChunk {
                    path,
                    start_line,
                    content_hash,
                    content,
                });
            }
        }
        Ok(pending)
    }

    /// Persist embedding vectors keyed by content hash. Model is part of the
    /// primary key because dimensions and vector spaces are model-specific.
    pub fn store_embeddings(
        &mut self,
        model: &str,
        embeddings: &[(String, Vec<f32>)],
    ) -> Result<(), SearchError> {
        if embeddings.iter().any(|(_, vector)| vector.is_empty()) {
            return Err(SearchError::InvalidInput("embedding vectors must not be empty".into()));
        }
        let tx = self.db.transaction()?;
        for (hash, vector) in embeddings {
            tx.execute(
                "INSERT OR REPLACE INTO embeddings(model, content_hash, dims, vector)
                 VALUES (?1, ?2, ?3, ?4)",
                params![model, hash, vector.len() as i64, encode_vector(vector)],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Exact cosine kNN over cached vectors. Single-project indexes are small
    /// enough that a linear scan is simpler and more predictable than maintaining
    /// a mutable approximate graph; embedding cache hits dominate real cost.
    pub fn vector_search(
        &self,
        model: &str,
        query_vector: &[f32],
        limit: usize,
    ) -> Result<Vec<Hit>, SearchError> {
        if query_vector.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let mut stmt = self.db.prepare(
            "SELECT c.path, c.start_line, e.vector
             FROM chunks c
             JOIN embeddings e
               ON e.content_hash = c.content_hash AND e.model = ?1",
        )?;
        let rows = stmt
            .query_map(params![model], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)? as u32,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut hits = Vec::new();
        for (path, start_line, bytes) in rows {
            let vector = decode_vector(&bytes);
            if vector.len() != query_vector.len() {
                continue;
            }
            let score = cosine_similarity(query_vector, &vector);
            if score > 0.0 {
                hits.push(Hit {
                    path,
                    start_line,
                    score,
                    snippet: String::new(),
                });
            }
        }
        hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        hits.truncate(limit);
        self.fill_snippets(&mut hits);
        hits.dedup_by(|a, b| a.path == b.path && a.start_line.abs_diff(b.start_line) < CHUNK_OVERLAP as u32);
        Ok(hits)
    }

    fn fill_snippets(&self, hits: &mut [Hit]) {
        for hit in hits {
            let full = self.root.join(&hit.path);
            if let Ok(text) = fs::read_to_string(&full) {
                let lines: Vec<&str> = text.lines().collect();
                let lo = hit.start_line.saturating_sub(1) as usize;
                let hi = (lo + 12).min(lines.len());
                if lo < lines.len() {
                    hit.snippet = lines[lo..hi].join("\n");
                }
            }
        }
    }
}

/// Reciprocal rank fusion for lexical and vector result lists. RRF uses rank
/// rather than incompatible raw scores (BM25 magnitude vs cosine similarity).
pub fn reciprocal_rank_fusion(lexical: &[Hit], vector: &[Hit], limit: usize) -> Vec<Hit> {
    const RRF_K: f32 = 60.0;
    #[derive(Clone)]
    struct Acc {
        hit: Hit,
        score: f32,
    }
    let mut merged: HashMap<(String, u32), Acc> = HashMap::new();
    for (rank, hit) in lexical.iter().enumerate() {
        merged.insert(
            (hit.path.clone(), hit.start_line),
            Acc {
                hit: hit.clone(),
                score: 1.0 / (RRF_K + rank as f32 + 1.0),
            },
        );
    }
    for (rank, hit) in vector.iter().enumerate() {
        let key = (hit.path.clone(), hit.start_line);
        merged
            .entry(key)
            .and_modify(|acc| {
                acc.score += 1.0 / (RRF_K + rank as f32 + 1.0);
                if acc.hit.snippet.is_empty() {
                    acc.hit.snippet = hit.snippet.clone();
                }
            })
            .or_insert_with(|| Acc {
                hit: hit.clone(),
                score: 1.0 / (RRF_K + rank as f32 + 1.0),
            });
    }
    let mut out = merged.into_values().collect::<Vec<_>>();
    out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    out.truncate(limit);
    out.into_iter()
        .map(|mut acc| {
            acc.hit.score = acc.score;
            acc.hit
        })
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn encode_vector(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn decode_vector(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_project() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("proj");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/main.rs"),
            "fn main() {\n    let tally = compute_total();\n    println!(\"{tally}\");\n}\n\nfn compute_total() -> i32 {\n    41 + 1\n}\n",
        )
        .unwrap();
        fs::write(
            root.join("README.md"),
            "# Demo\n\nThis project computes totals and prints them.\n",
        )
        .unwrap();
        fs::write(root.join("binary.bin"), b"\x00\x01\x02").unwrap();
        (dir, root)
    }

    #[test]
    fn index_and_search_finds_content() {
        let (tmp, root) = fixture_project();
        let data = tmp.path().join("data");
        let mut idx = ProjectIndex::open_or_create(&data, &root).unwrap();
        let stats = idx.index_project().unwrap();
        assert_eq!(stats.files_indexed, 2, "binary file skipped"); // main.rs + README.md

        let hits = idx.search("compute_total", 5).unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].path, "src/main.rs");
        assert!(hits[0].start_line >= 1);
        assert!(hits[0].snippet.contains("compute_total"));
    }

    #[test]
    fn incremental_reindex_skips_unchanged_and_picks_up_edits() {
        let (tmp, root) = fixture_project();
        let data = tmp.path().join("data");
        let mut idx = ProjectIndex::open_or_create(&data, &root).unwrap();
        idx.index_project().unwrap();
        let stats = idx.index_project().unwrap();
        assert_eq!(stats.files_indexed, 0);
        assert_eq!(stats.files_unchanged, 2);

        std::thread::sleep(std::time::Duration::from_millis(1100));
        fs::write(
            root.join("src/main.rs"),
            "fn main() {\n    println!(\"zebra crossing\");\n}\n",
        )
        .unwrap();
        let stats = idx.index_project().unwrap();
        assert_eq!(stats.files_indexed, 1);
        let hits = idx.search("zebra", 5).unwrap();
        assert!(!hits.is_empty());
        // Stale content must be gone after replacement.
        let stale = idx.search("compute_total", 5).unwrap();
        assert!(stale.is_empty());
    }

    #[test]
    fn deleted_files_drop_out_of_index() {
        let (tmp, root) = fixture_project();
        let data = tmp.path().join("data");
        let mut idx = ProjectIndex::open_or_create(&data, &root).unwrap();
        idx.index_project().unwrap();
        fs::remove_file(root.join("README.md")).unwrap();
        idx.index_project().unwrap();
        let hits = idx.search("computes totals", 5).unwrap();
        assert!(hits.iter().all(|h| h.path != "README.md"));
    }

    #[test]
    fn legacy_tantivy_meta_without_chunk_rows_is_reindexed() {
        let (tmp, root) = fixture_project();
        let data = tmp.path().join("data");
        let mut idx = ProjectIndex::open_or_create(&data, &root).unwrap();
        idx.index_project().unwrap();
        // Simulate an index created before vector metadata existed: mtimes say
        // every file is fresh, but the chunks table is empty.
        idx.db.execute("DELETE FROM chunks", []).unwrap();
        idx.index_project().unwrap();
        assert!(
            !idx.pending_embeddings("model-a").unwrap().is_empty(),
            "unchanged legacy files must still populate chunk metadata once"
        );
    }

    #[test]
    fn vector_embeddings_roundtrip_and_ranked_by_cosine() {
        let (tmp, root) = fixture_project();
        let data = tmp.path().join("data");
        let mut idx = ProjectIndex::open_or_create(&data, &root).unwrap();
        idx.index_project().unwrap();
        let pending = idx.pending_embeddings("model-a").unwrap();
        assert!(!pending.is_empty());
        let embeddings = pending
            .iter()
            .map(|chunk| {
                let vector = if chunk.content.contains("compute_total") {
                    vec![1.0, 0.0]
                } else {
                    vec![0.0, 1.0]
                };
                (chunk.content_hash.clone(), vector)
            })
            .collect::<Vec<_>>();
        idx.store_embeddings("model-a", &embeddings).unwrap();
        let hits = idx.vector_search("model-a", &[1.0, 0.0], 5).unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].path, "src/main.rs");
        assert!(hits[0].score > 0.99);
        assert!(hits[0].snippet.contains("compute_total"));
    }

    #[test]
    fn embedding_cache_is_per_model_and_survives_incremental_reindex() {
        let (tmp, root) = fixture_project();
        let data = tmp.path().join("data");
        let mut idx = ProjectIndex::open_or_create(&data, &root).unwrap();
        idx.index_project().unwrap();
        let pending = idx.pending_embeddings("model-a").unwrap();
        let embeddings = pending
            .iter()
            .map(|chunk| (chunk.content_hash.clone(), vec![1.0, 0.0]))
            .collect::<Vec<_>>();
        idx.store_embeddings("model-a", &embeddings).unwrap();

        assert!(idx.pending_embeddings("model-a").unwrap().is_empty());
        assert!(!idx.pending_embeddings("model-b").unwrap().is_empty());

        idx.index_project().unwrap();
        assert!(
            idx.pending_embeddings("model-a").unwrap().is_empty(),
            "unchanged chunks reuse cached embeddings after reindex"
        );
    }

    #[test]
    fn reciprocal_rank_fusion_includes_vector_only_hits() {
        let lexical = vec![Hit {
            path: "src/lexical.rs".into(),
            start_line: 10,
            score: 8.0,
            snippet: "lexical".into(),
        }];
        let vector = vec![Hit {
            path: "src/semantic.rs".into(),
            start_line: 30,
            score: 0.91,
            snippet: "semantic".into(),
        }];
        let merged = reciprocal_rank_fusion(&lexical, &vector, 5);
        assert_eq!(merged.len(), 2);
        assert!(merged.iter().any(|h| h.path == "src/semantic.rs"));
        assert!(merged.iter().any(|h| h.path == "src/lexical.rs"));
    }

    #[test]
    fn reciprocal_rank_fusion_boosts_chunks_present_in_both_rankings() {
        let shared = Hit {
            path: "src/shared.rs".into(),
            start_line: 10,
            score: 8.0,
            snippet: "shared".into(),
        };
        let lexical_only = Hit {
            path: "src/lexical.rs".into(),
            start_line: 10,
            score: 7.0,
            snippet: "lexical".into(),
        };
        let vector_only = Hit {
            path: "src/semantic.rs".into(),
            start_line: 30,
            score: 0.91,
            snippet: "semantic".into(),
        };
        let lexical = vec![shared.clone(), lexical_only];
        let vector = vec![shared.clone(), vector_only];
        let merged = reciprocal_rank_fusion(&lexical, &vector, 5);
        assert_eq!(merged[0].path, "src/shared.rs");
    }

    #[test]
    fn missing_root_is_error() {
        let tmp = tempfile::tempdir().unwrap();
        let res = ProjectIndex::open_or_create(tmp.path(), Path::new("/no/such/place"));
        assert!(res.is_err());
    }
}
