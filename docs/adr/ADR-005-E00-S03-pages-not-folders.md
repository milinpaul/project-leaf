# ADR-005. Pages all the way down, no folders

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-28
- **Story:** E00-S03

## Context

Note apps either organise with folders (files inside directories) or let pages contain pages. Folders force
people to decide where something lives before writing, and a folder cannot hold its own notes.

## Decision

There are no folders. Any page can contain child pages, to any depth, and still has its own content. The
sidebar shows this page tree. The hierarchy lives in the workspace doc's `LoroTree`, with titles there too so
the sidebar renders without opening pages.

## Alternatives considered

- **Folders plus pages:** familiar from file systems, but two kinds of container is more to learn and maintain.
- **Flat pages with tags or links only:** flexible, but loses the outline people use to navigate.

## Consequences

- One concept to learn: a page. Moving a page moves its whole subtree.
- Deep trees must stay fast: the 500-page, 6-level acceptance test covers this.
- Trash works on subtrees: trashing a page hides its children too.
- Plain English: every page can hold other pages, like nested documents, instead of using separate folders.
