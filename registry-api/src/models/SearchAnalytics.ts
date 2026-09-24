import { v4 as uuid } from "uuid";
import { ITemplate } from "./Template";

/**
 * In-memory usage analytics for the marketplace search experience.
 *
 * Tracks two kinds of events:
 *  - Search events (what people searched for, with which filters, and how
 *    many results came back) — powers search suggestions + "top queries".
 *  - Interaction events (view / download / click on a template) — powers
 *    trending templates and per-user personalization.
 *
 * Swap this for a persisted store (e.g. Mongo/Postgres) in production; the
 * public API is intentionally storage-agnostic.
 */

export interface ISearchEvent {
  id: string;
  userId?: string;
  query: string;
  filters?: Record<string, unknown>;
  resultCount: number;
  createdAt: Date;
}

export type InteractionType = "view" | "download" | "click";

export interface IInteractionEvent {
  id: string;
  userId?: string;
  templateId: string;
  type: InteractionType;
  createdAt: Date;
}

const INTERACTION_WEIGHT: Record<InteractionType, number> = {
  view: 1,
  click: 2,
  download: 3,
};

export class SearchAnalyticsStore {
  private searchEvents: ISearchEvent[] = [];
  private interactionEvents: IInteractionEvent[] = [];

  recordSearch(
    userId: string | undefined,
    query: string,
    filters: Record<string, unknown> | undefined,
    resultCount: number,
  ): ISearchEvent {
    const event: ISearchEvent = {
      id: uuid(),
      userId,
      query: (query || "").trim(),
      filters,
      resultCount,
      createdAt: new Date(),
    };
    this.searchEvents.push(event);
    return event;
  }

  recordInteraction(
    userId: string | undefined,
    templateId: string,
    type: InteractionType,
  ): IInteractionEvent {
    const event: IInteractionEvent = {
      id: uuid(),
      userId,
      templateId,
      type,
      createdAt: new Date(),
    };
    this.interactionEvents.push(event);
    return event;
  }

  /** Most frequently issued (non-empty) search queries. */
  getTopQueries(limit = 10): { query: string; count: number }[] {
    const counts = new Map<string, number>();
    for (const e of this.searchEvents) {
      if (!e.query) continue;
      const key = e.query.toLowerCase();
      counts.set(key, (counts.get(key) || 0) + 1);
    }
    return [...counts.entries()]
      .sort((a, b) => b[1] - a[1])
      .slice(0, limit)
      .map(([query, count]) => ({ query, count }));
  }

  /** Autocomplete: past queries starting with `prefix`, most popular first. */
  getSuggestions(prefix: string, limit = 10): string[] {
    const p = prefix.toLowerCase().trim();
    if (!p) return this.getTopQueries(limit).map((q) => q.query);

    const counts = new Map<string, number>();
    for (const e of this.searchEvents) {
      const q = e.query.toLowerCase();
      if (q && q.startsWith(p)) {
        counts.set(q, (counts.get(q) || 0) + 1);
      }
    }
    return [...counts.entries()]
      .sort((a, b) => b[1] - a[1])
      .slice(0, limit)
      .map(([q]) => q);
  }

  /** Templates with the most weighted interactions in the recent window. */
  getTrendingTemplateIds(
    windowMs: number = 7 * 24 * 60 * 60 * 1000,
    limit = 10,
  ): { templateId: string; score: number }[] {
    const cutoff = Date.now() - windowMs;
    const scores = new Map<string, number>();

    for (const e of this.interactionEvents) {
      if (new Date(e.createdAt).getTime() < cutoff) continue;
      const weight = INTERACTION_WEIGHT[e.type];
      scores.set(e.templateId, (scores.get(e.templateId) || 0) + weight);
    }

    return [...scores.entries()]
      .sort((a, b) => b[1] - a[1])
      .slice(0, limit)
      .map(([templateId, score]) => ({ templateId, score }));
  }

  /**
   * Personalization signal for a user: which tags and authors they tend to
   * engage with, weighted by interaction type (downloads count more than
   * views). `templateLookup` resolves a template by id so this store never
   * needs a direct dependency on TemplateStore.
   */
  getUserAffinity(
    userId: string,
    templateLookup: (id: string) => ITemplate | undefined,
  ): { tagScores: Map<string, number>; authorScores: Map<string, number> } {
    const tagScores = new Map<string, number>();
    const authorScores = new Map<string, number>();

    for (const e of this.interactionEvents) {
      if (e.userId !== userId) continue;
      const tpl = templateLookup(e.templateId);
      if (!tpl) continue;

      const weight = INTERACTION_WEIGHT[e.type];
      for (const tag of tpl.tags) {
        const key = tag.toLowerCase();
        tagScores.set(key, (tagScores.get(key) || 0) + weight);
      }
      authorScores.set(tpl.author, (authorScores.get(tpl.author) || 0) + weight);
    }

    return { tagScores, authorScores };
  }

  getEventCounts(): { searches: number; interactions: number } {
    return {
      searches: this.searchEvents.length,
      interactions: this.interactionEvents.length,
    };
  }
}

export const searchAnalytics = new SearchAnalyticsStore();
