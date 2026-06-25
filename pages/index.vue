<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "~/utils/tauri";
import type { AuthUser } from "~/composables/useModioAuth";
import { navigateToApp, readRedirectParam } from "~/utils/authNavigation";

type LoginStep = "enterEmail" | "codeSent";

const step = ref<LoginStep>("enterEmail");
const email = ref("");
const otp = ref("");
const loading = ref(false);
const error = ref("");
const infoMessage = ref("");
const {
  modioConfigured,
  modioMessage,
  modioStatusChecked,
  refreshModioStatus,
} = useModioStatus();

const emailPattern = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

const { authStatus, refreshAuthStatus } = useModioAuth();
const { resetSessionSync } = useModInstall();
const { refreshProfiles } = useProfiles();

const route = useRoute();
const redirect = computed(() => readRedirectParam(route.query.redirect));

function validateEmail(): boolean {
  if (!emailPattern.test(email.value.trim())) {
    error.value = "Enter a valid email address.";
    return false;
  }
  return true;
}

async function sendCode() {
  if (!validateEmail()) return;

  loading.value = true;
  error.value = "";
  infoMessage.value = "";

  try {
    const message = await invoke<string>("request_email_code", {
      email: email.value.trim(),
    });
    infoMessage.value = message;
    step.value = "codeSent";
  } catch (err) {
    error.value = String(err);
  } finally {
    loading.value = false;
  }
}

function changeEmail() {
  step.value = "enterEmail";
  otp.value = "";
  error.value = "";
  infoMessage.value = "";
}

async function resendCode() {
  await sendCode();
}

function skipLogin() {
  navigateToApp();
}

async function verifyCode() {
  if (!otp.value.trim()) {
    error.value = "Enter the security code from your email.";
    return;
  }

  loading.value = true;
  error.value = "";

  try {
    await invoke<AuthUser>("verify_email_code", { code: otp.value.trim() });
    resetSessionSync();
    await refreshAuthStatus();
    await refreshProfiles();
    const { activeProfileId, switchProfile } = useProfiles();
    const { resetSessionSync } = useModInstall();
    if (activeProfileId.value === "vanilla") {
      await switchProfile("user");
      resetSessionSync();
    }
    await navigateToApp(redirect.value);
  } catch (err) {
    error.value = String(err);
  } finally {
    loading.value = false;
  }
}

onMounted(async () => {
  await refreshAuthStatus();
  if (authStatus.value.loggedIn) {
    await navigateToApp(redirect.value);
    return;
  }
  await refreshModioStatus();
});
</script>

<template>
  <main class="login-shell">
    <div class="login">
      <div class="login-brand">
        <span class="login-brand-mark" aria-hidden="true" />
        <h1>Sign in with mod.io</h1>
      </div>

      <p v-if="modioStatusChecked && !modioConfigured" class="hint">
        {{
          modioMessage ||
            "Set MODIO_API_KEY and MODIO_GAME_ID in .env (see .env.example)."
        }}
      </p>

      <section v-else-if="modioConfigured" class="panel">
      <form
        v-if="step === 'enterEmail'"
        class="form"
        @submit.prevent="sendCode"
      >
        <label for="email">Email</label>
        <input
          id="email"
          v-model="email"
          type="email"
          autocomplete="email"
          placeholder="you@example.com"
          :disabled="loading"
        />
        <button type="submit" :disabled="loading">
          {{ loading ? "Sending…" : "Send code" }}
        </button>
      </form>

      <div v-else class="form">
        <div class="email-row">
          <label>Email</label>
          <div class="email-display">
            <input :value="email" type="email" disabled />
            <button type="button" class="link" @click="changeEmail">
              Change email
            </button>
          </div>
        </div>

        <label for="otp">Security code</label>
        <input
          id="otp"
          v-model="otp"
          type="text"
          inputmode="numeric"
          autocomplete="one-time-code"
          placeholder="Enter code from email"
          class="otp-input"
          :disabled="loading"
          @keyup.enter="verifyCode"
        />

        <div class="actions">
          <button type="button" :disabled="loading" @click="verifyCode">
            {{ loading ? "Verifying…" : "Verify" }}
          </button>
          <button
            type="button"
            class="secondary btn-secondary"
            :disabled="loading"
            @click="resendCode"
          >
            Resend code
          </button>
        </div>
      </div>

      <p v-if="infoMessage" class="info">{{ infoMessage }}</p>
      <p v-if="error" class="error">{{ error }}</p>
    </section>

      <p class="skip-row">
        <button type="button" class="link" @click="skipLogin">
          Continue without signing in
        </button>
      </p>
    </div>
  </main>
</template>

<style scoped>
.login-shell {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 2rem 1.5rem;
  background:
    radial-gradient(
      ellipse 70% 45% at 50% -10%,
      rgba(7, 193, 216, 0.14),
      transparent
    ),
    var(--modio-bg);
}

.login {
  width: 100%;
  max-width: 28rem;
}

.login-brand {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
  margin-bottom: 1.5rem;
}

.login-brand-mark {
  width: 0.35rem;
  height: 1.75rem;
  border-radius: 999px;
  background: var(--modio-accent);
}

.login-brand h1 {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 700;
  letter-spacing: -0.02em;
}

.panel {
  padding: 1.5rem;
  border-radius: var(--modio-radius);
  background: var(--modio-surface);
  border: 1px solid var(--modio-border);
  box-shadow: var(--modio-shadow);
}

.form {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

label {
  font-size: 0.8rem;
  font-weight: 600;
  color: var(--modio-text-muted);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

input {
  width: 100%;
}

input:disabled {
  opacity: 0.65;
}

.otp-input {
  font-size: 1.2rem;
  letter-spacing: 0.15em;
  text-align: center;
}

.email-display {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.actions {
  display: flex;
  gap: 0.75rem;
  margin-top: 0.5rem;
}

.actions button {
  flex: 1;
}

button.link {
  padding: 0;
  border: none;
  background: none;
  color: var(--modio-accent);
  box-shadow: none;
  font-size: 0.9rem;
  text-align: left;
  font-weight: 500;
}

button.link:hover:not(:disabled) {
  color: var(--modio-accent-hover);
  background: none;
}

.hint {
  text-align: center;
  padding: 1.25rem;
  border: 1px dashed var(--modio-border);
  border-radius: var(--modio-radius);
  background: var(--modio-surface);
}

.skip-row {
  margin-top: 1.25rem;
  text-align: center;
}

.hint,
.info,
.error {
  margin-top: 1rem;
}
</style>
