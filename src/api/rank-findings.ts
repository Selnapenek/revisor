import { invoke } from '@tauri-apps/api/core';
import type { RankingResult, RankingSnapshot } from '~/lib/rank-findings';

export const rankAiFindings = (snapshot: RankingSnapshot) =>
  invoke<RankingResult>('rank_ai_findings', { snapshot });
