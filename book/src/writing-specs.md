# Writing specs that read well

The generated manual is only as good as the words in the specs. A suite that tests well but
reads badly is half done: the same lines are the user's documentation. This is a loop that
works.

## 1. Start from what the user does

Write a handful of high-level expectations in the user's words, before any detail. Name
states for what the user sees ("the kettle is heating"), not for internals ("the relay is
closed"). Name events for what the user means ("the user starts the kettle"), not for the
control ("the user presses START"). The control goes in a `Say:`.

## 2. Let the checker shape the model

```bash
expectation check expectations
expectation graph expectations > flow.mmd
```

- A **conflict** is a question the spec hasn't answered. Decide which behaviour wins, and say
  so with `Overrides:`, or narrow a Given so they can't both apply.
- An **unreachable** expectation means nothing gets the system into its Given. Add the
  expectation that does, or fix the Background.
- **No way in** means a section's setup can't be reached from its parent's. The docs need that
  route to say "Before you begin".

## 3. Write stories as tutorials

One goal per story, in the order a new user meets things. The narrative says who and why.
The introduction is the tutorial's first paragraph. A story's Thens are what the reader
should notice, not everything that's true; the expectations already check the rest.

```bash
expectation plan expectations
```

shows each story as the test it compiles to, and errors when a story leans on behaviour the
expectations don't promise.

## 4. Give the docs words

Add `Say:` to events and phrases, `Set:` to states the reader changes, and `Shows:` to states
the reader sees. Turn long setups into phrases ("the kettle is plugged in and full"), so
"Before you begin" is one line, not a list of states. Define jargon with `Term:`.

## 5. Generate and read as a newcomer

```bash
expectation docs expectations docs
```

Read `docs/README.md` and the site as someone who has never seen the system. Better: have
someone else do it, or ask an AI agent to audit the pages as a reader and list what's vague,
missing or confusing.

## 6. Fix the specs, then loop

For each finding, change the spec, not the generated page:

| Finding | Fix |
| --- | --- |
| A vague step | a better `Say:`, a phrase, or a `Then` |
| A missing why | a description under the feature or expectation |
| A confusing title | a better name |
| A state dump in "Before you begin" | a setup phrase with a `Say:` |
| A word the reader doesn't know | a `Term:` |

Then check, regenerate and read again, until the docs read well.

## Style

Keep reader-facing text short and plain. Simplified Technical English (ASD-STE100) is a good
target: one instruction per sentence, active voice, present tense, common words, no more than
about 20 words a sentence. The docs generator already writes instructions this way, so specs
in the same style blend in.
