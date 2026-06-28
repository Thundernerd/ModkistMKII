import type { ModSort, ModTypeFilter } from "~/composables/useMods";
import { useMods } from "~/composables/useMods";
import { useModTagOptions } from "~/composables/useModTagOptions";

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
    categoryTags,
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

  const hasActiveFilters = computed(
    () =>
      Boolean(search.value.trim()) ||
      modType.value !== "all" ||
      categoryTags.value.length > 0,
  );

  function isCategoryTagSelected(tag: string) {
    return categoryTags.value.includes(tag);
  }

  function toggleCategoryTag(tag: string) {
    if (isCategoryTagSelected(tag)) {
      categoryTags.value = categoryTags.value.filter((value) => value !== tag);
      return;
    }

    categoryTags.value = [...categoryTags.value, tag];
  }

  function clearCategoryTags() {
    categoryTags.value = [];
  }

  function clearFilters() {
    search.value = "";
    modType.value = "all";
    categoryTags.value = [];
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
    categoryTags,
    sort,
    sortDir,
    hasMore,
    loadMore,
    activeCategoryLabel,
    activeCategoryOptions,
    hasActiveFilters,
    isCategoryTagSelected,
    toggleCategoryTag,
    clearCategoryTags,
    clearFilters,
    initialize,
  };
}
