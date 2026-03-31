export interface ProviderUser {
  login: string;
  avatar_url: string;
  name: string | null;
}

export interface Label {
  name: string;
  color: string;
}

export interface PullRequest {
  id: number;
  number: number;
  title: string;
  body: string | null;
  state: string;
  draft: boolean;
  author_login: string;
  author_avatar: string;
  labels: Label[];
  created_at: string;
  updated_at: string;
  merged_at: string | null;
  comments_count: number;
  review_comments_count: number;
  additions: number;
  deletions: number;
  changed_files: number;
  head_sha: string;
  base_sha: string;
  head_ref: string;
  base_ref: string;
  html_url: string;
}

export interface PrFile {
  sha: string;
  filename: string;
  status: string;
  additions: number;
  deletions: number;
  patch: string | null;
}

export interface ReviewComment {
  id: number;
  path: string;
  line: number | null;
  original_line: number | null;
  side: string | null;
  diff_hunk: string;
  body: string;
  author_login: string;
  author_avatar: string;
  created_at: string;
  updated_at: string;
  in_reply_to_id: number | null;
  review_id: number | null;
  html_url: string;
}

export interface RateLimitInfo {
  remaining: number;
  limit: number;
  reset_at: number;
}

export interface PaginatedResult<T> {
  items: T[];
  page: number;
  has_next_page: boolean;
  total_count: number | null;
}
