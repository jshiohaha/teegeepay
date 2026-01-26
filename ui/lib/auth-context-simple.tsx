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
const HAS_RESERVED_WALLET_KEY = "tg_has_reserved_wallet";

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
    hasReservedWallet: boolean;
    clearReservedWalletFlag: () => void;
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
    const [hasReservedWallet, setHasReservedWallet] = useState(false);
    const didInit = useRef(false);

    useEffect(() => {
        if (didInit.current) {
            return;
        }
        didInit.current = true;

        const doInit = async () => {
            // DEV MODE - still call backend to check for reserved wallet
            if (DEV_MODE) {
                try {
                    const res = await fetch("/api/auth/telegram", {
                        method: "POST",
                        headers: { "Content-Type": "application/json" },
                        body: JSON.stringify({ initData: "dev_mode" }),
                    });

                    if (res.ok) {
                        const authData = await res.json();
                        const { hasReservedWallet: reservedWallet } =
                            authData.data;
                        setToken(DEV_MOCK_TOKEN);
                        setUser(DEV_MOCK_USER);
                        setHasReservedWallet(reservedWallet || false);
                        if (reservedWallet) {
                            localStorage.setItem(
                                HAS_RESERVED_WALLET_KEY,
                                "true",
                            );
                        }
                        setStatus("authenticated");
                    } else {
                        setToken(DEV_MOCK_TOKEN);
                        setUser(DEV_MOCK_USER);
                        setStatus("authenticated");
                    }
                } catch (err) {
                    setToken(DEV_MOCK_TOKEN);
                    setUser(DEV_MOCK_USER);
                    setStatus("authenticated");
                }
                return;
            }

            // Wait for Telegram SDK
            let attempts = 0;
            while (attempts < 10) {
                const tg = window.Telegram?.WebApp;
                const data = tg?.initData;

                if (data && data.length > 0) {
                    tg?.ready();

                    // Check stored session first
                    const storedToken = localStorage.getItem(TOKEN_STORAGE_KEY);
                    const storedExpires =
                        localStorage.getItem(EXPIRES_STORAGE_KEY);
                    if (
                        storedToken &&
                        storedExpires &&
                        new Date(storedExpires) > new Date()
                    ) {
                        setToken(storedToken);
                        setUser(
                            JSON.parse(
                                localStorage.getItem(USER_STORAGE_KEY) || "{}",
                            ),
                        );
                        setHasReservedWallet(
                            localStorage.getItem(HAS_RESERVED_WALLET_KEY) ===
                                "true",
                        );
                        setStatus("authenticated");
                        return;
                    }

                    // Auth with backend
                    const res = await fetch("/api/auth/telegram", {
                        method: "POST",
                        headers: { "Content-Type": "application/json" },
                        body: JSON.stringify({ initData: data }),
                    });

                    if (res.ok) {
                        const authData = await res.json();
                        // Response is wrapped: { data: { token, user, expiresAt, hasReservedWallet } }
                        const {
                            token: newToken,
                            user: newUser,
                            expiresAt,
                            hasReservedWallet: reservedWallet,
                        } = authData.data;
                        localStorage.setItem(TOKEN_STORAGE_KEY, newToken);
                        localStorage.setItem(
                            USER_STORAGE_KEY,
                            JSON.stringify(newUser),
                        );
                        localStorage.setItem(EXPIRES_STORAGE_KEY, expiresAt);
                        if (reservedWallet) {
                            localStorage.setItem(
                                HAS_RESERVED_WALLET_KEY,
                                "true",
                            );
                        }
                        setToken(newToken);
                        setUser(newUser);
                        setHasReservedWallet(reservedWallet || false);
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
                await new Promise((r) => setTimeout(r, 100));
            }

            setStatus("unauthenticated");
        };

        doInit().catch((err) => {
            console.error("[SIMPLE_AUTH] Fatal error:", err);
            setError(String(err));
            setStatus("error");
        });
    }, []);

    const clearReservedWalletFlag = () => {
        localStorage.removeItem(HAS_RESERVED_WALLET_KEY);
        setHasReservedWallet(false);
    };

    const logout = () => {
        localStorage.removeItem(TOKEN_STORAGE_KEY);
        localStorage.removeItem(USER_STORAGE_KEY);
        localStorage.removeItem(EXPIRES_STORAGE_KEY);
        localStorage.removeItem(HAS_RESERVED_WALLET_KEY);
        setToken(null);
        setUser(null);
        setHasReservedWallet(false);
        setStatus("unauthenticated");
    };

    return (
        <AuthContext.Provider
            value={{
                status,
                token,
                user,
                error,
                hasReservedWallet,
                clearReservedWalletFlag,
                logout,
            }}
        >
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
