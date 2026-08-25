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
  /** Current user's rating: `1` positive, `-1` negative, `0` none. */
  userRating: number;
  mediaImageUrls: string[];
  hasDependencies: boolean;
  homepageUrl?: string;
  fileId?: number;
  /** True when mod.io admin status is archived (3). */
  isArchived: boolean;
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
  unavailable: boolean;
  unavailableReason?: string;
}

export interface ModFileEntry {
  id: number;
  version: string;
  filename: string;
  filesize: number;
  dateAdded: string;
  changelog: string;
  downloadable: boolean;
}

export interface ModFileListResult {
  files: ModFileEntry[];
  latestFileId: number | null;
}

export type DependencySort = "mostPopular" | "lastUpdated" | "alphabetical";

function applyOptimisticRating(current: ModDetail, nextRating: number): ModDetail {
  let positive = current.ratingsPositive;
  let negative = current.ratingsNegative;
  const previous = current.userRating;

  if (previous === 1) positive = Math.max(0, positive - 1);
  else if (previous === -1) negative = Math.max(0, negative - 1);

  if (nextRating === 1) positive += 1;
  else if (nextRating === -1) negative += 1;

  const total = positive + negative;
  const percentage = total > 0 ? Math.round((positive / total) * 100) : 0;

  return {
    ...current,
    userRating: nextRating,
    ratingsPositive: positive,
    ratingsNegative: negative,
    ratingsPercentagePositive: percentage,
  };
}

export function useModDetail() {
  const mod = ref<ModDetail | null>(null);
  const dependencies = ref<ModDependency[]>([]);
  const loading = ref(false);
  const dependenciesLoading = ref(false);
  const error = ref("");
  const dependenciesError = ref("");
  const ratingError = ref("");
  const ratingBusy = ref(false);

  async function fetchMod(modId: number) {
    loading.value = true;
    error.value = "";
    mod.value = null;
    dependencies.value = [];
    dependenciesError.value = "";
    ratingError.value = "";

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

  async function fetchModFiles(modId: number) {
    return invoke<ModFileListResult>("list_mod_files", { modId });
  }

  async function rateMod(modId: number, rating: number) {
    if (!mod.value || ratingBusy.value) return;
    if (![-1, 0, 1].includes(rating)) return;

    const previous = { ...mod.value };
    ratingBusy.value = true;
    ratingError.value = "";
    mod.value = applyOptimisticRating(mod.value, rating);

    try {
      await invoke("rate_mod", { modId, rating });
    } catch (err) {
      mod.value = previous;
      ratingError.value = String(err);
      throw err;
    } finally {
      ratingBusy.value = false;
    }
  }

  return {
    mod,
    dependencies,
    loading,
    dependenciesLoading,
    error,
    dependenciesError,
    ratingError,
    ratingBusy,
    fetchMod,
    fetchDependencies,
    fetchModFiles,
    rateMod,
  };
}
