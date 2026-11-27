//! Native code search for a single project — tantivy BM25 lexical search.
//!
//! Index layout: <data_dir>/search/<sha256(canonical project root)[..16]>/
//! Documents are ~50-line overlapping chunks with path + start_line stored,
//! so hits map back to file positions. `meta.json` beside the index tracks
//! per-file mtimes for incremental reindexing.

use ignore::WalkBuilder;
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Hit {
    pub path: String,
    pub start_line: u32,
    pub score: f32,
    pub snippet: String,
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

        Ok(Self {
            root: project_root.to_path_buf(),
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
            if meta.get(&rel) == Some(&mtime) {
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
                stats.chunks += 1;
                if end >= lines.len() {
                    break;
                }
                start += step;
            }
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
    fn missing_root_is_error() {
        let tmp = tempfile::tempdir().unwrap();
        let res = ProjectIndex::open_or_create(tmp.path(), Path::new("/no/such/place"));
        assert!(res.is_err());
    }
}
