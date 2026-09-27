import { invoke } from "@tauri-apps/api/core";
import type { ProviderUser } from "~/types/pr";

export interface DeviceLoginInfo {
  attemptId: string;
  userCode: string;
  verificationUri: string;
  expiresIn: number;
}

export async function startGithubLogin(): Promise<DeviceLoginInfo> {
  return invoke<DeviceLoginInfo>("start_github_login");
}

export async function validateAndStoreToken(token: string): Promise<ProviderUser> {
  return invoke<ProviderUser>("validate_and_store_token", { token });
}

export async function waitForGithubLogin(attemptId: string): Promise<ProviderUser> {
  return invoke<ProviderUser>("wait_for_github_login", { attemptId });
}

export async function cancelGithubLogin(attemptId: string): Promise<void> {
  return invoke("cancel_github_login", { attemptId });
}

export async function logout(): Promise<void> {
  return invoke("logout");
}

export async function restoreSession(): Promise<ProviderUser | null> {
  return invoke<ProviderUser | null>("restore_session");
}
