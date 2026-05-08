//! CodeGraph: tree-sitter based AST-level code understanding.
//!
//! Extracts symbols (functions, structs, classes, traits) from Rust / TypeScript / Python
//! source files, builds an in-memory index, and answers queries like:
//! - "which symbols are defined in this directory?"
//! - "where is symbol X defined?"
//! - "which files reference symbol X?" (approximate, text-level)

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    Impl,
    Class,
    Interface,
    Type,
    Const,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::Impl => "impl",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Type => "type",
            Self::Const => "const",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub file: PathBuf,
    pub line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lang {
    Rust,
    TypeScript,
    Tsx,
    Python,
}

impl Lang {
    fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()? {
            "rs" => Some(Self::Rust),
            "ts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            "py" => Some(Self::Python),
            _ => None,
        }
    }

    fn language(self) -> Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
        }
    }

    /// Tree-sitter query extracting (name, kind) pairs.
    /// Captures are named `@name` with a co-named metadata capture via predicate — here we
    /// use a small per-language query that tags each kind explicitly via the pattern index.
    fn query_source(self) -> &'static str {
        match self {
            Self::Rust => RUST_QUERY,
            Self::TypeScript | Self::Tsx => TS_QUERY,
            Self::Python => PYTHON_QUERY,
        }
    }

    /// Map each pattern index (in declaration order within `query_source`) to a SymbolKind.
    fn pattern_kinds(self) -> &'static [SymbolKind] {
        match self {
            Self::Rust => &[
                SymbolKind::Function,
                SymbolKind::Struct,
                SymbolKind::Enum,
                SymbolKind::Trait,
                SymbolKind::Type,
                SymbolKind::Const,
                SymbolKind::Impl,
            ],
            Self::TypeScript | Self::Tsx => &[
                SymbolKind::Function,
                SymbolKind::Class,
                SymbolKind::Interface,
                SymbolKind::Type,
                SymbolKind::Method,
            ],
            Self::Python => &[SymbolKind::Function, SymbolKind::Class],
        }
    }
}

const RUST_QUERY: &str = r#"
(function_item name: (identifier) @name)
(struct_item name: (type_identifier) @name)
(enum_item name: (type_identifier) @name)
(trait_item name: (type_identifier) @name)
(type_item name: (type_identifier) @name)
(const_item name: (identifier) @name)
(impl_item type: (type_identifier) @name)
"#;

const TS_QUERY: &str = r#"
(function_declaration name: (identifier) @name)
(class_declaration name: (type_identifier) @name)
(interface_declaration name: (type_identifier) @name)
(type_alias_declaration name: (type_identifier) @name)
(method_definition name: (property_identifier) @name)
"#;

const PYTHON_QUERY: &str = r#"
(function_definition name: (identifier) @name)
(class_definition name: (identifier) @name)
"#;

#[derive(Default)]
struct CachedFile {
    mtime: Option<SystemTime>,
    symbols: Vec<Symbol>,
}

pub struct CodeGraph {
    root: PathBuf,
    cache: Mutex<HashMap<PathBuf, CachedFile>>,
}

impl CodeGraph {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Index (or refresh) every supported file under root. Returns total symbols.
    pub fn index_all(&self) -> Result<usize> {
        let files = collect_source_files(&self.root);
        let mut total = 0;
        for f in files {
            total += self.index_file(&f)?.len();
        }
        Ok(total)
    }

    /// Parse one file, using mtime cache. Returns its symbols.
    pub fn index_file(&self, path: &Path) -> Result<Vec<Symbol>> {
        let abs = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };

        let mtime = fs::metadata(&abs).and_then(|m| m.modified()).ok();
        {
            let cache = self.cache.lock().unwrap();
            if let Some(entry) = cache.get(&abs)
                && entry.mtime == mtime
            {
                return Ok(entry.symbols.clone());
            }
        }

        let Some(lang) = Lang::from_path(&abs) else {
            return Ok(vec![]);
        };
        let source = fs::read_to_string(&abs)?;
        let symbols = parse_symbols(lang, &abs, &source)?;

        let mut cache = self.cache.lock().unwrap();
        cache.insert(
            abs,
            CachedFile {
                mtime,
                symbols: symbols.clone(),
            },
        );
        Ok(symbols)
    }

    /// All symbols currently in the cache (does not re-scan).
    pub fn all_symbols(&self) -> Vec<Symbol> {
        let cache = self.cache.lock().unwrap();
        cache.values().flat_map(|c| c.symbols.clone()).collect()
    }

    /// Find symbols whose name contains `needle` (case-insensitive).
    pub fn find(&self, needle: &str) -> Vec<Symbol> {
        let needle = needle.to_lowercase();
        self.all_symbols()
            .into_iter()
            .filter(|s| s.name.to_lowercase().contains(&needle))
            .collect()
    }

    /// Text-level search for files referencing `symbol_name`. Not precise — misses aliases,
    /// catches false positives — but fast and good enough for a first-pass "what would this change break?".
    pub fn find_references(&self, symbol_name: &str) -> Result<Vec<PathBuf>> {
        let files = collect_source_files(&self.root);
        let mut hits = Vec::new();
        for f in files {
            if let Ok(src) = fs::read_to_string(&f)
                && contains_word(&src, symbol_name)
            {
                hits.push(f);
            }
        }
        Ok(hits)
    }

    /// Summary: total files indexed, symbols by kind.
    pub fn stats(&self) -> GraphStats {
        let symbols = self.all_symbols();
        let files = {
            let cache = self.cache.lock().unwrap();
            cache.len()
        };
        let mut by_kind: HashMap<&'static str, usize> = HashMap::new();
        for s in &symbols {
            *by_kind.entry(s.kind.as_str()).or_insert(0) += 1;
        }
        GraphStats {
            files,
            symbols: symbols.len(),
            by_kind,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GraphStats {
    pub files: usize,
    pub symbols: usize,
    pub by_kind: HashMap<&'static str, usize>,
}

fn parse_symbols(lang: Lang, file: &Path, source: &str) -> Result<Vec<Symbol>> {
    let language = lang.language();
    let mut parser = Parser::new();
    parser.set_language(&language)?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter parse returned None for {:?}", file))?;

    let query = Query::new(&language, lang.query_source())?;
    let kinds = lang.pattern_kinds();

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    let mut symbols = Vec::new();
    while let Some(m) = matches.next() {
        let kind = *kinds
            .get(m.pattern_index)
            .unwrap_or(&SymbolKind::Function);
        for cap in m.captures {
            let node = cap.node;
            let name = node.utf8_text(source.as_bytes())?.to_string();
            let line = node.start_position().row + 1;
            symbols.push(Symbol {
                name,
                kind,
                file: file.to_path_buf(),
                line,
            });
        }
    }
    Ok(symbols)
}

fn collect_source_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(root, &mut out);
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.')
            || name == "target"
            || name == "node_modules"
            || name == "dist"
            || name == "build"
            || name == ".git"
        {
            continue;
        }
        if path.is_dir() {
            walk(&path, out);
        } else if Lang::from_path(&path).is_some() {
            out.push(path);
        }
    }
}

/// Whole-word containment check — avoids matching `foo` inside `foobar`.
fn contains_word(hay: &str, needle: &str) -> bool {
    let bytes = hay.as_bytes();
    let nb = needle.as_bytes();
    if nb.is_empty() || nb.len() > bytes.len() {
        return false;
    }
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut i = 0;
    while i + nb.len() <= bytes.len() {
        if &bytes[i..i + nb.len()] == nb {
            let before_ok = i == 0 || !is_ident(bytes[i - 1]);
            let after_idx = i + nb.len();
            let after_ok = after_idx == bytes.len() || !is_ident(bytes[after_idx]);
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn extracts_rust_symbols() {
        let tmp = tempdir();
        write_file(
            &tmp,
            "lib.rs",
            "pub struct Foo;\nimpl Foo { fn bar(&self) {} }\npub fn top() {}\n",
        );
        let graph = CodeGraph::new(&tmp);
        graph.index_all().unwrap();
        let syms = graph.all_symbols();
        let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Foo"));
        assert!(names.contains(&"top"));
        assert!(names.contains(&"bar"));
    }

    #[test]
    fn find_filters_by_name() {
        let tmp = tempdir();
        write_file(&tmp, "a.rs", "fn alpha() {}\nfn beta() {}\n");
        let graph = CodeGraph::new(&tmp);
        graph.index_all().unwrap();
        assert_eq!(graph.find("alp").len(), 1);
        assert_eq!(graph.find("ET").len(), 1);
    }

    #[test]
    fn contains_word_is_whole_word() {
        assert!(contains_word("call foo()", "foo"));
        assert!(!contains_word("foobar", "foo"));
        assert!(contains_word("x.foo = 1", "foo"));
    }

    #[test]
    fn extracts_python_class_and_fn() {
        let tmp = tempdir();
        write_file(
            &tmp,
            "m.py",
            "def top():\n    pass\n\nclass Bar:\n    def inner(self):\n        pass\n",
        );
        let graph = CodeGraph::new(&tmp);
        graph.index_all().unwrap();
        let names: Vec<String> = graph.all_symbols().into_iter().map(|s| s.name).collect();
        assert!(names.contains(&"top".to_string()));
        assert!(names.contains(&"Bar".to_string()));
        assert!(names.contains(&"inner".to_string()));
    }

    fn tempdir() -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "yangzz-cg-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&base).unwrap();
        base
    }
}
