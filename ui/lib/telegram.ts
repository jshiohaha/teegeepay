// Telegram WebApp types and helpers

declare global {
  interface Window {
    Telegram?: {
      WebApp?: {
        initData: string;
        initDataUnsafe: {
          user?: {
            id: number;
            first_name: string;
            last_name?: string;
            username?: string;
            language_code?: string;
          };
          auth_date: number;
          hash: string;
        };
        ready: () => void;
        expand?: () => void;
        close?: () => void;
      };
    };
  }
}

export type TelegramUser = {
  telegramUserId: number;
  username?: string;
  firstName?: string;
  lastName?: string;
  languageCode?: string;
};

export type TelegramWebAppAuthRequest = {
  initData: string;
};

export type TelegramWebAppAuthResponse = {
  token: string;
  user: TelegramUser;
  expiresAt: string;
};

export function getTelegramWebApp() {
  return window.Telegram?.WebApp;
}

export function getTelegramInitData(): string | null {
  const initData = window.Telegram?.WebApp?.initData;
  return initData && initData.length > 0 ? initData : null;
}

export function isTelegramEnvironment(): boolean {
  return getTelegramInitData() !== null;
}

export function initTelegramWebApp() {
  const webApp = getTelegramWebApp();
  if (webApp) {
    webApp.ready();
    webApp.expand?.();
  }
}

export async function telegramAuth(): Promise<TelegramWebAppAuthResponse> {
  const initData = getTelegramInitData();
  if (!initData) {
    throw new Error("Not running inside Telegram (initData missing)");
  }

  initTelegramWebApp();

  const payload: TelegramWebAppAuthRequest = { initData };

  const res = await fetch("/api/auth/telegram", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });

  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new Error(`Auth failed (${res.status}): ${text}`);
  }

  return (await res.json()) as TelegramWebAppAuthResponse;
}

export async function apiFetch<TResponse>(
  path: string,
  token: string,
  init?: Omit<RequestInit, "headers"> & { headers?: Record<string, string> }
): Promise<TResponse> {
  const res = await fetch(path, {
    ...init,
    headers: {
      ...(init?.headers ?? {}),
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
  });

  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new Error(`API error (${res.status}): ${text}`);
  }

  return (await res.json()) as TResponse;
}
