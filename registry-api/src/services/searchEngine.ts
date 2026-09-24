import { ITemplate } from "../models/Template";

/**
 * Intelligent search engine for the template marketplace.
 *
 * Implements:
 *  - Natural language tokenization
 *  - Lightweight semantic query expansion via a domain synonym graph
 *  - TF-IDF vectorization + cosine similarity ranking (no external ML
 *    service required, so it works fully offline / in-process)
 *  - "Find similar templates" via document-vector similarity
 *  - Structured filtering (tags, verified, license, rating, downloads,
 *    author, date range)
 */

// ---------------------------------------------------------------------------
// Tokenization
// ---------------------------------------------------------------------------

const STOPWORDS = new Set([
  "a", "an", "the", "is", "are", "was", "were", "be", "been", "being",
  "to", "of", "in", "on", "at", "for", "with", "and", "or", "but", "not",
  "that", "this", "these", "those", "i", "me", "my", "we", "our", "you",
  "your", "it", "its", "as", "by", "from", "if", "so", "do", "does", "did",
  "can", "could", "would", "should", "will", "shall", "want", "wants",
  "wanted", "need", "needs", "needed", "looking", "look", "find", "finding",
  "show", "give", "get", "have", "has", "had", "please", "some", "any",
  "all", "just", "like", "similar", "about", "using", "use", "based",
]);

/** Split free text into lowercase alphanumeric tokens. */
export function tokenize(text: string): string[] {
  return (text || "")
    .toLowerCase()
    .replace(/[^a-z0-9\s]/g, " ")
    .split(/\s+/)
    .filter(Boolean);
}

/** Tokenize and drop stopwords / single-character noise. */
export function meaningfulTokens(text: string): string[] {
  return tokenize(text).filter((t) => !STOPWORDS.has(t) && t.length > 1);
}

// ---------------------------------------------------------------------------
// Semantic expansion (domain synonym graph)
// ---------------------------------------------------------------------------
// Groups of interchangeable terms relevant to a smart-contract template
// marketplace. Expanding a query with these lets "find something for
// trading tokens" match templates tagged `dex`/`swap`/`amm` even though
// none of those exact words were typed.

const SYNONYM_GROUPS: string[][] = [
  ["token", "asset", "currency", "coin", "fungible"],
  ["nft", "collectible", "art"],
  ["defi", "finance", "decentralized"],
  ["dex", "exchange", "swap", "amm", "trade", "trading"],
  ["lending", "borrowing", "loan", "credit", "collateral"],
  ["dao", "governance", "voting", "proposal", "poll", "election"],
  ["wallet", "account", "multisig", "signature", "threshold"],
  ["security", "audit", "safety", "protection"],
  ["staking", "stake", "rewards", "yield"],
  ["oracle", "price", "feed", "data"],
  ["bridge", "cross", "chain", "interop"],
  ["marketplace", "market", "store"],
  ["escrow", "holding", "custody"],
  ["vesting", "lock", "timelock", "schedule"],
  ["airdrop", "distribution", "giveaway"],
  ["auction", "bidding", "bid"],
  ["game", "gaming", "play"],
  ["auth", "authentication", "login", "identity"],
  ["payment", "pay", "transfer", "remittance"],
  ["subscription", "recurring", "billing"],
];

function buildSynonymMap(): Map<string, Set<string>> {
  const map = new Map<string, Set<string>>();
  for (const group of SYNONYM_GROUPS) {
    for (const term of group) {
      const set = map.get(term) || new Set<string>();
      for (const other of group) {
        if (other !== term) set.add(other);
      }
      map.set(term, set);
    }
  }
  return map;
}

const SYNONYM_MAP = buildSynonymMap();

/** Expand a set of tokens with their semantic neighbours. */
export function expandTokens(tokens: string[]): Set<string> {
  const expanded = new Set<string>();
  for (const raw of tokens) {
    if (STOPWORDS.has(raw) || raw.length <= 1) continue;
    expanded.add(raw);
    const synonyms = SYNONYM_MAP.get(raw);
    if (synonyms) {
      for (const s of synonyms) expanded.add(s);
    }
  }
  return expanded;
}

// ---------------------------------------------------------------------------
// TF-IDF + cosine similarity
// ---------------------------------------------------------------------------

type TermVector = Map<string, number>;

export function cosineSimilarity(a: TermVector, b: TermVector): number {
  let dot = 0;
  let normA = 0;
  let normB = 0;

  for (const [term, weight] of a) {
    normA += weight * weight;
    const other = b.get(term);
    if (other) dot += weight * other;
  }
  for (const weight of b.values()) {
    normB += weight * weight;
  }

  if (normA === 0 || normB === 0) return 0;
  return dot / (Math.sqrt(normA) * Math.sqrt(normB));
}

function buildDocumentText(t: ITemplate): string {
  return [
    t.name,
    t.description,
    ...(t.tags || []),
    ...(t.functionality || []),
    t.author,
  ]
    .filter(Boolean)
    .join(" ");
}

// ---------------------------------------------------------------------------
// Filtering
// ---------------------------------------------------------------------------

export interface SearchOptions {
  tags?: string[];
  verified?: boolean;
  license?: string;
  minQuality?: number;
  minDownloads?: number;
  author?: string;
  dateFrom?: string | Date;
  dateTo?: string | Date;
}

export function passesFilters(t: ITemplate, options: SearchOptions): boolean {
  if (
    options.tags &&
    options.tags.length > 0 &&
    !options.tags.every((tag) =>
      t.tags.some((tt) => tt.toLowerCase() === tag.toLowerCase()),
    )
  ) {
    return false;
  }

  if (options.verified && !t.verified) return false;

  if (
    options.license &&
    (t.license || "").toLowerCase() !== options.license.toLowerCase()
  ) {
    return false;
  }

  if (
    typeof options.minQuality === "number" &&
    t.ratings.average < options.minQuality
  ) {
    return false;
  }

  if (
    typeof options.minDownloads === "number" &&
    t.downloads < options.minDownloads
  ) {
    return false;
  }

  if (
    options.author &&
    !t.author.toLowerCase().includes(options.author.toLowerCase())
  ) {
    return false;
  }

  if (options.dateFrom) {
    const from = new Date(options.dateFrom).getTime();
    if (!Number.isNaN(from) && new Date(t.createdAt).getTime() < from) {
      return false;
    }
  }

  if (options.dateTo) {
    const to = new Date(options.dateTo).getTime();
    if (!Number.isNaN(to) && new Date(t.createdAt).getTime() > to) {
      return false;
    }
  }

  return true;
}

// ---------------------------------------------------------------------------
// Search engine
// ---------------------------------------------------------------------------

export interface ScoredTemplate {
  template: ITemplate;
  relevanceScore: number;
  matchedTerms: string[];
}

export class SearchEngine {
  private cachedFingerprint = "";
  private cachedVectors: Map<string, TermVector> | null = null;
  private idf: Map<string, number> = new Map();

  private fingerprint(templates: ITemplate[]): string {
    return templates
      .map((t) => `${t.id}:${new Date(t.updatedAt).getTime()}`)
      .sort()
      .join("|");
  }

  /** (Re)build the TF-IDF index only when the template corpus has changed. */
  private ensureIndex(templates: ITemplate[]): void {
    const fp = this.fingerprint(templates);
    if (fp === this.cachedFingerprint && this.cachedVectors) return;

    const docTokens = new Map<string, string[]>();
    const documentFrequency = new Map<string, number>();

    for (const t of templates) {
      const tokens = meaningfulTokens(buildDocumentText(t));
      docTokens.set(t.id, tokens);
      for (const term of new Set(tokens)) {
        documentFrequency.set(term, (documentFrequency.get(term) || 0) + 1);
      }
    }

    const totalDocs = templates.length || 1;
    this.idf = new Map();
    for (const [term, df] of documentFrequency) {
      this.idf.set(term, Math.log((totalDocs + 1) / (df + 1)) + 1);
    }

    this.cachedVectors = new Map();
    for (const t of templates) {
      const tokens = docTokens.get(t.id) || [];
      const termFreq = new Map<string, number>();
      for (const term of tokens) {
        termFreq.set(term, (termFreq.get(term) || 0) + 1);
      }
      const vector: TermVector = new Map();
      const length = tokens.length || 1;
      for (const [term, freq] of termFreq) {
        vector.set(term, (freq / length) * (this.idf.get(term) || 1));
      }
      this.cachedVectors.set(t.id, vector);
    }

    this.cachedFingerprint = fp;
  }

  /**
   * Rank templates against a natural-language query. Filters are applied
   * first, then remaining templates are scored by semantic + lexical
   * relevance. An empty query returns all filtered templates unscored
   * (caller typically falls back to a downloads/date sort in that case).
   */
  search(
    templates: ITemplate[],
    query: string,
    options: SearchOptions = {},
  ): ScoredTemplate[] {
    const filtered = templates.filter((t) => passesFilters(t, options));

    if (!query || !query.trim()) {
      return filtered.map((t) => ({
        template: t,
        relevanceScore: 0,
        matchedTerms: [],
      }));
    }

    this.ensureIndex(templates);

    const queryTokens = meaningfulTokens(query);
    const expandedTerms = expandTokens(queryTokens);

    const queryTermFreq = new Map<string, number>();
    for (const term of expandedTerms) {
      queryTermFreq.set(term, (queryTermFreq.get(term) || 0) + 1);
    }
    const queryVector: TermVector = new Map();
    for (const [term, freq] of queryTermFreq) {
      queryVector.set(term, freq * (this.idf.get(term) || 1));
    }

    const queryLower = query.toLowerCase().trim();
    const results: ScoredTemplate[] = [];

    for (const t of filtered) {
      const docVector = this.cachedVectors?.get(t.id) || new Map();
      const similarity = cosineSimilarity(queryVector, docVector);

      const matchedTerms = [...expandedTerms].filter((term) =>
        docVector.has(term),
      );

      // Exact / substring boosts so precise queries still win outright.
      let boost = 0;
      const nameLower = t.name.toLowerCase();
      if (nameLower === queryLower) boost += 0.5;
      else if (nameLower.includes(queryLower)) boost += 0.25;
      if (t.tags.some((tag) => tag.toLowerCase() === queryLower)) boost += 0.2;

      const relevanceScore = similarity + boost;
      if (relevanceScore > 0) {
        results.push({ template: t, relevanceScore, matchedTerms });
      }
    }

    results.sort((a, b) => b.relevanceScore - a.relevanceScore);
    return results;
  }

  /** Find templates whose document vector is closest to the target's. */
  findSimilar(
    templates: ITemplate[],
    targetId: string,
    limit = 5,
  ): ScoredTemplate[] {
    this.ensureIndex(templates);
    const targetVector = this.cachedVectors?.get(targetId);
    if (!targetVector) return [];

    const results: ScoredTemplate[] = [];
    for (const t of templates) {
      if (t.id === targetId) continue;
      const vector = this.cachedVectors?.get(t.id) || new Map();
      const score = cosineSimilarity(targetVector, vector);
      if (score > 0) {
        results.push({ template: t, relevanceScore: score, matchedTerms: [] });
      }
    }

    results.sort((a, b) => b.relevanceScore - a.relevanceScore);
    return results.slice(0, limit);
  }
}

export const searchEngine = new SearchEngine();
