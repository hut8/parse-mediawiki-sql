use bstr::ByteSlice;
use memmap::Mmap;
use parse_mediawiki_sql::{
    iterate_sql_insertions,
    schemas::{Page, Redirect},
    types::PageNamespace,
};
use std::collections::BTreeMap as Map;
use std::fs::File;

unsafe fn memory_map(path: &str) -> Mmap {
    Mmap::map(
        &File::open(path)
            .unwrap_or_else(|e| panic!("Failed to open {}: {}", &path, e)),
    )
    .unwrap_or_else(|e| panic!("Failed to memory-map {}: {}", &path, e))
}

// Expects page.sql and redirect.sql in the current directory.
// Generates JSON: { target: [source1, source2, source3, ...], ...}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).take(2).collect();
    let page_sql = unsafe {
        memory_map(args.get(0).map(String::as_str).unwrap_or("page.sql"))
    };
    let redirect_sql = unsafe {
        memory_map(args.get(1).map(String::as_str).unwrap_or("redirect.sql"))
    };
    let mut pages = iterate_sql_insertions::<Page>(&page_sql);
    let template_namespace = PageNamespace::from(10);
    // This works if every template redirect in redirect.sql is also marked
    // as a redirect in page.sql.
    let id_to_title: Map<_, _> = pages
        .filter(
            |Page {
                 namespace,
                 is_redirect,
                 ..
             }| *is_redirect && *namespace == template_namespace,
        )
        .map(|Page { id, title, .. }| (id, title))
        .collect();
    assert_eq!(
        pages
            .finish()
            .map(|(input, _)| input.chars().take(4).collect::<String>()),
        Ok(";\n/*".into())
    );
    let mut redirects = iterate_sql_insertions::<Redirect>(&redirect_sql);
    let target_to_sources: Map<_, _> = redirects
        .filter_map(|Redirect { from, title, .. }| {
            id_to_title.get(&from).map(|from| (from, title))
        })
        .fold(Map::new(), |mut map, (from, title)| {
            let entry = map.entry(title.into_inner()).or_insert_with(Vec::new);
            entry.push(from.clone().into_inner());
            map
        });
    assert_eq!(
        redirects
            .finish()
            .map(|(input, _)| input.chars().take(4).collect::<String>()),
        Ok(";\n/*".into())
    );
    serde_json::to_writer(std::io::stdout(), &target_to_sources).unwrap();
}