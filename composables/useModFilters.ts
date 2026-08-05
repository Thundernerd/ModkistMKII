import type { ModSort, ModTypeFilter } from "~/composables/useMods";
import { useMods } from "~/composables/useMods";
import { useModTagOptions } from "~/composables/useModTagOptions";

export type CategoryTagState = "off" | "in" | "not-in";

export const MOD_TYPE_OPTIONS: { value: ModTypeFilter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "plugin", label: "Plugin" },
  { value: "blueprint", label: "Blueprint" },
];

export const SORT_OPTIONS: { value: ModSort; label: string }[] = [
  { value: "recentlyAdded", label: "Recently added" },
  { value: "lastUpdated", label: "Last updated" },
  { value: "trending", label: "Trending" },
  { value: "mostPopular", label: "Most popular" },
  { value: "mostSubscribers", label: "Most subscribers" },
  { value: "highestRated", label: "Highest rated" },
  { value: "alphabetical", label: "Alphabetical" },
];

export function useModFilters() {
  const {
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
    fetchMods,
    loadMore,
  } = useMods();

  const { tagOptions, fetchTagOptions } = useModTagOptions();

  const activeCategoryLabel = computed(() => {
    if (modType.value === "plugin") return "Plugin type";
    if (modType.value === "blueprint") return "Blueprint type";
    return "";
  });

  const activeCategoryOptions = computed(() => {
    if (!tagOptions.value) return [];
    if (modType.value === "plugin") return tagOptions.value.pluginTypes;
    if (modType.value === "blueprint") return tagOptions.value.blueprintTypes;
    return [];
  });

  const hasCategoryTagFilters = computed(
    () =>
      categoryTagsIn.value.length > 0 || categoryTagsNotIn.value.length > 0,
  );

  const hasActiveFilters = computed(
    () =>
      Boolean(search.value.trim()) ||
      modType.value !== "all" ||
      hasCategoryTagFilters.value,
  );

  function categoryTagState(tag: string): CategoryTagState {
    if (categoryTagsIn.value.includes(tag)) return "in";
    if (categoryTagsNotIn.value.includes(tag)) return "not-in";
    return "off";
  }

  function toggleCategoryTag(tag: string) {
    const state = categoryTagState(tag);

    if (state === "off") {
      categoryTagsIn.value = [...categoryTagsIn.value, tag];
      return;
    }

    if (state === "in") {
      categoryTagsIn.value = categoryTagsIn.value.filter(
        (value) => value !== tag,
      );
      categoryTagsNotIn.value = [...categoryTagsNotIn.value, tag];
      return;
    }

    categoryTagsNotIn.value = categoryTagsNotIn.value.filter(
      (value) => value !== tag,
    );
  }

  function clearCategoryTags() {
    categoryTagsIn.value = [];
    categoryTagsNotIn.value = [];
  }

  function clearFilters() {
    search.value = "";
    modType.value = "all";
    categoryTagsIn.value = [];
    categoryTagsNotIn.value = [];
  }

  async function initialize() {
    await Promise.all([fetchTagOptions(), fetchMods()]);
  }

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
    loadMore,
    activeCategoryLabel,
    activeCategoryOptions,
    hasActiveFilters,
    hasCategoryTagFilters,
    categoryTagState,
    toggleCategoryTag,
    clearCategoryTags,
    clearFilters,
    initialize,
  };
}
