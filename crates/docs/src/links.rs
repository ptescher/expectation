//! A check of the links in a written site: every relative link in `site/*.html` must go to a
//! file that exists, and a `#fragment` to an `id` that the page has.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// The links in the HTML pages under `dir/site` that go nowhere, one line each:
/// `page: link`. Absolute links, `mailto:` and the like are not checked.
pub fn broken_links(dir: &Path) -> Vec<String> {
    let site = dir.join("site");
    let mut pages = vec![];
    collect(&site, &mut pages);
    pages.sort();
    // Each page's links and ids, read once.
    let mut ids: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    let mut links: Vec<(PathBuf, Vec<String>)> = vec![];
    for page in pages {
        let Ok(text) = std::fs::read_to_string(&page) else {
            continue;
        };
        let text = without_scripts(&text);
        ids.insert(page.clone(), attributes(&text, "id"));
        links.push((page, attributes(&text, "href").into_iter().collect()));
    }
    let mut broken = vec![];
    for (page, hrefs) in &links {
        let mut hrefs = hrefs.clone();
        hrefs.sort();
        for href in hrefs {
            if href.is_empty() || href.contains(':') || href.starts_with("//") {
                continue;
            }
            let (path, fragment) = match href.split_once('#') {
                Some((p, f)) => (p, Some(f)),
                None => (href.as_str(), None),
            };
            let target = if path.is_empty() {
                page.clone()
            } else {
                normalize(&page.parent().unwrap_or(&site).join(path))
            };
            let ok = match (ids.get(&target), fragment) {
                (Some(known), Some(f)) => f.is_empty() || known.contains(f),
                (Some(_), None) => true,
                (None, _) => target.exists(),
            };
            if !ok {
                let shown = page.strip_prefix(&site).unwrap_or(page).display();
                broken.push(format!("{shown}: {href}"));
            }
        }
    }
    broken
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "html") {
            out.push(p);
        }
    }
}

/// The page without its scripts, whose templates hold text that only looks like links.
fn without_scripts(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("<script") {
        out.push_str(&rest[..i]);
        rest = match rest[i..].find("</script>") {
            Some(j) => &rest[i + j + "</script>".len()..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

/// `a/b/../c` as `a/c`.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

/// The values of `name="…"` attributes, undoing the escapes the pages use.
fn attributes(text: &str, name: &str) -> HashSet<String> {
    let needle = format!("{name}=\"");
    let mut out = HashSet::new();
    let mut at = 0;
    while let Some(i) = text[at..].find(&needle) {
        let start = at + i;
        at = start + needle.len();
        // `data-id="…"` and the like are not the attribute.
        let before = text[..start].chars().next_back();
        if !before.is_some_and(char::is_whitespace) {
            continue;
        }
        if let Some(end) = text[at..].find('"') {
            out.insert(text[at..at + end].replace("&amp;", "&"));
            at += end;
        }
    }
    out
}
