"use client";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useWallet, type TransferType } from "@/lib/wallet-context";
import { ChevronLeft, Wallet } from "lucide-react";
import { useState } from "react";

const BASE58_REGEX = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/;
const TELEGRAM_USERNAME_REGEX = /^@[a-zA-Z][a-zA-Z0-9_]{4,31}$/;

function detectTransferType(input: string): TransferType | null {
    const trimmed = input.trim();
    if (trimmed.startsWith("@")) {
        return TELEGRAM_USERNAME_REGEX.test(trimmed) ? "telegram" : null;
    }
    return BASE58_REGEX.test(trimmed) ? "solana" : null;
}

export function SendScreen() {
    const { wallet, setCurrentScreen, setTransaction, transaction } =
        useWallet();
    const [recipient, setRecipient] = useState(transaction.recipient);
    const [amount, setAmount] = useState(transaction.amount);
    const [errors, setErrors] = useState<{
        recipient?: string;
        amount?: string;
    }>({});

    const validateAndContinue = () => {
        const newErrors: { recipient?: string; amount?: string } = {};

        const trimmedRecipient = recipient.trim();
        if (!trimmedRecipient) {
            newErrors.recipient = "Please enter a recipient";
        } else {
            const transferType = detectTransferType(trimmedRecipient);
            if (!transferType) {
                if (trimmedRecipient.startsWith("@")) {
                    newErrors.recipient =
                        "Invalid Telegram username (must be 5-32 characters)";
                } else {
                    newErrors.recipient =
                        "Enter a valid @username or Solana address";
                }
            }
        }

        const amountNum = parseFloat(amount);
        if (!amount.trim()) {
            newErrors.amount = "Please enter an amount";
        } else if (isNaN(amountNum) || amountNum <= 0) {
            newErrors.amount = "Please enter a valid amount";
        } else if (amountNum > wallet.tgusd.total) {
            newErrors.amount = "Insufficient balance";
        }

        setErrors(newErrors);

        if (Object.keys(newErrors).length === 0) {
            const transferType = detectTransferType(trimmedRecipient)!;
            setTransaction({
                recipient: trimmedRecipient,
                amount,
                transferType,
            });
            setCurrentScreen("review");
        }
    };

    const handleMaxAmount = () => {
        setAmount(wallet.tgusd.total.toString());
    };

    return (
        <div className="flex flex-col h-full">
            {/* Header */}
            <div className="flex items-center gap-3 p-4 border-b border-border">
                <button
                    onClick={() => setCurrentScreen("balance")}
                    className="flex items-center justify-center w-8 h-8 rounded-full hover:cursor-pointer"
                >
                    <ChevronLeft className="w-5 h-5 text-foreground" />
                </button>
                <span className="font-semibold text-foreground">
                    Send tgUSD
                </span>
            </div>

            {/* Form */}
            <div className="flex-1 p-4 flex flex-col">
                <div className="space-y-5">
                    {/* Recipient */}
                    <div className="space-y-2">
                        <Label
                            htmlFor="recipient"
                            className="text-xs uppercase tracking-wider font-medium text-muted-foreground"
                        >
                            Send to
                        </Label>
                        <Input
                            id="recipient"
                            placeholder="@username or Solana address"
                            value={recipient}
                            onChange={(e) => {
                                setRecipient(e.target.value);
                                setErrors((prev) => ({
                                    ...prev,
                                    recipient: undefined,
                                }));
                            }}
                            className={`h-12 bg-secondary/50 border-border text-foreground placeholder:text-muted-foreground ${
                                errors.recipient ? "border-destructive" : ""
                            }`}
                        />
                        {errors.recipient && (
                            <p className="text-xs text-destructive">
                                {errors.recipient}
                            </p>
                        )}
                    </div>

                    {/* Amount */}
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
                                className="text-xs font-medium text-muted-foreground hover:underline hover:cursor-pointer"
                            >
                                Max: {wallet.tgusd.total} tgUSD
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
                                    setErrors((prev) => ({
                                        ...prev,
                                        amount: undefined,
                                    }));
                                }}
                                className={`h-12 pr-16 bg-secondary/50 border-border text-foreground placeholder:text-muted-foreground ${
                                    errors.amount ? "border-destructive" : ""
                                }`}
                            />
                            <div className="absolute right-3 top-1/2 -translate-y-1/2 flex items-center gap-1.5 text-muted-foreground">
                                <Wallet className="w-4 h-4" />
                                <span className="text-sm font-medium">
                                    tgUSD
                                </span>
                            </div>
                        </div>
                        {errors.amount && (
                            <p className="text-xs text-destructive">
                                {errors.amount}
                            </p>
                        )}
                    </div>
                </div>

                {/* Network Info */}
                <div className="mt-6 p-3 rounded-lg bg-secondary/30">
                    <div className="flex items-center justify-between text-sm">
                        <span className="text-muted-foreground">Network</span>
                        <span className="font-medium text-foreground">
                            Solana Surfpool
                        </span>
                    </div>
                </div>

                {/* Spacer */}
                <div className="flex-1" />

                {/* Continue Button */}
                <Button
                    className="w-full h-12 bg-primary hover:bg-primary/90 text-primary-foreground mt-6 hover:cursor-pointer"
                    onClick={validateAndContinue}
                >
                    Review
                </Button>
            </div>
        </div>
    );
}
