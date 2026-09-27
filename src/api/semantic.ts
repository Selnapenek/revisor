import { invoke } from "@tauri-apps/api/core";
import type { SemanticResult, SemanticSnapshot } from "~/lib/semantic-search";

export const searchSemanticDiff = (snapshot: SemanticSnapshot, query: string) =>
  invoke<SemanticResult>("search_semantic_diff", { snapshot, query });
