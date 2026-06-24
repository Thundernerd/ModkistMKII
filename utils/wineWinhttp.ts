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
      text: `Wine winhttp override configured for BepInEx${label}.`,
    };
  }

  if (status.state === "notFound") {
    return {
      tone: "warn",
      text:
        status.message ||
        "Could not find a Wine prefix for your game. Add a winhttp override manually in Wine Configuration (Libraries → winhttp → native, builtin). On Linux with Proton you can also add WINEDLLOVERRIDES=\"winhttp=n,b\" %command% to Steam launch options.",
    };
  }

  return {
    tone: "error",
    text:
      status.message ||
      "Could not configure the Wine winhttp override automatically. Add it manually in Wine Configuration (Libraries → winhttp → native, builtin).",
  };
}
