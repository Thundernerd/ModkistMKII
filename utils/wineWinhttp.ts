export interface WineWinhttpStatus {
  state:
    | "notApplicable"
    | "notFound"
    | "alreadyConfigured"
    | "applied"
    | "failed";
  message?: string;
  prefixLabel?: string;
}

export interface WineWinhttpFeedback {
  tone: "success" | "warn" | "error";
  text: string;
}

export function wineWinhttpFeedback(
  status?: WineWinhttpStatus | null,
): WineWinhttpFeedback | null {
  if (!status || status.state === "notApplicable") {
    return null;
  }

  if (status.state === "applied" || status.state === "alreadyConfigured") {
    const label = status.prefixLabel ? ` (${status.prefixLabel})` : "";
    return {
      tone: "success",
      text: `Modkist configured the Wine winhttp override for BepInEx${label}.`,
    };
  }

  if (status.state === "notFound") {
    return {
      tone: "warn",
      text:
        status.message ||
        'Did not find a Wine prefix for your game. Launch Zeepkist one time with Steam, Proton, CrossOver, or GameHub. Then the prefix exists. Then retry in Modkist. You can also add a winhttp override manually in Wine Configuration (Libraries → winhttp → native, builtin). On Linux with Proton, add WINEDLLOVERRIDES="winhttp=n,b" %command% to the Steam launch options.',
    };
  }

  return {
    tone: "error",
    text:
      status.message ||
      "Did not configure the Wine winhttp override. Add it manually in Wine Configuration (Libraries → winhttp → native, builtin).",
  };
}
