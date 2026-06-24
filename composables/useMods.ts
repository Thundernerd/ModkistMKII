import { invoke } from "~/utils/tauri";

export interface ModSummary {
  id: number;
  name: string;
  summary: string;
  profileUrl: string;
  logoUrl: string;
  downloadsTotal: number;
  subscribersTotal: number;
  popularityRank: number | null;
  tags: string[];
  dateUpdated: string;
}

export interface ModListResult {
  mods: ModSummary[];
  total: number;
}

export type ModSort =
  | "recentlyAdded"
  | "lastUpdated"
  | "trending"
  | "mostPopular"
  | "mostSubscribers"
  | "highestRated"
  | "alphabetical";

export type ModTypeFilter = "all" | "plugin" | "blueprint";

export type SortDir = "asc" | "desc";

export interface ListModsParams {
  search?: string;
  modType?: ModTypeFilter;
  sort?: ModSort;
  sortDir?: SortDir;
  limit?: number;
  offset?: number;
}

const DEFAULT_LIMIT = 20;

export function useMods() {
  const mods = ref<ModSummary[]>([]);
  const total = ref(0);
  const loading = ref(false);
  const error = ref("");

  const search = ref("");
  const modType = ref<ModTypeFilter>("all");
  const sort = ref<ModSort>("trending");
  const sortDir = ref<SortDir>("desc");
  const offset = ref(0);

  let searchDebounce: ReturnType<typeof setTimeout> | undefined;

  async function fetchMods(append = false) {
    loading.value = true;
    if (!append) {
      error.value = "";
    }

    try {
      const result = await invoke<ModListResult>("list_mods", {
        params: {
          search: search.value.trim() || undefined,
          modType: modType.value,
          sort: sort.value,
          sortDir: sortDir.value,
          limit: DEFAULT_LIMIT,
          offset: offset.value,
        },
      });

      if (append) {
        mods.value = [...mods.value, ...result.mods];
      } else {
        mods.value = result.mods;
      }
      total.value = result.total;
    } catch (err) {
      error.value = String(err);
      if (!append) {
        mods.value = [];
        total.value = 0;
      }
    } finally {
      loading.value = false;
    }
  }

  function resetAndFetch() {
    offset.value = 0;
    return fetchMods(false);
  }

  function scheduleSearchFetch() {
    if (searchDebounce) {
      clearTimeout(searchDebounce);
    }
    searchDebounce = setTimeout(() => {
      resetAndFetch();
    }, 300);
  }

  function loadMore() {
    if (loading.value || mods.value.length >= total.value) return;
    offset.value += DEFAULT_LIMIT;
    return fetchMods(true);
  }

  const hasMore = computed(() => mods.value.length < total.value);

  watch(sort, () => resetAndFetch());
  watch(sortDir, () => resetAndFetch());
  watch(modType, () => resetAndFetch());
  watch(search, () => scheduleSearchFetch());

  onUnmounted(() => {
    if (searchDebounce) {
      clearTimeout(searchDebounce);
    }
  });

  return {
    mods,
    total,
    loading,
    error,
    search,
    modType,
    sort,
    sortDir,
    hasMore,
    fetchMods: resetAndFetch,
    loadMore,
  };
}
