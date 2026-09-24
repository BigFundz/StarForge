# Intelligent Template Marketplace Search

Natural language search, semantic matching, personalization, advanced
filtering, search suggestions, and usage analytics for the `/api/templates`
marketplace — implemented entirely in-process (no external ML/search
service required).

## How it works

| Concern | Implementation |
| --- | --- |
| Natural language search | `src/services/searchEngine.ts` tokenizes free text and strips stopwords ("I need something for...") so the meaningful words drive the match. |
| Semantic matching | A domain synonym graph (`SYNONYM_GROUPS`) expands each query token to related concepts — e.g. `swap` ⇄ `dex`/`amm`/`trade`, `voting` ⇄ `dao`/`governance`/`poll` — so templates match on meaning, not just exact words. |
| Ranking | TF-IDF vectors are built over each template's name/description/tags/functionality, and ranked against the (expanded) query vector via cosine similarity, with small boosts for exact name/tag matches. |
| Personalized results | `src/models/SearchAnalytics.ts` tracks per-user view/download interactions and derives a tag/author affinity, which nudges relevance scores in `/search` and drives `/discover/recommended`. |
| Advanced filtering | tags, `verified`, `license`, `min_quality` (rating), `min_downloads`, `author`, `date_from`/`date_to` — combinable with the query. |
| Search suggestions | `/search/suggestions` returns autocomplete from past popular queries plus matching template names/tags. |
| Usage analytics | Every search and every view/download is recorded; `/analytics/summary` surfaces top queries, trending template ids, and totals. |
| Find similar templates | `/​:id/similar` compares TF-IDF document vectors to surface related templates. |
| Discover trending | `/discover/trending` ranks by recency-weighted interaction volume, falling back to most-downloaded when there isn't enough activity yet. |

The TF-IDF index is cached and only rebuilt when the template corpus
actually changes (fingerprinted by `id:updatedAt`), so repeated searches
stay fast even as the catalog grows.

## API

### `POST /api/templates/search`

```json
{
  "query": "something for swapping tokens",
  "tags": ["defi"],
  "verified": true,
  "license": "MIT",
  "min_quality": 4,
  "min_downloads": 0,
  "author": "Stellar Community",
  "date_from": "2026-01-01",
  "date_to": "2026-12-31",
  "sort_by": "relevance",
  "limit": 20,
  "offset": 0
}
```

`sort_by` accepts `relevance` (default), `downloads`, `rating`, `recent`,
or `trending`. Pass an `Authorization: Bearer <token>` header to get
personalized ranking; the response includes `"personalized": true/false`
so clients can show a "based on your activity" hint.

Each result includes `match_score` (relevance) and `matched_terms` (which
expanded query terms it hit) for transparency/debugging.

### `GET /api/templates/search/suggestions?q=<prefix>&limit=10`

Autocomplete: past popular queries plus matching template names/tags.

### `GET /api/templates/discover/trending?limit=10&window_days=7`

Recently popular templates (weighted: download > click > view).

### `GET /api/templates/discover/recommended?limit=10`

Personalized picks for the authenticated user (`Authorization` header),
based on their view/download history. Anonymous users, or users without
enough history yet, get the most-downloaded templates instead
(`"personalized": false`).

### `GET /api/templates/:id/similar?limit=5`

Templates most similar to the given template id, by document-vector
similarity.

### `GET /api/templates/analytics/summary?limit=10`

Top search queries, trending template ids, and event totals.

## Extending the synonym graph

`SYNONYM_GROUPS` in `src/services/searchEngine.ts` is a flat list of
interchangeable single-word groups. Add a new group (or extend an
existing one) for any domain concept that should match across phrasing —
no need to touch the ranking logic.

## Tests

`src/tests/search.test.ts` covers natural language search, semantic
matching via synonyms, filter combinations, sorting, suggestions,
similarity, trending, and personalization (both for `/discover/recommended`
and for the general `/search` endpoint).

```bash
npm test
```
