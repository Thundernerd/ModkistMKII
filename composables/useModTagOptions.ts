import { invoke } from "~/utils/tauri";

export interface ModTagOptions {
  pluginTypes: string[];
  blueprintTypes: string[];
}

export function useModTagOptions() {
  const tagOptions = ref<ModTagOptions | null>(null);
  const loading = ref(false);
  const error = ref("");

  async function fetchTagOptions() {
    loading.value = true;
    error.value = "";

    try {
      tagOptions.value = await invoke<ModTagOptions>("get_mod_tag_options");
    } catch (err) {
      error.value = String(err);
      tagOptions.value = null;
    } finally {
      loading.value = false;
    }
  }

  return {
    tagOptions,
    loading,
    error,
    fetchTagOptions,
  };
}
