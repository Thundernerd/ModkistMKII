import { invoke } from "~/utils/tauri";

export interface ModDetail {
  id: number;
  name: string;
  summary: string;
  profileUrl: string;
  logoUrl: string;
  heroImageUrl: string;
  downloadsTotal: number;
  downloadsToday: number;
  subscribersTotal: number;
  popularityRank: number | null;
  tags: string[];
  dateAdded: string;
  dateUpdated: string;
  dateLive: string;
  descriptionHtml?: string;
  submittedByUsername: string;
  submittedByProfileUrl: string;
  submittedByAvatarUrl?: string;
  ratingsDisplayText: string;
  ratingsPercentagePositive: number;
  ratingsPositive: number;
  ratingsNegative: number;
  mediaImageUrls: string[];
  hasDependencies: boolean;
  homepageUrl?: string;
  fileId?: number;
}

export interface ModDependency {
  id: number;
  name: string;
  profileUrl: string;
  logoUrl: string;
  submittedByUsername: string;
  dateUpdated: string;
  downloadsTotal: number;
  fileSizeBytes?: number;
}

export type DependencySort = "mostPopular" | "lastUpdated" | "alphabetical";

export function useModDetail() {
  const mod = ref<ModDetail | null>(null);
  const dependencies = ref<ModDependency[]>([]);
  const loading = ref(false);
  const dependenciesLoading = ref(false);
  const error = ref("");
  const dependenciesError = ref("");

  async function fetchMod(modId: number) {
    loading.value = true;
    error.value = "";
    mod.value = null;
    dependencies.value = [];
    dependenciesError.value = "";

    try {
      mod.value = await invoke<ModDetail>("get_mod", { modId });
    } catch (err) {
      error.value = String(err);
    } finally {
      loading.value = false;
    }
  }

  async function fetchDependencies(modId: number) {
    if (dependenciesLoading.value) return;

    dependenciesLoading.value = true;
    dependenciesError.value = "";

    try {
      const result = await invoke<{ mods: ModDependency[] }>(
        "list_mod_dependencies",
        { modId },
      );
      dependencies.value = result.mods;
    } catch (err) {
      dependenciesError.value = String(err);
    } finally {
      dependenciesLoading.value = false;
    }
  }

  return {
    mod,
    dependencies,
    loading,
    dependenciesLoading,
    error,
    dependenciesError,
    fetchMod,
    fetchDependencies,
  };
}
