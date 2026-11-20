//! An API's manual: the driver hands over the HTTP requests it made, the runner keeps them
//! redacted in the results, and the docs show each step's request as a `curl` command with the
//! response it got, where a screenshot would be.
#[path = "../examples/invoices/api.rs"]
mod api;

use expectation_harness::{Acceptance, Role};
use expectation_story::Status;
use std::path::{Path, PathBuf};

/// The invoices example, with a story that starts from an open invoice, so a test arranges one.
fn suite(name: &str) -> (Acceptance, PathBuf) {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&root);
    let src = root.join("suite");
    std::fs::create_dir_all(&src).unwrap();
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/invoices/invoices.expect");
    std::fs::copy(example, src.join("invoices.expect")).unwrap();
    std::fs::write(
        src.join("correct.expect"),
        "
Story: Void an invoice sent by mistake
  As an admin of the account
  I want to take back an invoice
  So that the customer isn't asked to pay it

  Given the invoice is open
  When the caller voids the invoice
  Then the invoice is void
",
    )
    .unwrap();
    let a = Acceptance::load(&src)
        .unwrap_or_else(|e| panic!("{e}"))
        .with_coverage();
    (a, root.join("docs"))
}

/// Every file under `dir`, as text.
fn every_file(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out = vec![];
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                todo.push(p);
            } else if let Ok(text) = std::fs::read_to_string(&p) {
                out.push((p, text));
            }
        }
    }
    out
}

#[test]
fn the_manual_shows_each_steps_request_and_keeps_nothing_secret() {
    let (a, docs) = suite("exchanges");
    let report = a.run(&mut api::driver(false), &api::options());
    assert!(report.passed(), "{}", report.failures(&a.suite, &a.plans));
    let i = |slug: &str| {
        a.plans
            .iter()
            .position(|p| p.slug.starts_with(slug))
            .unwrap()
    };

    // Each request is marked with what it did in its step.
    let billed = &report.results[i("bill-a-customer")];
    assert!(
        billed.start.exchanges.is_empty(),
        "nothing to set up or read: {:?}",
        billed.start.exchanges
    );
    let roles = |n: usize| -> Vec<(Role, String)> {
        billed.steps[n]
            .exchanges
            .iter()
            .map(|x| (x.role, x.label()))
            .collect()
    };
    let step = roles(0);
    assert_eq!(step.len(), 2, "{step:?}");
    assert_eq!(step[0], (Role::Action, "POST /v1/invoices → 201".into()));
    assert_eq!(step[1].0, Role::Check);
    assert!(step[1].1.starts_with("GET /v1/invoices/in_"), "{step:?}");
    let corrected = &report.results[i("void-an-invoice-sent-by-mistake")];
    let setup: Vec<String> = corrected
        .start
        .exchanges
        .iter()
        .filter(|x| x.role == Role::Setup)
        .map(|x| x.label())
        .collect();
    assert_eq!(setup.len(), 2, "created and finalized: {setup:?}");

    a.publish(&docs, "api", &report).unwrap();
    // Nothing secret anywhere in what is published, results included.
    let files = every_file(&docs);
    assert!(files.iter().any(|(p, _)| p.ends_with("results/api.tsv")));
    for (path, text) in &files {
        for secret in [
            api::ADMIN_KEY,
            api::MEMBER_KEY,
            "ada@example.com",
            "grace@example.com",
            "127.0.0.1",
            "X-Request-Id",
            "User-Agent",
        ] {
            assert!(!text.contains(secret), "{secret} in {}", path.display());
        }
    }
    let read = |page: &str| std::fs::read_to_string(docs.join(page)).unwrap();

    // The feature page shows the exception as the member saw it.
    let md = read("features/voiding.md");
    assert!(
        md.contains("/void\" \\\n  -H \"Authorization: Bearer $MEMBER_API_KEY\"\n```"),
        "{md}"
    );
    assert!(md.contains("```http\nHTTP/1.1 403 Forbidden\n"), "{md}");
    assert!(!md.contains("no-screenshot"), "{md}");
    let html = read("site/feature-voiding.html");
    assert!(html.contains("Bearer $MEMBER_API_KEY"), "{html}");
    assert!(html.contains(r#"<span class="status">HTTP/1.1 403 Forbidden</span>"#));
    assert!(!html.contains("no-screenshot"), "{html}");

    // A tutorial step shows the request it tells the reader to send, not the reads that checked it.
    let tutorial = read("tutorials/bill-a-customer.md");
    assert!(tutorial.contains("curl -X POST \"$BASE_URL/v1/invoices\""));
    assert!(
        !tutorial.contains("curl \"$BASE_URL/v1/invoices/in_"),
        "{tutorial}"
    );
    // The test page has those too, folded away.
    let test = read("site/test-bill-a-customer.html");
    assert!(test.contains("Requests that checked it (1)"), "{test}");
    let test = read(&format!(
        "site/test-{}.html",
        a.plans[i("void-an-invoice-sent-by-mistake")].slug
    ));
    assert!(test.contains("Requests that set it up (2)"), "{test}");

    // From the saved results alone, the same pages.
    let saved = expectation_story::load_results_for(&docs.join("results"), &a.plans, None);
    let again = docs.with_file_name("again");
    a.docs(Some(&saved)).write(&again).unwrap();
    for page in ["features/voiding.md", "tutorials/bill-a-customer.md"] {
        assert_eq!(
            std::fs::read_to_string(again.join(page)).unwrap(),
            read(page),
            "{page}"
        );
    }
}

#[test]
fn a_failure_shows_what_came_back() {
    let (a, docs) = suite("exchanges-bug");
    let report = a.run(&mut api::driver(true), &api::options());
    let p = a
        .plans
        .iter()
        .position(|p| p.slug.starts_with("ask-an-admin-to-void"))
        .unwrap();
    assert_eq!(report.results[p].status, Status::Failed);
    let void = &report.results[p].steps[2];
    let label = void.exchanges[0].label();
    assert!(
        label.starts_with("POST /v1/invoices/in_") && label.ends_with("/void → 200"),
        "{label}"
    );
    a.publish(&docs, "api", &report).unwrap();
    let md = std::fs::read_to_string(docs.join("tutorials/ask-an-admin-to-void.md")).unwrap();
    assert!(md.contains("*What came back when this failed.*"), "{md}");
}
