"use client";

import { useSimpleAuth } from "@/lib/auth-context-simple";
import { Loader2 } from "lucide-react";
import type { ReactNode } from "react";

export function SimpleAuthGate({ children }: { children: ReactNode }) {
    const { status, error } = useSimpleAuth();

    if (status === "loading") {
        return (
            <div className="w-full max-w-sm mx-auto h-[600px] bg-card rounded-2xl shadow-lg border border-border overflow-hidden flex flex-col items-center justify-center gap-4">
                <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
                <p className="text-sm text-muted-foreground">
                    Authenticating...
                </p>
            </div>
        );
    }

    if (status === "error") {
        return (
            <div className="w-full max-w-sm mx-auto h-[600px] bg-card rounded-2xl shadow-lg border border-border overflow-hidden flex flex-col items-center justify-center gap-4 p-6">
                <h2 className="text-lg font-semibold">Auth Error</h2>
                <p className="text-sm text-muted-foreground">{error}</p>
            </div>
        );
    }

    if (status === "unauthenticated") {
        return (
            <div className="w-full max-w-sm mx-auto h-[600px] bg-card rounded-2xl shadow-lg border border-border overflow-hidden flex flex-col items-center justify-center gap-4 p-6">
                <h2 className="text-lg font-semibold">Telegram Required</h2>
                <p className="text-sm text-muted-foreground">
                    Open this app in Telegram
                </p>
            </div>
        );
    }

    return <>{children}</>;
}
