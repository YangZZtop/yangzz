#[test]
fn smoke_indexes_yangzz_itself() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let g = yangzz_core::codegraph::CodeGraph::new(root);
    let total = g.index_all().expect("index");
    assert!(total > 100, "expected >100 symbols in yangzz-core, got {total}");
    let stats = g.stats();
    assert!(stats.files > 10);
    let hits = g.find("CodeGraph");
    assert!(!hits.is_empty(), "expected to find CodeGraph symbol");
    let refs = g.find_references("CodeGraphTool").expect("refs");
    assert!(refs.len() >= 2, "expected CodeGraphTool referenced in >=2 files, got {}", refs.len());
    eprintln!("smoke ok: files={} symbols={} hits={} refs={}", stats.files, stats.symbols, hits.len(), refs.len());
}
