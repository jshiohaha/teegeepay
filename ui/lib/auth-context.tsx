"use client";

import {
    createContext,
    useCallback,
    useContext,
    useEffect,
    useRef,
    useState,
    type ReactNode,
} from "react";
import {
    initTelegramWebApp,
    isTelegramEnvironment,
    telegramAuth,
    type TelegramUser,
    type TelegramWebAppAuthResponse,
} from "./telegram";

export const DEV_MODE = process.env.NEXT_PUBLIC_DEV_MODE === "true";

const DEV_MOCK_USER: TelegramUser = {
    telegramUserId: 123456789,
    username: "dev_user",
    firstName: "Dev",
    lastName: "User",
    languageCode: "en",
};

const DEV_MOCK_TOKEN = "dev_mock_token_for_local_testing";
const DEV_MOCK_EXPIRES_AT = new Date(
    Date.now() + 365 * 24 * 60 * 60 * 1000,
).toISOString();

type AuthStatus = "loading" | "authenticated" | "unauthenticated" | "error";

interface AuthContextType {
    status: AuthStatus;
    token: string | null;
    user: TelegramUser | null;
    error: string | null;
    isTelegram: boolean;
    expiresAt: string | null;
    authenticate: () => Promise<TelegramWebAppAuthResponse>;
    logout: () => void;
    isTokenExpiringSoon: () => boolean;
    authFetch: <T>(path: string, init?: RequestInit) => Promise<T>;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

const TOKEN_STORAGE_KEY = "tg_auth_token";
const USER_STORAGE_KEY = "tg_auth_user";
const EXPIRES_STORAGE_KEY = "tg_auth_expires";

function getStoredAuth(): {
    token: string;
    user: TelegramUser;
    expiresAt: string;
} | null {
    console.log("[AUTH] getStoredAuth called");
    if (typeof window === "undefined") {
        console.log("[AUTH] window undefined, returning null");
        return null;
    }

    const token = localStorage.getItem(TOKEN_STORAGE_KEY);
    const userStr = localStorage.getItem(USER_STORAGE_KEY);
    const expiresAt = localStorage.getItem(EXPIRES_STORAGE_KEY);

    console.log("[AUTH] stored token exists:", !!token, "length:", token?.length ?? 0);
    console.log("[AUTH] stored user exists:", !!userStr);
    console.log("[AUTH] stored expiresAt:", expiresAt);

    if (!token || !userStr || !expiresAt) {
        console.log("[AUTH] missing stored auth data");
        return null;
    }

    // Check if token is expired
    const isExpired = new Date(expiresAt) <= new Date();
    console.log("[AUTH] token expired:", isExpired, "expiresAt:", expiresAt, "now:", new Date().toISOString());
    if (isExpired) {
        console.log("[AUTH] clearing expired auth");
        clearStoredAuth();
        return null;
    }

    try {
        const user = JSON.parse(userStr) as TelegramUser;
        console.log("[AUTH] returning stored auth for user:", user.telegramUserId);
        return { token, user, expiresAt };
    } catch {
        console.log("[AUTH] failed to parse stored user, clearing");
        clearStoredAuth();
        return null;
    }
}

function storeAuth(auth: TelegramWebAppAuthResponse) {
    console.log("[AUTH] storeAuth called, user:", auth.user?.telegramUserId, "expiresAt:", auth.expiresAt);
    localStorage.setItem(TOKEN_STORAGE_KEY, auth.token);
    localStorage.setItem(USER_STORAGE_KEY, JSON.stringify(auth.user));
    localStorage.setItem(EXPIRES_STORAGE_KEY, auth.expiresAt);
}

function clearStoredAuth() {
    console.log("[AUTH] clearStoredAuth called");
    localStorage.removeItem(TOKEN_STORAGE_KEY);
    localStorage.removeItem(USER_STORAGE_KEY);
    localStorage.removeItem(EXPIRES_STORAGE_KEY);
}

const TOKEN_EXPIRY_BUFFER_MS = 5 * 60 * 1000; // Refresh if expiring within 5 minutes

export function AuthProvider({ children }: { children: ReactNode }) {
    const [status, setStatus] = useState<AuthStatus>("loading");
    const [token, setToken] = useState<string | null>(null);
    const [user, setUser] = useState<TelegramUser | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [isTelegram, setIsTelegram] = useState(false);
    const [expiresAt, setExpiresAt] = useState<string | null>(null);
    const isAuthenticatingRef = useRef(false);
    const hasInitializedRef = useRef(false);

    const authenticate = useCallback(async (): Promise<TelegramWebAppAuthResponse> => {
        console.log("[AUTH] authenticate() called, isAuthenticatingRef:", isAuthenticatingRef.current);
        if (isAuthenticatingRef.current) {
            console.log("[AUTH] already authenticating, throwing");
            throw new Error("Authentication already in progress");
        }
        
        isAuthenticatingRef.current = true;
        console.log("[AUTH] setting status to loading");
        setStatus("loading");
        setError(null);

        try {
            if (DEV_MODE) {
                console.log("[AUTH] DEV_MODE, using mock auth");
                const authResponse = {
                    token: DEV_MOCK_TOKEN,
                    user: DEV_MOCK_USER,
                    expiresAt: DEV_MOCK_EXPIRES_AT,
                };
                storeAuth(authResponse);
                setToken(authResponse.token);
                setUser(authResponse.user);
                setExpiresAt(authResponse.expiresAt);
                setStatus("authenticated");
                isAuthenticatingRef.current = false;
                console.log("[AUTH] DEV_MODE auth complete");
                return authResponse;
            }

            console.log("[AUTH] calling telegramAuth()");
            const authResponse = await telegramAuth();
            console.log("[AUTH] telegramAuth() returned, storing auth");
            storeAuth(authResponse);
            setToken(authResponse.token);
            setUser(authResponse.user);
            setExpiresAt(authResponse.expiresAt);
            setStatus("authenticated");
            isAuthenticatingRef.current = false;
            console.log("[AUTH] authenticate() complete, status set to authenticated");
            return authResponse;
        } catch (err) {
            const message =
                err instanceof Error ? err.message : "Authentication failed";
            console.error("[AUTH] authenticate() error:", message);
            setError(message);
            setStatus("error");
            isAuthenticatingRef.current = false;
            throw err;
        }
    }, []);

    const logout = useCallback(() => {
        clearStoredAuth();
        setToken(null);
        setUser(null);
        setExpiresAt(null);
        setStatus("unauthenticated");
    }, []);

    const getExpiresAtValue = useCallback(() => {
        if (expiresAt) return expiresAt;
        if (typeof window === "undefined") return null;
        return localStorage.getItem(EXPIRES_STORAGE_KEY);
    }, [expiresAt]);

    const isTokenExpiringSoon = useCallback(() => {
        const expiryValue = getExpiresAtValue();
        if (!expiryValue) return true;
        const expiryTime = new Date(expiryValue).getTime();
        if (Number.isNaN(expiryTime)) return true;
        const now = Date.now();
        return expiryTime - now < TOKEN_EXPIRY_BUFFER_MS;
    }, [getExpiresAtValue]);

    const authFetch = useCallback(
        async <T,>(path: string, init?: RequestInit): Promise<T> => {
            console.log("[AUTH] authFetch called, path:", path);
            console.log("[AUTH] authFetch state - token exists:", !!token, "isTelegram:", isTelegram, "isAuthenticating:", isAuthenticatingRef.current);
            
            // Always try to get token from localStorage as fallback (state might be stale)
            let currentToken = token ?? localStorage.getItem(TOKEN_STORAGE_KEY);
            console.log("[AUTH] authFetch - currentToken from state or localStorage:", !!currentToken);
            
            const expiringSoon = isTokenExpiringSoon();
            console.log("[AUTH] authFetch - isTokenExpiringSoon:", expiringSoon);
            
            if (expiringSoon && isTelegram && !isAuthenticatingRef.current) {
                console.log("[AUTH] authFetch - token expiring soon, refreshing");
                try {
                    const refreshed = await authenticate();
                    currentToken =
                        refreshed?.token ??
                        localStorage.getItem(TOKEN_STORAGE_KEY) ??
                        null;
                    console.log("[AUTH] authFetch - token refreshed");
                } catch (err) {
                    console.log("[AUTH] authFetch - refresh failed, using existing token:", err);
                    // currentToken already set above, no need to reassign
                }
            }

            console.log("[AUTH] authFetch - currentToken exists:", !!currentToken);
            if (!currentToken) {
                console.error("[AUTH] authFetch - no token available, throwing");
                throw new Error("Not authenticated");
            }

            console.log("[AUTH] authFetch - executing fetch to:", path);
            const res = await fetch(path, {
                ...init,
                headers: {
                    ...init?.headers,
                    "Content-Type": "application/json",
                    Authorization: `Bearer ${currentToken}`,
                },
            });

            console.log("[AUTH] authFetch - response status:", res.status, "for path:", path);

            // If 401, try to refresh and retry once (but not if already authenticating)
            if (res.status === 401 && isTelegram && !isAuthenticatingRef.current) {
                console.log("[AUTH] authFetch - got 401, attempting refresh");
                try {
                    await authenticate();
                    console.log("[AUTH] authFetch - refresh after 401 succeeded");
                } catch (err) {
                    console.error("[AUTH] authFetch - refresh after 401 failed:", err);
                    throw new Error("Authentication failed");
                }
                const newToken = localStorage.getItem(TOKEN_STORAGE_KEY);
                if (!newToken) {
                    console.error("[AUTH] authFetch - no new token after refresh");
                    throw new Error("Failed to refresh authentication");
                }

                console.log("[AUTH] authFetch - retrying request with new token");
                const retryRes = await fetch(path, {
                    ...init,
                    headers: {
                        ...init?.headers,
                        "Content-Type": "application/json",
                        Authorization: `Bearer ${newToken}`,
                    },
                });

                console.log("[AUTH] authFetch - retry response status:", retryRes.status);
                if (!retryRes.ok) {
                    const text = await retryRes.text().catch(() => "");
                    console.error("[AUTH] authFetch - retry failed:", retryRes.status, text);
                    throw new Error(`API error (${retryRes.status}): ${text}`);
                }

                return (await retryRes.json()) as T;
            }

            if (!res.ok) {
                const text = await res.text().catch(() => "");
                console.error("[AUTH] authFetch - request failed:", res.status, text);
                throw new Error(`API error (${res.status}): ${text}`);
            }

            console.log("[AUTH] authFetch - success for path:", path);
            return (await res.json()) as T;
        },
        [token, isTelegram, isTokenExpiringSoon, authenticate],
    );

    useEffect(() => {
        // Prevent double initialization (React Strict Mode, HMR, etc.)
        if (hasInitializedRef.current) {
            console.log("[AUTH] useEffect skipped - already initialized");
            return;
        }
        hasInitializedRef.current = true;
        
        console.log("[AUTH] useEffect running, DEV_MODE:", DEV_MODE);
        const init = async () => {
            console.log("[AUTH] init() starting");
            if (DEV_MODE) {
                console.log("[AUTH] DEV_MODE path");
                const stored = getStoredAuth();
                if (stored) {
                    console.log("[AUTH] DEV_MODE: using stored auth");
                    setToken(stored.token);
                    setUser(stored.user);
                    setExpiresAt(stored.expiresAt);
                } else {
                    console.log("[AUTH] DEV_MODE: no stored auth, creating mock");
                    storeAuth({
                        token: DEV_MOCK_TOKEN,
                        user: DEV_MOCK_USER,
                        expiresAt: DEV_MOCK_EXPIRES_AT,
                    });
                    setToken(DEV_MOCK_TOKEN);
                    setUser(DEV_MOCK_USER);
                    setExpiresAt(DEV_MOCK_EXPIRES_AT);
                }

                setIsTelegram(false);
                setStatus("authenticated");
                console.log("[AUTH] DEV_MODE init complete");
                return;
            }

            console.log("[AUTH] checking isTelegramEnvironment()");
            const inTelegram = isTelegramEnvironment();
            console.log("[AUTH] inTelegram:", inTelegram);
            setIsTelegram(inTelegram);

            if (inTelegram) {
                console.log("[AUTH] initializing Telegram WebApp");
                initTelegramWebApp();
            }

            // Check for existing valid session
            console.log("[AUTH] checking for stored session");
            const stored = getStoredAuth();
            if (stored) {
                console.log("[AUTH] found valid stored session, using it");
                setToken(stored.token);
                setUser(stored.user);
                setExpiresAt(stored.expiresAt);
                setStatus("authenticated");
                console.log("[AUTH] init complete with stored session");
                return;
            }

            // If in Telegram environment, auto-authenticate
            if (inTelegram) {
                console.log("[AUTH] in Telegram, no stored session, calling telegramAuth directly");
                isAuthenticatingRef.current = true;
                setStatus("loading");
                setError(null);
                
                try {
                    const authResponse = await telegramAuth();
                    console.log("[AUTH] telegramAuth() succeeded");
                    storeAuth(authResponse);
                    setToken(authResponse.token);
                    setUser(authResponse.user);
                    setExpiresAt(authResponse.expiresAt);
                    setStatus("authenticated");
                } catch (err) {
                    const message = err instanceof Error ? err.message : "Authentication failed";
                    console.error("[AUTH] telegramAuth() failed in init:", message);
                    setError(message);
                    setStatus("error");
                } finally {
                    isAuthenticatingRef.current = false;
                }
            } else {
                console.log("[AUTH] not in Telegram, no stored session, setting unauthenticated");
                setStatus("unauthenticated");
            }
            console.log("[AUTH] init() complete");
        };

        init();
    }, []); // No dependencies - run once on mount

    return (
        <AuthContext.Provider
            value={{
                status,
                token,
                user,
                error,
                isTelegram,
                expiresAt,
                authenticate,
                logout,
                isTokenExpiringSoon,
                authFetch,
            }}
        >
            {children}
        </AuthContext.Provider>
    );
}

export function useAuth() {
    const context = useContext(AuthContext);
    if (context === undefined) {
        throw new Error("useAuth must be used within an AuthProvider");
    }
    return context;
}
