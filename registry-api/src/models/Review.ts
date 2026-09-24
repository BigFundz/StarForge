export interface IReview {
  id: string;
  templateId: string;
  userId: string;
  rating: number; // 1-5
  comment?: string;
  createdAt: Date;
  updatedAt: Date;
}

export class ReviewStore {
  private reviews: Map<string, IReview> = new Map();

  async create(review: IReview): Promise<IReview> {
    this.reviews.set(review.id, review);
    return review;
  }

  async findByTemplateId(templateId: string): Promise<IReview[]> {
    const results: IReview[] = [];
    for (const review of this.reviews.values()) {
      if (review.templateId === templateId) {
        results.push(review);
      }
    }
    return results.sort(
      (a, b) =>
        new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime(),
    );
  }

  async findByUserAndTemplate(
    userId: string,
    templateId: string,
  ): Promise<IReview | null> {
    for (const review of this.reviews.values()) {
      if (review.userId === userId && review.templateId === templateId) {
        return review;
      }
    }
    return null;
  }

  async update(id: string, updates: Partial<IReview>): Promise<IReview | null> {
    const review = this.reviews.get(id);
    if (!review) return null;
    const updated = { ...review, ...updates };
    this.reviews.set(id, updated);
    return updated;
  }

  async delete(id: string): Promise<boolean> {
    return this.reviews.delete(id);
  }
}
