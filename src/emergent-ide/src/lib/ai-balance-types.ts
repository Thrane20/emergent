import type { AIProvider } from "./ai-settings";

export type AIBalance = {
  provider: AIProvider;
  label: string;
  state: "unchecked" | "ready" | "unsupported" | "unavailable";
  balance: number | null;
  message: string;
  billingUrl: string;
  checkedAt: string | null;
};
