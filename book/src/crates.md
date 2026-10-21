# Crates

Expectation is a workspace of small crates with no dependencies outside the standard library.
Most projects only need `expectation_harness` (and `expectation_story` for the report types).

| Crate | Role |
| --- | --- |
| `expectation_model` | The vocabulary, the condition algebra, expectations and stories as plain data. |
| `expectation_language` | The `.expect` format: parsing, platforms, and resolving steps against the vocabulary. |
| `expectation_graph` | Conflicts, override precedence, the flow DAG, Mermaid and DOT export. |
| `expectation_story` | Stories → acceptance-test plans, coverage, results and verdicts. |
| `expectation_docs` | The manual, as an HTML site and Markdown. |
| `expectation_theme` | The colours, type and shapes the docs and graphs are drawn in. |
| `expectation_service` | External services a run needs: started once, stopped after, "not supported yet" when they can't be had. |
| `expectation_harness` | The `Driver` trait, `Bindings`, the runner, and the `Acceptance` pipeline. Re-exports the service crate as `expectation_harness::service`. |
| `expectation_cli` | The `expectation` command, as a binary and a library. |

Each crate depends only on the ones before it: `language`, `graph` and `story` build on
`model`; `docs` on those and `theme`; `harness` on everything, plus `service`; and `cli` on
everything but `harness`.

The [API reference](../api/expectation_harness/index.html) has every public type.
