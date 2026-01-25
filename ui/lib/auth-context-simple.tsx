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

    console.log("[SIMPLE_AUTH] Component render - status:", status);

    useEffect(() => {
        console.log("[SIMPLE_AUTH] useEffect - didInit:", didInit.current);
        
        if (didInit.current) {
            console.log("[SIMPLE_AUTH] Already initialized, skipping");
            return;
        }
        didInit.current = true;

        const doInit = async () => {
            console.log("[SIMPLE_AUTH] doInit starting");
            console.log("[SIMPLE_AUTH] DEV_MODE:", DEV_MODE);

            // DEV MODE
            if (DEV_MODE) {
                console.log("[SIMPLE_AUTH] Dev mode - auto auth");
                setToken(DEV_MOCK_TOKEN);
                setUser(DEV_MOCK_USER);
                setStatus("authenticated");
                return;
            }

            // Wait for Telegram SDK
            console.log("[SIMPLE_AUTH] Checking Telegram SDK...");
            let attempts = 0;
            while (attempts < 10) {
                const tg = window.Telegram?.WebApp;
                const data = tg?.initData;
                console.log("[SIMPLE_AUTH] Attempt", attempts, "- WebApp:", !!tg, "initData:", !!data, "len:", data?.length);
                
                if (data && data.length > 0) {
                    console.log("[SIMPLE_AUTH] Got initData, proceeding with auth");
                    tg?.ready();
                    
                    // Check stored session first
                    const storedToken = localStorage.getItem(TOKEN_STORAGE_KEY);
                    const storedExpires = localStorage.getItem(EXPIRES_STORAGE_KEY);
                    if (storedToken && storedExpires && new Date(storedExpires) > new Date()) {
                        console.log("[SIMPLE_AUTH] Using stored session");
                        setToken(storedToken);
                        setUser(JSON.parse(localStorage.getItem(USER_STORAGE_KEY) || "{}"));
                        setStatus("authenticated");
                        return;
                    }
                    
                    // Auth with backend
                    console.log("[SIMPLE_AUTH] Calling /api/auth/telegram");
                    const res = await fetch("/api/auth/telegram", {
                        method: "POST",
                        headers: { "Content-Type": "application/json" },
                        body: JSON.stringify({ initData: data }),
                    });
                    
                    console.log("[SIMPLE_AUTH] Response:", res.status);
                    
                    if (res.ok) {
                        const authData = await res.json();
                        console.log("[SIMPLE_AUTH] Auth success");
                        localStorage.setItem(TOKEN_STORAGE_KEY, authData.token);
                        localStorage.setItem(USER_STORAGE_KEY, JSON.stringify(authData.user));
                        localStorage.setItem(EXPIRES_STORAGE_KEY, authData.expiresAt);
                        setToken(authData.token);
                        setUser(authData.user);
                        setStatus("authenticated");
                    } else {
                        const text = await res.text();
                        console.error("[SIMPLE_AUTH] Auth failed:", text);
                        setError(`Auth failed: ${res.status}`);
                        setStatus("error");
                    }
                    return;
                }
                
                attempts++;
                await new Promise(r => setTimeout(r, 100));
            }
            
            console.log("[SIMPLE_AUTH] No Telegram initData after 10 attempts");
            setStatus("unauthenticated");
        };

        doInit().catch(err => {
            console.error("[SIMPLE_AUTH] Fatal error:", err);
            setError(String(err));
            setStatus("error");
        });
    }, []);

    const logout = () => {
        localStorage.removeItem(TOKEN_STORAGE_KEY);
        localStorage.removeItem(USER_STORAGE_KEY);
        localStorage.removeItem(EXPIRES_STORAGE_KEY);
        setToken(null);
        setUser(null);
        setStatus("unauthenticated");
    };

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
