import { invoke } from "@tauri-apps/api/core";
import type { ProviderUser } from "~/types/pr";

export async function getStoredToken(): Promise<string | null> {
  return invoke<string | null>("get_stored_token");
}

export async function validateAndStoreToken(token: string): Promise<ProviderUser> {
  return invoke<ProviderUser>("validate_and_store_token", { token });
}

export async function logout(): Promise<void> {
  return invoke("logout");
}

export async function restoreSession(): Promise<ProviderUser | null> {
  return invoke<ProviderUser | null>("restore_session");
}
