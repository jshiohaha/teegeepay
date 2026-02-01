"use client";

import { BalanceScreen } from "@/components/screens/balance-screen";
import { ConvertScreen } from "@/components/screens/convert-screen";
import { OnboardingScreen } from "@/components/screens/onboarding-screen";
import { ReviewScreen } from "@/components/screens/review-screen";
import { SendScreen } from "@/components/screens/send-screen";
import { StatusScreen } from "@/components/screens/status-screen";
import { SimpleAuthGate } from "@/components/simple-auth-gate";
import { SimpleAuthProvider } from "@/lib/auth-context-simple";
import { WalletProvider, useWallet } from "@/lib/wallet-context";

function WalletScreens() {
    const { currentScreen, isLoading } = useWallet();

    const screens = {
        onboarding: <OnboardingScreen />,
        balance: <BalanceScreen />,
        send: <SendScreen />,
        review: <ReviewScreen />,
        status: <StatusScreen />,
        convert: <ConvertScreen />,
    };

    return (
        <div className="w-full max-w-sm mx-auto h-[600px] bg-card rounded-2xl shadow-lg border border-border overflow-hidden flex flex-col relative">
            {isLoading ? (
                <div className="flex-1 flex items-center justify-center">
                    <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
                </div>
            ) : (
                screens[currentScreen]
            )}
        </div>
    );
}

export function WalletApp() {
    return (
        <SimpleAuthProvider>
            <SimpleAuthGate>
                <WalletProvider>
                    <WalletScreens />
                </WalletProvider>
            </SimpleAuthGate>
        </SimpleAuthProvider>
    );
}
