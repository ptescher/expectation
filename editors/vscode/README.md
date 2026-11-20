# Expectation syntax for VS Code

Syntax highlighting for `.expect` files: keywords, steps, the vocabulary's values and ranges,
`[[links]]`, `{value}` and whole-line `#` comments. A `#` after `Approval:`, `Status:`, `Docs:`
or `Scopes:` is shown as an error, because the language has no trailing comments.

It is not on the Marketplace. To use it, link this directory into your extensions and reload
the window (the same works for Cursor and other VS Code builds, in their own extensions
directory):

```bash
ln -s "$(pwd)/editors/vscode" ~/.vscode/extensions/expectation-syntax
```

The grammar is a TextMate grammar ([`syntaxes/expect.tmLanguage.json`](syntaxes/expect.tmLanguage.json),
scope `source.expect`), so editors that read those (Sublime Text, the JetBrains TextMate
bundles plugin) can use the same file.

It only colours the text. `expectation check` is what says whether a suite is right.
