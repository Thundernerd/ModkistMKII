import { rateLimitUserMessage } from "~/utils/modioError";

const BANNER_DURATION_MS = 60_000;

const bannerVisible = ref(false);
let hideTimer: ReturnType<typeof setTimeout> | undefined;

export function useModioRateLimit() {
  function showRateLimitBanner() {
    bannerVisible.value = true;

    if (hideTimer) {
      clearTimeout(hideTimer);
    }

    hideTimer = setTimeout(() => {
      bannerVisible.value = false;
      hideTimer = undefined;
    }, BANNER_DURATION_MS);
  }

  function dismissRateLimitBanner() {
    bannerVisible.value = false;
    if (hideTimer) {
      clearTimeout(hideTimer);
      hideTimer = undefined;
    }
  }

  return {
    rateLimitBannerVisible: bannerVisible,
    rateLimitBannerMessage: rateLimitUserMessage,
    showRateLimitBanner,
    dismissRateLimitBanner,
  };
}
