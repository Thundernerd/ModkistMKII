<script setup lang="ts">
import type { ProfileSummary } from "~/composables/useProfiles";

const props = defineProps<{
  open: boolean;
  profiles: ProfileSummary[];
  title?: string;
  description?: string;
}>();

const emit = defineEmits<{
  close: [];
  select: [profileId: string];
}>();

const selectedId = ref("");

watch(
  () => props.open,
  (isOpen) => {
    if (!isOpen) {
      selectedId.value = "";
      return;
    }

    const vanilla = props.profiles.find((profile) => profile.kind === "vanilla");
    selectedId.value = vanilla?.id ?? props.profiles[0]?.id ?? "";
  },
);

function confirmSelection() {
  if (!selectedId.value) return;
  emit("select", selectedId.value);
}
</script>

<template>
  <div v-if="open" class="profile-picker-backdrop" @click.self="emit('close')">
    <div
      class="profile-picker-dialog panel"
      role="dialog"
      aria-modal="true"
      :aria-label="title ?? 'Choose a profile'"
    >
      <h2 class="profile-picker-title">{{ title ?? "Choose a profile" }}</h2>
      <p v-if="description" class="hint profile-picker-desc">
        {{ description }}
      </p>

      <ul class="profile-picker-list">
        <li v-for="profile in profiles" :key="profile.id">
          <label class="profile-picker-option">
            <input
              v-model="selectedId"
              type="radio"
              name="logout-profile"
              :value="profile.id"
              :disabled="!profile.selectable"
            />
            <span class="profile-picker-option-body">
              <span class="profile-picker-option-name">{{ profile.name }}</span>
              <span v-if="profile.kind === 'vanilla'" class="profile-picker-tag">
                No mods
              </span>
              <span
                v-else-if="profile.kind === 'custom'"
                class="profile-picker-tag"
              >
                Custom
              </span>
            </span>
          </label>
        </li>
      </ul>

      <div class="profile-picker-actions">
        <button type="button" class="btn-secondary" @click="emit('close')">
          Cancel
        </button>
        <button
          type="button"
          class="btn-primary"
          :disabled="!selectedId"
          @click="confirmSelection"
        >
          Continue
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.profile-picker-backdrop {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1rem;
  background: rgba(0, 0, 0, 0.55);
}

.profile-picker-dialog {
  width: min(100%, 24rem);
  padding: 1.25rem;
  border-radius: var(--modio-radius);
  background: var(--modio-surface);
  border: 1px solid var(--modio-border);
  box-shadow: var(--modio-shadow);
}

.profile-picker-title {
  margin: 0 0 0.35rem;
  font-size: 1.1rem;
  font-weight: 600;
}

.profile-picker-desc {
  margin: 0 0 1rem;
}

.profile-picker-list {
  list-style: none;
  margin: 0 0 1rem;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.profile-picker-option {
  display: flex;
  align-items: flex-start;
  gap: 0.65rem;
  padding: 0.65rem 0.75rem;
  border: 1px solid var(--modio-border);
  border-radius: var(--modio-radius-sm);
  cursor: pointer;
}

.profile-picker-option:has(input:checked) {
  border-color: var(--modio-accent);
  background: rgba(var(--modio-accent-rgb), 0.08);
}

.profile-picker-option-body {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.profile-picker-option-name {
  font-weight: 600;
}

.profile-picker-tag {
  font-size: 0.78rem;
  color: var(--modio-text-muted);
}

.profile-picker-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.65rem;
}
</style>
