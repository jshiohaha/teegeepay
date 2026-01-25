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
    console.log("[TG] getTelegramInitData called, DEV_MODE:", DEV_MODE);
    if (DEV_MODE) {
        console.log("[TG] returning dev initData");
        return createDevInitData();
    } else {
        const webApp = window.Telegram?.WebApp;
        console.log("[TG] window.Telegram exists:", !!window.Telegram);
        console.log("[TG] window.Telegram.WebApp exists:", !!webApp);
        console.log("[TG] initData length:", webApp?.initData?.length ?? 0);
        console.log("[TG] initDataUnsafe:", JSON.stringify(webApp?.initDataUnsafe ?? {}));
        const initData = webApp?.initData;
        return initData && initData.length > 0 ? initData : null;
    }
}

export function isTelegramEnvironment(): boolean {
    const result = getTelegramInitData() !== null;
    console.log("[TG] isTelegramEnvironment:", result);
    return result;
}

export function initTelegramWebApp() {
    console.log("[TG] initTelegramWebApp called");
    if (DEV_MODE) {
        console.log("[TG] DEV_MODE, skipping init");
        return;
    }

    const webApp = getTelegramWebApp();
    console.log("[TG] webApp exists:", !!webApp);
    if (webApp) {
        console.log("[TG] calling webApp.ready()");
        webApp.ready();
        webApp.expand?.();
    }
}

export async function telegramAuth(): Promise<TelegramWebAppAuthResponse> {
    console.log("[TG] telegramAuth called");
    const initData = getTelegramInitData();
    console.log("[TG] initData exists:", !!initData, "length:", initData?.length ?? 0);
    
    if (!initData) {
        console.error("[TG] ERROR: initData missing");
        throw new Error("Not running inside Telegram (initData missing)");
    }

    initTelegramWebApp();

    const payload: TelegramWebAppAuthRequest = { initData };
    console.log("[TG] posting to /api/auth/telegram");

    const res = await fetch("/api/auth/telegram", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
    });

    console.log("[TG] auth response status:", res.status);

    if (!res.ok) {
        const text = await res.text().catch(() => "");
        console.error("[TG] auth failed:", res.status, text);
        throw new Error(`Auth failed (${res.status}): ${text}`);
    }

    const authResponse = (await res.json()) as TelegramWebAppAuthResponse;
    console.log("[TG] auth success, user:", authResponse.user?.telegramUserId, "expiresAt:", authResponse.expiresAt);
    return authResponse;
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
