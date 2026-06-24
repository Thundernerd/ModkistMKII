<script setup lang="ts">
import { confirm } from "@tauri-apps/plugin-dialog";

definePageMeta({ layout: "app" });

const REQUIRED_VERSION = "5.4.20";

const {
  bepinexStatus,
  loading,
  installing,
  error,
  refreshBepInExStatus,
  reinstallBepInEx,
} = useBepInEx();

const verifyMessage = ref("");
const verifyTone = ref<"info" | "success" | "error">("info");

const statusLabel = computed(() => {
  switch (bepinexStatus.value.state) {
    case "installed":
      return bepinexStatus.value.foundVersion
        ? `Installed (v${bepinexStatus.value.foundVersion})`
        : `Installed (v${REQUIRED_VERSION})`;
    case "wrongVersion":
      return bepinexStatus.value.foundVersion
        ? `Wrong version (v${bepinexStatus.value.foundVersion})`
        : "Wrong version";
    case "missing":
    default:
      return "Not installed";
  }
});

const statusClass = computed(() => {
  switch (bepinexStatus.value.state) {
    case "installed":
      return "status-ok";
    case "wrongVersion":
      return "status-warn";
    default:
      return "status-bad";
  }
});

function setVerifyResult(tone: "info" | "success" | "error", message: string) {
  verifyTone.value = tone;
  verifyMessage.value = message;
}

async function verifyBepInEx() {
  verifyMessage.value = "";
  error.value = "";

  try {
    await refreshBepInExStatus();
    const status = bepinexStatus.value;

    if (status.state === "installed") {
      setVerifyResult(
        "success",
        status.foundVersion
          ? `BepInEx ${status.foundVersion} is installed correctly.`
          : `BepInEx ${REQUIRED_VERSION} (x64) is installed correctly.`,
      );
      return;
    }

    if (status.state === "wrongVersion") {
      setVerifyResult(
        "error",
        status.message ||
          `Expected BepInEx ${REQUIRED_VERSION} (x64), but a different version was found.`,
      );
      return;
    }

    setVerifyResult(
      "error",
      status.message || "BepInEx is not installed in your game directory.",
    );
  } catch (err) {
    setVerifyResult("error", String(err));
  }
}

async function reinstallBepInExInstall() {
  const confirmed = await confirm(
    "This will remove the existing BepInEx installation from your game folder, including any mods in BepInEx/plugins. A fresh BepInEx 5.4.20 (x64) will then be installed.",
    { title: "Reinstall BepInEx?", kind: "warning" },
  );

  if (!confirmed) {
    return;
  }

  verifyMessage.value = "";
  error.value = "";

  try {
    await reinstallBepInEx();
    setVerifyResult(
      "success",
      bepinexStatus.value.foundVersion
        ? `Reinstalled BepInEx ${bepinexStatus.value.foundVersion}.`
        : `Reinstalled BepInEx ${REQUIRED_VERSION} (x64).`,
    );
  } catch (err) {
    setVerifyResult("error", String(err));
  }
}

onMounted(() => {
  refreshBepInExStatus();
});
</script>

<template>
  <div class="page">
    <header class="page-header">
      <h1>Settings</h1>
      <p class="hint">Manage your Modkist preferences.</p>
    </header>

    <section class="panel">
      <h2 class="panel-title">Game directory</h2>
      <p class="hint panel-desc">
        Folder that contains <code>zeepkist.exe</code>.
      </p>
      <GamePathForm input-id="settings-game-path" />
    </section>

    <section class="panel">
      <h2 class="panel-title">BepInEx</h2>
      <p class="hint panel-desc">
        Modkist requires BepInEx {{ REQUIRED_VERSION }} (x64) in your game
        folder.
      </p>

      <p class="status-line">
        Status:
        <span :class="['status-badge', statusClass]">{{ statusLabel }}</span>
      </p>

      <div class="action-row">
        <button
          type="button"
          class="btn-secondary"
          :disabled="loading || installing"
          @click="verifyBepInEx"
        >
          {{ loading ? "Verifying…" : "Verify installation" }}
        </button>
        <button
          type="button"
          class="btn-danger"
          :disabled="loading || installing"
          @click="reinstallBepInExInstall"
        >
          {{ installing ? "Reinstalling…" : "Reinstall" }}
        </button>
      </div>

      <p
        v-if="verifyMessage"
        :class="verifyTone === 'success' ? 'info' : 'error'"
        class="feedback"
      >
        {{ verifyMessage }}
      </p>
      <p v-else-if="error" class="error feedback">{{ error }}</p>
    </section>
  </div>
</template>

<style scoped>
.page {
  max-width: 40rem;
}

.page-header {
  margin-bottom: 1.5rem;
}

.page-header h1 {
  margin: 0 0 0.35rem;
  font-size: 1.5rem;
  font-weight: 700;
  letter-spacing: -0.02em;
}

.page-header p {
  margin: 0;
}

.panel {
  padding: 1.5rem;
  border-radius: var(--modio-radius);
  background: var(--modio-surface);
  border: 1px solid var(--modio-border);
  box-shadow: var(--modio-shadow);
}

.panel + .panel {
  margin-top: 1rem;
}

.panel-title {
  margin: 0 0 0.35rem;
  font-size: 1rem;
  font-weight: 600;
}

.panel-desc {
  margin: 0 0 1rem;
}

.panel-desc code {
  color: var(--modio-text);
  font-size: 0.85em;
}

.status-line {
  margin: 0 0 1rem;
  font-size: 0.9rem;
  color: var(--modio-text-muted);
}

.status-badge {
  display: inline-block;
  margin-left: 0.35rem;
  padding: 0.15rem 0.5rem;
  border-radius: 999px;
  font-size: 0.8rem;
  font-weight: 600;
}

.status-ok {
  background: rgba(74, 222, 128, 0.12);
  color: var(--modio-success);
}

.status-warn {
  background: rgba(251, 191, 36, 0.12);
  color: #fbbf24;
}

.status-bad {
  background: rgba(248, 113, 113, 0.12);
  color: var(--modio-danger);
}

.action-row {
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem;
}

.feedback {
  margin: 0.85rem 0 0;
}
</style>
