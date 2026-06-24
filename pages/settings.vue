<script setup lang="ts">
definePageMeta({ layout: "app" });
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
</style>
