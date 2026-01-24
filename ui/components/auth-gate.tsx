"use client";

import { useAuth } from "@/lib/auth-context";
import { Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { ReactNode } from "react";

interface AuthGateProps {
  children: ReactNode;
  fallback?: ReactNode;
}

function AuthLoading() {
  return (
    <div className="w-full max-w-sm mx-auto h-[600px] bg-card rounded-2xl shadow-lg border border-border overflow-hidden flex flex-col items-center justify-center gap-4">
      <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
      <p className="text-sm text-muted-foreground">Authenticating...</p>
    </div>
  );
}

function AuthError() {
  const { error, authenticate, isTelegram } = useAuth();

  return (
    <div className="w-full max-w-sm mx-auto h-[600px] bg-card rounded-2xl shadow-lg border border-border overflow-hidden flex flex-col items-center justify-center gap-4 p-6">
      <div className="text-center space-y-2">
        <h2 className="text-lg font-semibold text-foreground">
          Authentication Failed
        </h2>
        <p className="text-sm text-muted-foreground">{error}</p>
      </div>
      {isTelegram && (
        <Button onClick={() => authenticate()} variant="default">
          Try Again
        </Button>
      )}
      {!isTelegram && (
        <p className="text-xs text-muted-foreground text-center">
          Please open this app inside Telegram to continue.
        </p>
      )}
    </div>
  );
}

function NotInTelegram() {
  return (
    <div className="w-full max-w-sm mx-auto h-[600px] bg-card rounded-2xl shadow-lg border border-border overflow-hidden flex flex-col items-center justify-center gap-4 p-6">
      <div className="text-center space-y-2">
        <h2 className="text-lg font-semibold text-foreground">
          Telegram Required
        </h2>
        <p className="text-sm text-muted-foreground">
          This app must be opened inside Telegram to work properly.
        </p>
      </div>
    </div>
  );
}

export function AuthGate({ children, fallback }: AuthGateProps) {
  const { status, isTelegram } = useAuth();

  if (status === "loading") {
    return <AuthLoading />;
  }

  if (status === "error") {
    return <AuthError />;
  }

  if (status === "unauthenticated") {
    if (!isTelegram) {
      return fallback ?? <NotInTelegram />;
    }
    // If in Telegram but unauthenticated, show loading (auto-auth should be happening)
    return <AuthLoading />;
  }

  return <>{children}</>;
}
