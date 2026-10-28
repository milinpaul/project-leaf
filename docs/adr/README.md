# Decision records

One short file per significant decision, so we can see later what we chose and why.
Records cover two kinds of decision:

- **Architecture:** crates, libraries, data model, storage, testing approach.
- **Feature:** product behaviour that is not obvious from the spec, or that changes it (for example how a key behaves in an edge case).

## How to add one

1. Copy [0000-template.md](0000-template.md) to `NNNN-short-title.md`, using the next free number.
2. Fill it in. Keep it to one screen.
3. Add a row to the index below in the same PR as the change it describes.
4. Never rewrite an accepted record. If a decision changes, write a new one and mark the old one `Superseded by NNNN`.

## Index

| # | Title | Type | Status |
|---|---|---|---|
| — | _No records yet_ | | |
