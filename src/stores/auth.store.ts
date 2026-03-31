import { createSignal } from "solid-js";
import type { ProviderUser } from "~/types/pr";

const [user, setUser] = createSignal<ProviderUser | null>(null);
const [isLoading, setIsLoading] = createSignal(true);

export const authStore = {
  get user() { return user(); },
  get isAuthenticated() { return user() !== null; },
  get isLoading() { return isLoading(); },
  setUser,
  setIsLoading,
};
