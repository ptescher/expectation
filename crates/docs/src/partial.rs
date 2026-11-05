//! Partial manuals: just some sections, for a pull request's preview.
//!
//! A partial build writes the pages of the sections it is given, everything under them, and
//! their features, tests and tutorials. It skips the pages about the whole suite: the test report,
//! the maps, the glossary and the vocabulary. The navigation still shows the whole tree. A link to
//! a page that wasn't written goes to the full published manual when there is a base URL for it,
//! and reads as plain text when there isn't, so no link is broken. Checking, planning and
//! results are those of the whole suite.

use expectation_model::{SectionId, Suite};
use std::collections::HashSet;

/// Which sections a partial build writes, and where the full manual is.
#[derive(Clone, Debug, Default)]
pub struct Partial {
    /// The sections given and every section under them.
    pub sections: HashSet<SectionId>,
    /// The full manual's address, the directory `expectation docs` wrote it to
    /// (`https://example.com/manual/`). A link to a page this build doesn't write goes to the same
    /// page there. Without one, it reads as plain text.
    pub base_url: Option<String>,
}

impl Partial {
    /// The sections `given` and everything under them.
    pub fn of(suite: &Suite, given: &[SectionId], base_url: Option<String>) -> Self {
        let sections = given.iter().flat_map(|&s| suite.subtree(s)).collect();
        Partial { sections, base_url }
    }
}

/// The sections a `--section` value names: a section's directory (`02-billing/invoices`), its
/// slug, or its name, ignoring case. A name two sections share is an error that lists them.
pub fn find_sections(suite: &Suite, wanted: &str) -> Result<SectionId, String> {
    let w = wanted.trim().trim_matches('/');
    let lower = w.to_lowercase();
    let by_path: Vec<_> = suite.sections.iter().filter(|s| s.path == w).collect();
    if let [s] = by_path.as_slice() {
        return Ok(s.id);
    }
    if let Some(s) = suite.sections.iter().find(|s| !w.is_empty() && s.slug == w) {
        return Ok(s.id);
    }
    let by_name: Vec<_> = suite
        .sections
        .iter()
        .filter(|s| s.name.to_lowercase() == lower)
        .collect();
    match by_name.as_slice() {
        [s] => Ok(s.id),
        [] => {
            let some: Vec<String> = suite
                .sections
                .iter()
                .filter(|s| s.parent.is_some())
                .take(8)
                .map(|s| format!("{} ({})", s.name, s.path))
                .collect();
            Err(format!(
                "no section {wanted}: give a section's name, slug or directory, such as {}",
                some.join(", ")
            ))
        }
        many => Err(format!(
            "{wanted} names {} sections; give its directory: {}",
            many.len(),
            many.iter()
                .map(|s| s.path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// `link` (as written in a page in the directory `from`, relative to the docs directory) as a
/// path from the docs directory, if it is a page of the manual: not outside, not a file, not an
/// anchor on the same page.
fn page_path(from: &str, link: &str) -> Option<String> {
    if link.is_empty() || link.starts_with('#') || link.contains(':') || link.starts_with('/') {
        return None;
    }
    let path = link.split(['#', '?']).next()?;
    if !(path.ends_with(".html") || path.ends_with(".md")) {
        return None;
    }
    let mut parts: Vec<&str> = from.split('/').filter(|p| !p.is_empty()).collect();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    Some(parts.join("/"))
}

/// What a link to a page this build doesn't write becomes: the same page in the full manual, or
/// nothing (plain text).
fn elsewhere(base: Option<&str>, path: &str, link: &str) -> Option<String> {
    let base = base?;
    let fragment = link.find('#').map_or("", |i| &link[i..]);
    Some(format!("{}/{path}{fragment}", base.trim_end_matches('/')))
}

/// HTML from a page in `from` with its links to unbuilt pages fixed: `<a href>`s to the full
/// manual, or to plain text when there is no full manual to go to.
pub(crate) fn html(text: &str, from: &str, built: &HashSet<String>, base: Option<&str>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("<a ") {
        out.push_str(&rest[..at]);
        let tag_end = match rest[at..].find('>') {
            Some(e) => at + e + 1,
            None => break,
        };
        let tag = &rest[at..tag_end];
        let href = tag
            .split_once("href=\"")
            .and_then(|(_, h)| h.split_once('"'))
            .map(|(h, _)| h);
        let missing = href.and_then(|h| {
            page_path(from, h)
                .filter(|p| !built.contains(p))
                .map(|p| (h, p))
        });
        let Some((link, path)) = missing else {
            out.push_str(tag);
            rest = &rest[tag_end..];
            continue;
        };
        match elsewhere(base, &path, link) {
            Some(url) => {
                out.push_str(&tag.replacen(
                    &format!("href=\"{link}\""),
                    &format!("href=\"{url}\""),
                    1,
                ));
                rest = &rest[tag_end..];
            }
            None => {
                // Plain text: the link's content, without the link.
                let close = rest[tag_end..].find("</a>").map(|c| tag_end + c);
                match close {
                    Some(c) => {
                        out.push_str(r#"<span class="unbuilt" title="Not in this preview">"#);
                        out.push_str(&rest[tag_end..c]);
                        out.push_str("</span>");
                        rest = &rest[c + 4..];
                    }
                    None => {
                        out.push_str(tag);
                        rest = &rest[tag_end..];
                    }
                }
            }
        }
    }
    out.push_str(rest);
    out
}

/// Markdown from a page in `from` with its links to unbuilt pages fixed the same way: `[text](url)`
/// into the full manual, or `text`. Images (`![…](…)`) aren't pages, and stay.
pub(crate) fn markdown(
    text: &str,
    from: &str,
    built: &HashSet<String>,
    base: Option<&str>,
) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    let mut i = 0;
    while let Some(off) = text[i..].find("](") {
        let mid = i + off;
        // The `[` that opens this link's text, allowing for nested brackets and `\[`.
        let mut depth = 0;
        let mut open = None;
        let mut j = mid;
        while j > copied {
            j -= 1;
            let escaped = j > 0 && bytes[j - 1] == b'\\';
            match bytes[j] {
                b']' if !escaped => depth += 1,
                b'[' if !escaped => {
                    if depth == 0 {
                        open = Some(j);
                        break;
                    }
                    depth -= 1;
                }
                _ => {}
            }
        }
        let close = text[mid + 2..].find(')').map(|c| mid + 2 + c);
        let (Some(open), Some(close)) = (open, close) else {
            i = mid + 2;
            continue;
        };
        let image = open > 0 && bytes[open - 1] == b'!';
        let link = &text[mid + 2..close];
        let missing = page_path(from, link).filter(|p| !built.contains(p));
        if let (false, Some(path)) = (image, missing) {
            out.push_str(&text[copied..open]);
            let label = &text[open + 1..mid];
            match elsewhere(base, &path, link) {
                Some(url) => {
                    out.push('[');
                    out.push_str(label);
                    out.push_str("](");
                    out.push_str(&url);
                    out.push(')');
                }
                None => out.push_str(label),
            }
            copied = close + 1;
        }
        i = close + 1;
    }
    out.push_str(&text[copied..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn built(paths: &[&str]) -> HashSet<String> {
        paths.iter().map(|p| p.to_string()).collect()
    }

    #[test]
    fn links_to_unbuilt_pages_go_to_the_full_manual_or_become_text() {
        let have = built(&["site/feature-a.html", "features/a.md"]);
        let page = r##"<p><a href="feature-a.html#x">A</a> <a href="glossary.html#t">term</a> <a class="card" href="tests.html"><span>Report</span></a> <a href="https://x.org/">out</a> <a href="#here">here</a></p>"##;
        let base = html(
            page,
            "site",
            &have,
            Some("https://docs.example.com/manual/"),
        );
        assert!(base.contains(r#"<a href="feature-a.html#x">A</a>"#));
        assert!(base.contains(
            r#"<a href="https://docs.example.com/manual/site/glossary.html#t">term</a>"#
        ));
        assert!(base.contains(
            r#"<a class="card" href="https://docs.example.com/manual/site/tests.html">"#
        ));
        assert!(
            base.contains(r#"<a href="https://x.org/">out</a>"#)
                && base.contains(r##"<a href="#here">"##)
        );
        let plain = html(page, "site", &have, None);
        assert!(plain.contains(r#"<span class="unbuilt" title="Not in this preview">term</span>"#));
        assert!(plain.contains(
            r#"<span class="unbuilt" title="Not in this preview"><span>Report</span></span>"#
        ));
        assert!(plain.contains(r#"<a href="feature-a.html#x">A</a>"#));

        let md = "See [A](../features/a.md), the [report](../tests.md#t), ![shot](../screenshots/x.png) and [\\[odd\\] name](../glossary.md).";
        let with = markdown(md, "tests", &have, Some("https://d.example/m"));
        assert_eq!(
            with,
            "See [A](../features/a.md), the [report](https://d.example/m/tests.md#t), ![shot](../screenshots/x.png) and [\\[odd\\] name](https://d.example/m/glossary.md)."
        );
        let without = markdown(md, "tests", &have, None);
        assert_eq!(
            without,
            "See [A](../features/a.md), the report, ![shot](../screenshots/x.png) and \\[odd\\] name."
        );
    }

    #[test]
    fn paths_are_resolved_from_the_page() {
        assert_eq!(
            page_path("site", "feature-a.html#x").as_deref(),
            Some("site/feature-a.html")
        );
        assert_eq!(
            page_path("tests", "../features/a.md").as_deref(),
            Some("features/a.md")
        );
        assert_eq!(page_path("", "tests.md").as_deref(), Some("tests.md"));
        assert_eq!(page_path("site", "../screenshots/x.png"), None);
        assert_eq!(page_path("site", "https://x.org/a.html"), None);
        assert_eq!(page_path("", "../../outside.md"), None);
    }
}
