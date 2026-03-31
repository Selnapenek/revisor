import { createSignal } from "solid-js";

export interface RepoEntry {
  owner: string;
  repo: string;
}

const [savedRepos, setSavedRepos] = createSignal<RepoEntry[]>([]);
const [currentRepo, setCurrentRepo] = createSignal<RepoEntry | null>(null);
const [isLoading, setIsLoading] = createSignal(false);

export const reposStore = {
  get savedRepos() { return savedRepos(); },
  get currentRepo() { return currentRepo(); },
  get isLoading() { return isLoading(); },
  setSavedRepos,
  setCurrentRepo,
  setIsLoading,
};
