<script setup lang="ts">
import type { InstallUiStatus } from "~/composables/useModInstall";

const props = withDefaults(
  defineProps<{
    modId: number;
    status: InstallUiStatus;
    canUninstall?: boolean;
    isUninstalling?: boolean;
    error?: string;
    compact?: boolean;
  }>(),
  {
    canUninstall: false,
    isUninstalling: false,
    error: "",
    compact: false,
  },
);

const emit = defineEmits<{
  install: [];
  uninstall: [];
}>();

const label = computed(() => {
  switch (props.status) {
    case "installing":
      return props.isUninstalling ? "Uninstalling…" : "Installing…";
    case "updateAvailable":
      return "Update";
    case "upToDate":
      return "Uninstall";
    case "unavailable":
      return "Unavailable";
    case "installBlocked":
      return "Install blocked";
    default:
      return "Install";
  }
});

const isDisabled = computed(() => {
  if (
    props.status === "installing" ||
    props.status === "unavailable" ||
    props.status === "installBlocked"
  ) {
    return true;
  }
  if (props.status === "upToDate") {
    return !props.canUninstall;
  }
  return false;
});

const buttonClass = computed(() => {
  if (props.status === "upToDate") {
    return props.canUninstall ? "btn-uninstall" : "btn-uninstall-disabled";
  }
  if (props.status === "updateAvailable") return "btn-update";
  if (props.compact) return "btn-compact";
  return "btn-primary";
});

const displayError = computed(() => props.error);

function onClick() {
  if (isDisabled.value) return;
  if (props.status === "upToDate") {
    emit("uninstall");
    return;
  }
  if (props.status === "notInstalled" || props.status === "updateAvailable") {
    emit("install");
  }
}
</script>

<template>
  <div class="mod-install-button" :class="{ compact }">
    <button
      type="button"
      class="install-btn"
      :class="buttonClass"
      :disabled="isDisabled"
      @click.stop="onClick"
    >
      <span v-if="status === 'installing'" class="spinner" aria-hidden="true" />
      {{ label }}
    </button>
    <p v-if="displayError" class="install-error">{{ displayError }}</p>
  </div>
</template>

<style scoped>
.mod-install-button {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.mod-install-button.compact {
  flex-direction: column;
  align-items: stretch;
}

.install-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.45rem;
  border-radius: var(--modio-radius-sm);
  font-size: 0.85rem;
  font-weight: 600;
  padding: 0.5rem 0.85rem;
  box-shadow: none;
}

.compact .install-btn {
  padding: 0.38rem 0.7rem;
  font-size: 0.8rem;
}

.btn-primary {
  background: var(--modio-accent);
  border-color: var(--modio-accent);
  color: #041316;
}

.btn-primary:hover:not(:disabled) {
  background: var(--modio-accent-hover);
  border-color: var(--modio-accent-hover);
}

.btn-update {
  background: rgba(7, 193, 216, 0.16);
  border-color: rgba(7, 193, 216, 0.55);
  color: var(--modio-accent);
}

.btn-update:hover:not(:disabled) {
  background: rgba(7, 193, 216, 0.24);
}

.btn-uninstall {
  background: transparent;
  border: 1px solid var(--modio-border);
  color: var(--modio-text-muted);
}

.btn-uninstall:hover:not(:disabled) {
  background: var(--modio-surface-hover);
  color: var(--modio-text);
  border-color: var(--modio-border);
}

.btn-uninstall-disabled {
  background: var(--modio-surface-raised);
  border: 1px solid var(--modio-border);
  color: var(--modio-text-muted);
  opacity: 0.7;
  cursor: not-allowed;
}

.install-btn:disabled.btn-primary,
.install-btn:disabled.btn-update {
  opacity: 0.7;
  cursor: not-allowed;
}

.install-error {
  margin: 0;
  font-size: 0.78rem;
  color: var(--modio-danger);
}

.spinner {
  width: 0.85rem;
  height: 0.85rem;
  border: 2px solid rgba(4, 19, 22, 0.25);
  border-top-color: currentColor;
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
