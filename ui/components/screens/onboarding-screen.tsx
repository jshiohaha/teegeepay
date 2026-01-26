"use client";

import { Button } from "@/components/ui/button";
import { useWallet } from "@/lib/wallet-context";
import { Gift, Shield, Sparkles, Zap } from "lucide-react";
import { useState } from "react";

export function OnboardingScreen() {
    const { createWallet, claimWallet, onboardingMode } = useWallet();

    const [isCreating, setIsCreating] = useState(false);
    const [debugMsg, setDebugMsg] = useState("Ready");

    const isClaimMode = onboardingMode === "claim";

    const handleCreate = async () => {
        setDebugMsg("Clicked!");
        setIsCreating(true);
        setDebugMsg(isClaimMode ? "Claiming wallet..." : "Creating wallet...");
        try {
            if (isClaimMode) {
                await claimWallet();
            } else {
                await createWallet();
            }
            setDebugMsg("Success!");
            setIsCreating(false);
        } catch (error) {
            setDebugMsg(
                `Error: ${error instanceof Error ? error.message : String(error)}`,
            );
            setIsCreating(false);
        }
    };

    return (
        <div className="flex flex-col h-full">
            {/* Header */}
            <div className="flex items-center justify-center p-4 border-b border-border">
                <div className="flex items-center gap-2">
                    {/* <div className="flex items-center justify-center w-8 h-8 rounded-full bg-primary">
                        <Wallet className="w-4 h-4 text-primary-foreground" />
                    </div> */}
                    <span className="font-semibold text-foreground">
                        Cypherpay Wallet
                    </span>
                </div>
            </div>

            {/* Content */}
            <div className="flex-1 p-6 flex flex-col">
                {/* Hero Section */}
                <div className="flex-1 flex flex-col items-center justify-center text-center">
                    <div className="w-20 h-20 rounded-2xl bg-linear-to-br from-primary/10 to-primary/5 flex items-center justify-center mb-6">
                        {isClaimMode ? (
                            <Gift className="w-10 h-10 text-primary" />
                        ) : (
                            <Sparkles className="w-10 h-10 text-primary" />
                        )}
                    </div>

                    <h1 className="text-2xl font-bold text-foreground mb-2 text-balance">
                        {isClaimMode
                            ? "You've Got Crypto!"
                            : "For Your Eyes Only"}
                    </h1>
                    <p className="text-muted-foreground text-lg leading-relaxed max-w-[260px]">
                        Send and receive confidentially
                    </p>
                </div>

                {/* Features */}
                <div className="space-y-3 mb-8">
                    <div className="flex items-center gap-3 p-3 rounded-lg bg-secondary/50">
                        <div className="w-9 h-9 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                            <Shield className="w-4 h-4 text-primary" />
                        </div>
                        <div className="text-left">
                            <p className="text-sm font-medium text-foreground">
                                Confidential by default
                            </p>
                            <p className="text-xs text-muted-foreground">
                                Transfers and balances encrypted
                            </p>
                        </div>
                    </div>

                    <div className="flex items-center gap-3 p-3 rounded-lg bg-secondary/50">
                        <div className="w-9 h-9 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                            <Zap className="w-4 h-4 text-primary" />
                        </div>
                        <div className="text-left">
                            <p className="text-sm font-medium text-foreground">
                                Lightning Fast
                            </p>
                            <p className="text-xs text-muted-foreground">
                                Settled on Solana
                            </p>
                        </div>
                    </div>
                </div>

                {/* CTA */}
                <Button
                    className="w-full h-12 bg-primary hover:bg-primary/90 text-primary-foreground"
                    onClick={handleCreate}
                    disabled={isCreating}
                >
                    {isCreating ? (
                        <span className="flex items-center gap-2">
                            <span className="w-4 h-4 border-2 border-primary-foreground/30 border-t-primary-foreground rounded-full animate-spin" />
                            {isClaimMode
                                ? "Claiming Wallet..."
                                : "Creating Wallet..."}
                        </span>
                    ) : (
                        <span className="text-md hover:cursor-pointer">
                            {isClaimMode ? "Claim Wallet" : "Create Wallet"}
                        </span>
                    )}
                </Button>

                <p className="text-xs text-muted-foreground text-center mt-4">
                    By continuing, you agree to our Terms of Service
                </p>

                {/* Debug message - remove after fixing */}
                <p className="text-xs text-center mt-2 p-2 bg-yellow-100 text-yellow-800 rounded">
                    Debug: {debugMsg}
                </p>
            </div>
        </div>
    );
}
