"use client";

import {
    createContext,
    useContext,
    useEffect,
    useRef,
    useState,
    type ReactNode,
} from "react";

export const DEV_MODE = process.env.NEXT_PUBLIC_DEV_MODE === "true";

const TOKEN_STORAGE_KEY = "tg_auth_token";
const USER_STORAGE_KEY = "tg_auth_user";
const EXPIRES_STORAGE_KEY = "tg_auth_expires";

export type TelegramUser = {
    telegramUserId: number;
    username?: string;
    firstName?: string;
    lastName?: string;
    languageCode?: string;
};

type AuthStatus = "loading" | "authenticated" | "unauthenticated" | "error";

interface AuthContextType {
    status: AuthStatus;
    token: string | null;
    user: TelegramUser | null;
    error: string | null;
    logout: () => void;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

const DEV_MOCK_USER: TelegramUser = {
    telegramUserId: 123456789,
    username: "dev_user",
    firstName: "Dev",
    lastName: "User",
    languageCode: "en",
};
const DEV_MOCK_TOKEN = "dev_mock_token_for_local_testing";

export function SimpleAuthProvider({ children }: { children: ReactNode }) {
    const [status, setStatus] = useState<AuthStatus>("loading");
    const [token, setToken] = useState<string | null>(null);
    const [user, setUser] = useState<TelegramUser | null>(null);
    const [error, setError] = useState<string | null>(null);
    const didInit = useRef(false);

    useEffect(() => {
        if (didInit.current) return;
        didInit.current = true;

        const init = async () => {
            console.log("[SIMPLE_AUTH] Starting init, DEV_MODE:", DEV_MODE);

            // DEV MODE - instant auth
            if (DEV_MODE) {
                console.log("[SIMPLE_AUTH] Dev mode - setting authenticated");
                setToken(DEV_MOCK_TOKEN);
                setUser(DEV_MOCK_USER);
                setStatus("authenticated");
                return;
            }

            // Check for Telegram
            const tgWebApp = window.Telegram?.WebApp;
            const initData = tgWebApp?.initData;
            console.log("[SIMPLE_AUTH] Telegram WebApp exists:", !!tgWebApp);
            console.log("[SIMPLE_AUTH] initData exists:", !!initData, "length:", initData?.length);

            if (!initData || initData.length === 0) {
                console.log("[SIMPLE_AUTH] Not in Telegram, setting unauthenticated");
                setStatus("unauthenticated");
                return;
            }

            // Check localStorage for existing session
            const storedToken = localStorage.getItem(TOKEN_STORAGE_KEY);
            const storedUser = localStorage.getItem(USER_STORAGE_KEY);
            const storedExpires = localStorage.getItem(EXPIRES_STORAGE_KEY);

            if (storedToken && storedUser && storedExpires) {
                const isExpired = new Date(storedExpires) <= new Date();
                if (!isExpired) {
                    console.log("[SIMPLE_AUTH] Found valid stored session");
                    setToken(storedToken);
                    setUser(JSON.parse(storedUser));
                    setStatus("authenticated");
                    return;
                }
                console.log("[SIMPLE_AUTH] Stored session expired, clearing");
                localStorage.removeItem(TOKEN_STORAGE_KEY);
                localStorage.removeItem(USER_STORAGE_KEY);
                localStorage.removeItem(EXPIRES_STORAGE_KEY);
            }

            // Authenticate with backend
            console.log("[SIMPLE_AUTH] Authenticating with backend...");
            try {
                tgWebApp?.ready();
                
                const res = await fetch("/api/auth/telegram", {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({ initData }),
                });

                console.log("[SIMPLE_AUTH] Auth response status:", res.status);

                if (!res.ok) {
                    const text = await res.text().catch(() => "");
                    throw new Error(`Auth failed (${res.status}): ${text}`);
                }

                const data = await res.json();
                console.log("[SIMPLE_AUTH] Auth success, user:", data.user?.telegramUserId);

                // Store in localStorage
                localStorage.setItem(TOKEN_STORAGE_KEY, data.token);
                localStorage.setItem(USER_STORAGE_KEY, JSON.stringify(data.user));
                localStorage.setItem(EXPIRES_STORAGE_KEY, data.expiresAt);

                // Update state
                setToken(data.token);
                setUser(data.user);
                setStatus("authenticated");
                console.log("[SIMPLE_AUTH] State updated to authenticated");
            } catch (err) {
                console.error("[SIMPLE_AUTH] Auth error:", err);
                setError(err instanceof Error ? err.message : "Auth failed");
                setStatus("error");
            }
        };

        init();
    }, []);

    const logout = () => {
        localStorage.removeItem(TOKEN_STORAGE_KEY);
        localStorage.removeItem(USER_STORAGE_KEY);
        localStorage.removeItem(EXPIRES_STORAGE_KEY);
        setToken(null);
        setUser(null);
        setStatus("unauthenticated");
    };

    console.log("[SIMPLE_AUTH] Render - status:", status, "hasToken:", !!token);

    return (
        <AuthContext.Provider value={{ status, token, user, error, logout }}>
            {children}
        </AuthContext.Provider>
    );
}

export function useSimpleAuth() {
    const context = useContext(AuthContext);
    if (!context) {
        throw new Error("useSimpleAuth must be used within SimpleAuthProvider");
    }
    return context;
}
