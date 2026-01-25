// Telegram WebApp types and helpers

import { DEV_MODE } from "./auth-context";

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

const createDevInitData = () => {
    const user = {
        id: 123,
        first_name: "Dev",
        last_name: "User",
        username: "dev-user",
        language_code: "en",
    };

    const params = new URLSearchParams();
    params.set("user", JSON.stringify(user));
    params.set("auth_date", Math.floor(Date.now() / 1000).toString());
    params.set("query_id", "AADevMockQueryId");
    params.set("hash", "dev_hash");

    return params.toString();
};

export function getTelegramInitData(): string | null {
    if (DEV_MODE) {
        return createDevInitData();
    } else {
        const initData = window.Telegram?.WebApp?.initData;
        return initData && initData.length > 0 ? initData : null;
    }
}

export function isTelegramEnvironment(): boolean {
    return getTelegramInitData() !== null;
}

export function initTelegramWebApp() {
    if (DEV_MODE) {
        return;
    }

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
    init?: Omit<RequestInit, "headers"> & { headers?: Record<string, string> },
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
