"use client";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useWallet } from "@/lib/wallet-context";
import { ArrowRight, ChevronLeft, Eye, EyeOff, Loader2 } from "lucide-react";
import { useState } from "react";

export function ConvertScreen() {
    const {
        wallet,
        conversion,
        setConversion,
        setCurrentScreen,
        executeConversion,
        setTransaction,
        setTransactionStatus,
        setTransactionMessage,
        refreshBalance,
    } = useWallet();

    const [amount, setAmount] = useState(conversion.amount);
    const [error, setError] = useState<string | undefined>();
    const [isConverting, setIsConverting] = useState(false);

    const isToPrivate = conversion.direction === "toPrivate";
    const availableBalance = isToPrivate
        ? wallet.cusd.public
        : wallet.cusd.private;
    const normalizedAvailableBalance = Number.isFinite(availableBalance)
        ? availableBalance
        : 0;

    const validateAndConvert = async () => {
        const amountNum = parseFloat(amount);

        if (!amount.trim()) {
            setError("Please enter an amount");
            return;
        }
        if (isNaN(amountNum) || amountNum <= 0) {
            setError("Please enter a valid amount");
            return;
        }
        if (amountNum > availableBalance) {
            setError("Insufficient balance");
            return;
        }

        setError(undefined);
        setIsConverting(true);

        try {
            setConversion({ amount });
            const signature = await executeConversion(amount);
            setTransaction({
                steps: signature.map((signature) => ({
                    label: signature.label,
                    txId: signature.signature,
                })),
            });
            setTransactionStatus("success");
            setTransactionMessage(
                isToPrivate
                    ? "Your balance is now private."
                    : "Your balance is now public.",
            );
            setCurrentScreen("status");
            await refreshBalance();
        } catch (err) {
            console.error("Conversion failed:", err);
            setError("Conversion failed. Please try again.");
        } finally {
            setIsConverting(false);
        }
    };

    const handleMaxAmount = () => {
        setAmount(normalizedAvailableBalance.toString());
        setError(undefined);
    };

    const formatBalance = (value: number, decimals = 2) => {
        return value.toLocaleString(undefined, {
            minimumFractionDigits: decimals,
            maximumFractionDigits: decimals,
        });
    };

    return (
        <div className="flex flex-col h-full">
            {/* Header */}
            <div className="flex items-center gap-3 p-4 border-b border-border">
                <button
                    onClick={() => setCurrentScreen("balance")}
                    className="flex items-center justify-center w-8 h-8 rounded-full hover:bg-secondary transition-colors"
                >
                    <ChevronLeft className="w-5 h-5 text-foreground" />
                </button>
                <span className="font-semibold text-foreground">
                    {isToPrivate ? "Make Private" : "Make Public"}
                </span>
            </div>

            {/* Content */}
            <div className="flex-1 p-4 flex flex-col">
                {/* Direction Indicator */}
                <div className="flex items-center justify-center gap-3 py-4 mb-4">
                    <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-secondary/50">
                        {isToPrivate ? (
                            <Eye className="w-4 h-4 text-muted-foreground" />
                        ) : (
                            <EyeOff className="w-4 h-4 text-muted-foreground" />
                        )}
                        <span className="text-sm font-medium text-foreground">
                            {isToPrivate ? "Public" : "Private"}
                        </span>
                    </div>
                    <ArrowRight className="w-4 h-4 text-muted-foreground" />
                    <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-primary/10 border border-primary/20">
                        {isToPrivate ? (
                            <EyeOff className="w-4 h-4 text-primary" />
                        ) : (
                            <Eye className="w-4 h-4 text-primary" />
                        )}
                        <span className="text-sm font-medium text-primary">
                            {isToPrivate ? "Private" : "Public"}
                        </span>
                    </div>
                </div>

                {/* Amount Input */}
                <div className="space-y-2">
                    <div className="flex items-center justify-between">
                        <Label
                            htmlFor="amount"
                            className="text-xs uppercase tracking-wider font-medium text-muted-foreground"
                        >
                            Amount
                        </Label>
                        <button
                            onClick={handleMaxAmount}
                            className="text-xs font-medium text-accent hover:underline"
                        >
                            Max: {formatBalance(normalizedAvailableBalance)}{" "}
                            cUSD
                        </button>
                    </div>
                    <div className="relative">
                        <Input
                            id="amount"
                            type="number"
                            placeholder="0.00"
                            value={amount}
                            onChange={(e) => {
                                setAmount(e.target.value);
                                setError(undefined);
                            }}
                            className={`h-12 pr-16 bg-secondary/50 border-border text-foreground placeholder:text-muted-foreground ${
                                error ? "border-destructive" : ""
                            }`}
                        />
                        <div className="absolute right-3 top-1/2 -translate-y-1/2 text-sm font-medium text-muted-foreground">
                            cUSD
                        </div>
                    </div>
                    {error && (
                        <p className="text-xs text-destructive">{error}</p>
                    )}
                </div>

                {/* Info Box */}
                <div className="mt-6 p-3 rounded-lg bg-secondary/30">
                    <p className="text-xs text-muted-foreground leading-relaxed">
                        {isToPrivate
                            ? "Cncrypt your balance. Only you will be able to see it."
                            : "Make your balance visible to anyone."}
                    </p>
                </div>

                {/* Spacer */}
                <div className="flex-1" />

                {/* Actions */}
                <div className="space-y-3 mt-6">
                    <Button
                        className="w-full h-12 bg-primary hover:bg-primary/90 text-primary-foreground"
                        onClick={validateAndConvert}
                        disabled={isConverting}
                    >
                        {isConverting ? (
                            <>
                                <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                                Converting...
                            </>
                        ) : (
                            <>
                                {isToPrivate ? "Make Private" : "Make Public"}
                                <ArrowRight className="w-4 h-4 ml-2" />
                            </>
                        )}
                    </Button>
                    <Button
                        variant="ghost"
                        className="w-full h-10 text-muted-foreground hover:text-foreground"
                        onClick={() => setCurrentScreen("balance")}
                        disabled={isConverting}
                    >
                        Cancel
                    </Button>
                </div>
            </div>
        </div>
    );
}
