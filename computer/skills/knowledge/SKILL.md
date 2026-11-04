---
name: knowledge
description: "Find records across the Hub's servers by meaning and words, with text, image, audio or video queries."
---

# Knowledge

Each server that keeps searchable records serves one tool, `<slug>__search_knowledge`.
The search runs on that server, over its own
records, and returns only what the caller may read. There is no central index: to search
everything, call each server's tool and merge the hits yourself.

## Which servers and collections

A collection is named `<slug>.<plural>`. A server's resource
templates mark each of its collections in `_meta["ai.veoveo/knowledge"].collection`; list
the templates to see what a server searches. The servers whose tools list
`search_knowledge` are the ones to call.

## The query

```json
{
  "parts": [
    {"type": "text", "text": "a red kite over the dunes"},
    {"type": "image", "url": "<signed GET URL>", "mediaType": "image/png", "size": 48213}
  ],
  "collections": ["<slug>.<plural>"],
  "limit": 10
}
```

- `parts` is 1 to 16 parts, in order, each `text`, `image`, `audio` or `video`. All the
  parts together make one query. Any server takes any modality: an image query on a
  server of text records still ranks them by meaning.
- A media part is a signed GET URL the caller already holds, such as one a server's
  download tool returns, with its `mediaType`, `size` in bytes and, when the signature
  requires them, `headers`, all passed as given. Keep signed URLs out of notes, logs and
  answers.
- `collections` names at most 16 of the server's collections, each once. Leave it out to
  search all of them.
- `limit` is 1 to 20; it is 10 when left out.

## The hits

Each hit has the record's `uri`, its `collection`, a `title`, a `snippet` of at most 320
characters and, when the server knows it, `modifiedAt`. Hits come in rank order. Read a
hit with `resources/read` on its `uri` before relying on it: the snippet is a preview,
not the record.

Hits carry no score. When merging hits from several servers, keep each server's order
and interleave them.

## Fresh writes

A server finds a new or changed record by its words at once, and by its meaning once
the record is embedded. When you write a record directly, the write's result names a
Task in `_meta["io.modelcontextprotocol/related-task"]`; follow it with `tasks/get`.
When it has succeeded, the record is found by meaning too. If the Task fails, its
message names each record that could not be embedded and why. Those records are still
found by their words.
