"use client";

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import {
  telegramAuth,
  isTelegramEnvironment,
  initTelegramWebApp,
  type TelegramUser,
  type TelegramWebAppAuthResponse,
} from "./telegram";

type AuthStatus = "loading" | "authenticated" | "unauthenticated" | "error";

interface AuthContextType {
  status: AuthStatus;
  token: string | null;
  user: TelegramUser | null;
  error: string | null;
  isTelegram: boolean;
  expiresAt: string | null;
  authenticate: () => Promise<void>;
  logout: () => void;
  isTokenExpiringSoon: () => boolean;
  authFetch: <T>(path: string, init?: RequestInit) => Promise<T>;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

const TOKEN_STORAGE_KEY = "tg_auth_token";
const USER_STORAGE_KEY = "tg_auth_user";
const EXPIRES_STORAGE_KEY = "tg_auth_expires";

function getStoredAuth(): { token: string; user: TelegramUser; expiresAt: string } | null {
  if (typeof window === "undefined") return null;

  const token = localStorage.getItem(TOKEN_STORAGE_KEY);
  const userStr = localStorage.getItem(USER_STORAGE_KEY);
  const expiresAt = localStorage.getItem(EXPIRES_STORAGE_KEY);

  if (!token || !userStr || !expiresAt) return null;

  // Check if token is expired
  if (new Date(expiresAt) <= new Date()) {
    clearStoredAuth();
    return null;
  }

  try {
    const user = JSON.parse(userStr) as TelegramUser;
    return { token, user, expiresAt };
  } catch {
    clearStoredAuth();
    return null;
  }
}

function storeAuth(auth: TelegramWebAppAuthResponse) {
  localStorage.setItem(TOKEN_STORAGE_KEY, auth.token);
  localStorage.setItem(USER_STORAGE_KEY, JSON.stringify(auth.user));
  localStorage.setItem(EXPIRES_STORAGE_KEY, auth.expiresAt);
}

function clearStoredAuth() {
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

  const authenticate = useCallback(async () => {
    setStatus("loading");
    setError(null);

    try {
      const authResponse = await telegramAuth();
      storeAuth(authResponse);
      setToken(authResponse.token);
      setUser(authResponse.user);
      setExpiresAt(authResponse.expiresAt);
      setStatus("authenticated");
    } catch (err) {
      const message = err instanceof Error ? err.message : "Authentication failed";
      setError(message);
      setStatus("error");
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

  const isTokenExpiringSoon = useCallback(() => {
    if (!expiresAt) return true;
    const expiryTime = new Date(expiresAt).getTime();
    const now = Date.now();
    return expiryTime - now < TOKEN_EXPIRY_BUFFER_MS;
  }, [expiresAt]);

  const authFetch = useCallback(
    async <T,>(path: string, init?: RequestInit): Promise<T> => {
      // Refresh token if expiring soon
      if (isTokenExpiringSoon() && isTelegram) {
        await authenticate();
      }

      const currentToken = token;
      if (!currentToken) {
        throw new Error("Not authenticated");
      }

      const res = await fetch(path, {
        ...init,
        headers: {
          ...init?.headers,
          "Content-Type": "application/json",
          Authorization: `Bearer ${currentToken}`,
        },
      });

      // If 401, try to refresh and retry once
      if (res.status === 401 && isTelegram) {
        await authenticate();
        const newToken = localStorage.getItem(TOKEN_STORAGE_KEY);
        if (!newToken) {
          throw new Error("Failed to refresh authentication");
        }

        const retryRes = await fetch(path, {
          ...init,
          headers: {
            ...init?.headers,
            "Content-Type": "application/json",
            Authorization: `Bearer ${newToken}`,
          },
        });

        if (!retryRes.ok) {
          const text = await retryRes.text().catch(() => "");
          throw new Error(`API error (${retryRes.status}): ${text}`);
        }

        return (await retryRes.json()) as T;
      }

      if (!res.ok) {
        const text = await res.text().catch(() => "");
        throw new Error(`API error (${res.status}): ${text}`);
      }

      return (await res.json()) as T;
    },
    [token, isTelegram, isTokenExpiringSoon, authenticate]
  );

  useEffect(() => {
    const init = async () => {
      const inTelegram = isTelegramEnvironment();
      setIsTelegram(inTelegram);

      if (inTelegram) {
        initTelegramWebApp();
      }

      // Check for existing valid session
      const stored = getStoredAuth();
      if (stored) {
        setToken(stored.token);
        setUser(stored.user);
        setExpiresAt(stored.expiresAt);
        setStatus("authenticated");
        return;
      }

      // If in Telegram environment, auto-authenticate
      if (inTelegram) {
        try {
          await authenticate();
        } catch {
          // Error already set in authenticate()
        }
      } else {
        // Not in Telegram and no stored session
        setStatus("unauthenticated");
      }
    };

    init();
  }, [authenticate]);

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
