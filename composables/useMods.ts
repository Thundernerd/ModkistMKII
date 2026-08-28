import { invoke } from "~/utils/tauri";
import { useModioRateLimit } from "~/composables/useModioRateLimit";
import { useNotifications } from "~/composables/useNotifications";
import { isRateLimitedMessage, rateLimitUserMessage } from "~/utils/modioError";

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
  categoryTagsIn?: string[];
  categoryTagsNotIn?: string[];
  sort?: ModSort;
  sortDir?: SortDir;
  limit?: number;
  offset?: number;
}

const DEFAULT_LIMIT = 20;

export function useMods() {
  const { pushNotification } = useNotifications();
  const { showRateLimitBanner } = useModioRateLimit();
  const mods = ref<ModSummary[]>([]);
  const total = ref(0);
  const loading = ref(false);

  const search = ref("");
  const modType = ref<ModTypeFilter>("all");
  const categoryTagsIn = ref<string[]>([]);
  const categoryTagsNotIn = ref<string[]>([]);
  const sort = ref<ModSort>("trending");
  const sortDir = ref<SortDir>("desc");
  const offset = ref(0);

  let searchDebounce: ReturnType<typeof setTimeout> | undefined;
  let fetchGeneration = 0;

  async function fetchMods(append = false) {
    const generation = ++fetchGeneration;
    loading.value = true;

    try {
      const result = await invoke<ModListResult>("list_mods", {
        params: {
          search: search.value.trim() || undefined,
          modType: modType.value,
          categoryTagsIn:
            categoryTagsIn.value.length > 0
              ? [...categoryTagsIn.value]
              : undefined,
          categoryTagsNotIn:
            categoryTagsNotIn.value.length > 0
              ? [...categoryTagsNotIn.value]
              : undefined,
          sort: sort.value,
          sortDir: sortDir.value,
          limit: DEFAULT_LIMIT,
          offset: offset.value,
        },
      });

      if (generation !== fetchGeneration) {
        return;
      }

      if (append) {
        const seen = new Set(mods.value.map((mod) => mod.id));
        mods.value = [
          ...mods.value,
          ...result.mods.filter((mod) => !seen.has(mod.id)),
        ];
      } else {
        mods.value = result.mods;
      }
      total.value = result.total;
    } catch (err) {
      if (generation !== fetchGeneration) {
        return;
      }
      const message = String(err);
      const rateLimited = isRateLimitedMessage(message);
      if (rateLimited) {
        showRateLimitBanner();
      }
      pushNotification({
        title: append ? "Did not load more mods" : "Did not load mods",
        message: rateLimited ? rateLimitUserMessage() : message,
        tone: rateLimited ? "warning" : "error",
        durationMs: 10_000,
      });
      if (!append) {
        mods.value = [];
        total.value = 0;
      }
    } finally {
      if (generation === fetchGeneration) {
        loading.value = false;
      }
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
    if (loading.value || offset.value + DEFAULT_LIMIT >= total.value) return;
    offset.value += DEFAULT_LIMIT;
    return fetchMods(true);
  }

  const hasMore = computed(() => offset.value + DEFAULT_LIMIT < total.value);

  watch(sort, () => resetAndFetch());
  watch(sortDir, () => resetAndFetch());
  watch(modType, () => {
    const hadCategoryFilters =
      categoryTagsIn.value.length > 0 || categoryTagsNotIn.value.length > 0;
    if (hadCategoryFilters) {
      categoryTagsIn.value = [];
      categoryTagsNotIn.value = [];
      return;
    }

    resetAndFetch();
  });
  watch([categoryTagsIn, categoryTagsNotIn], () => resetAndFetch(), {
    deep: true,
  });
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
    search,
    modType,
    categoryTagsIn,
    categoryTagsNotIn,
    sort,
    sortDir,
    hasMore,
    fetchMods: resetAndFetch,
    loadMore,
  };
}
