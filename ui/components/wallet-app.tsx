"use client";

import { BalanceScreen } from "@/components/screens/balance-screen";
import { OnboardingScreen } from "@/components/screens/onboarding-screen";
import { ReviewScreen } from "@/components/screens/review-screen";
import { SendScreen } from "@/components/screens/send-screen";
import { StatusScreen } from "@/components/screens/status-screen";
import { ThemeToggle } from "@/components/theme-toggle";
import { AuthGate } from "@/components/auth-gate";
import { AuthProvider } from "@/lib/auth-context";
import { WalletProvider, useWallet } from "@/lib/wallet-context";

function WalletScreens() {
    const { currentScreen } = useWallet();

    const screens = {
        onboarding: <OnboardingScreen />,
        balance: <BalanceScreen />,
        send: <SendScreen />,
        review: <ReviewScreen />,
        status: <StatusScreen />,
    };

    return (
        <div className="w-full max-w-sm mx-auto h-[600px] bg-card rounded-2xl shadow-lg border border-border overflow-hidden flex flex-col relative">
            <div className="absolute top-3 right-3 z-10">
                <ThemeToggle />
            </div>
            {screens[currentScreen]}
        </div>
    );
}

export function WalletApp() {
    return (
        <AuthProvider>
            <AuthGate>
                <WalletProvider>
                    <WalletScreens />
                </WalletProvider>
            </AuthGate>
        </AuthProvider>
    );
}
