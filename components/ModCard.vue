<script setup lang="ts">
import type { ModSummary } from "~/composables/useMods";

defineProps<{
  mod: ModSummary;
}>();

function formatDate(iso: string) {
  if (!iso) return "";
  return new Date(iso).toLocaleDateString();
}

function formatCount(value: number) {
  return value.toLocaleString();
}
</script>

<template>
  <NuxtLink :to="`/mods/${mod.id}`" class="mod-card">
    <div class="mod-thumb">
      <img
        v-if="mod.logoUrl"
        :src="mod.logoUrl"
        :alt="`${mod.name} thumbnail`"
        loading="lazy"
      />
      <div v-else class="mod-thumb-fallback" />
    </div>
    <div class="mod-content">
      <h2 class="mod-name">{{ mod.name }}</h2>
      <p class="mod-summary">{{ mod.summary }}</p>
      <div class="mod-stats">
        <span class="stat">
          <span class="stat-icon" aria-hidden="true">↓</span>
          {{ formatCount(mod.downloadsTotal) }}
        </span>
        <span class="stat">
          <span class="stat-icon" aria-hidden="true">★</span>
          {{ formatCount(mod.subscribersTotal) }}
        </span>
        <span v-if="mod.popularityRank" class="stat">
          #{{ mod.popularityRank }}
        </span>
      </div>
      <div v-if="mod.tags.length" class="mod-tags">
        <span v-for="tag in mod.tags" :key="tag" class="tag">{{ tag }}</span>
      </div>
      <p v-if="mod.dateUpdated" class="mod-updated">
        Updated {{ formatDate(mod.dateUpdated) }}
      </p>
    </div>
  </NuxtLink>
</template>

<style scoped>
.mod-card {
  display: flex;
  flex-direction: column;
  height: 100%;
  color: inherit;
  text-decoration: none;
  background: var(--modio-surface);
  border: 1px solid var(--modio-border);
  border-radius: var(--modio-radius);
  overflow: hidden;
  transition:
    transform 0.2s ease,
    border-color 0.2s ease,
    box-shadow 0.2s ease;
}

.mod-card:hover {
  transform: translateY(-2px);
  border-color: rgba(7, 193, 216, 0.45);
  box-shadow: var(--modio-shadow);
  color: inherit;
}

.mod-thumb {
  aspect-ratio: 16 / 9;
  background: var(--modio-surface-raised);
  overflow: hidden;
}

.mod-thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.mod-thumb-fallback {
  width: 100%;
  height: 100%;
  background: linear-gradient(
    135deg,
    var(--modio-surface-raised),
    var(--modio-surface-hover)
  );
}

.mod-content {
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
  padding: 0.85rem 0.9rem 1rem;
  flex: 1;
}

.mod-name {
  margin: 0;
  font-size: 0.98rem;
  font-weight: 600;
  line-height: 1.3;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.mod-summary {
  margin: 0;
  font-size: 0.82rem;
  line-height: 1.45;
  color: var(--modio-text-subtle);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  flex: 1;
}

.mod-stats {
  display: flex;
  flex-wrap: wrap;
  gap: 0.55rem;
  margin-top: 0.15rem;
}

.stat {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
  padding: 0.15rem 0.45rem;
  border-radius: 999px;
  background: var(--modio-surface-raised);
  color: var(--modio-text-muted);
  font-size: 0.75rem;
  font-weight: 500;
}

.stat-icon {
  color: var(--modio-accent);
  font-size: 0.7rem;
}

.mod-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
}

.tag {
  padding: 0.12rem 0.5rem;
  border-radius: 999px;
  border: 1px solid var(--modio-border);
  background: transparent;
  color: var(--modio-text-muted);
  font-size: 0.72rem;
}

.mod-updated {
  margin: 0;
  font-size: 0.72rem;
  color: var(--modio-text-subtle);
}
</style>
