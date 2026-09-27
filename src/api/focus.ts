import { invoke } from "@tauri-apps/api/core";
import type { FocusResult, FocusSnapshot } from "~/lib/focus-files";

export const focusHasKey = () => invoke<boolean>("focus_has_key");
export const focusSaveKey = (key: string) => invoke<void>("focus_save_key", { key });
export const focusRemoveKey = () => invoke<void>("focus_remove_key");
export const findFocusFiles = (snapshot: FocusSnapshot) =>
  invoke<FocusResult>("find_focus_files", { snapshot });
