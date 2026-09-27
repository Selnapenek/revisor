import type { PrSearchFilters } from "~/types/pr";

export function withAuthorPreset(filters: PrSearchFilters, login: string): PrSearchFilters {
  return { ...filters, authors: [login] };
}

export function withReviewRequestedPreset(filters: PrSearchFilters): PrSearchFilters {
  return { ...filters, review_status: "review-requested:@me" };
}
