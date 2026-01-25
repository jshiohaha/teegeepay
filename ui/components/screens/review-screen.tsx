"use client";

import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { truncateString } from "@/lib/utils";
import { useWallet } from "@/lib/wallet-context";
import { AlertCircle, ArrowRight, ChevronLeft } from "lucide-react";
import { useState } from "react";

export function ReviewScreen() {
    const {
        transfer,
        transaction,
        setTransactionMessage,
        setCurrentScreen,
        setTransaction,
        setTransactionStatus,
        refreshBalance,
    } = useWallet();

    const [isConfirming, setIsConfirming] = useState(false);

    const handleConfirm = async () => {
        setIsConfirming(true);

        try {
            const adjustedAmount =
                Number.parseFloat(transaction.amount) * 10 ** 9;
            const signatures = await transfer(
                transaction.recipient,
                adjustedAmount.toString(),
            );

            if (signatures.length === 0) {
                setTransactionStatus("failed");
                setTransactionMessage("Failed to transfer tokens");
                return;
            }

            await refreshBalance();

            setTransaction({
                steps: signatures.map((signature) => ({
                    label: signature.label,
                    txId: signature.signature,
                })),
            });
            setTransactionStatus("success");
            setCurrentScreen("status");
            setTransactionMessage("Your transfer is complete.");
        } catch (error) {
            console.error("Failed to transfer tokens", error);
            setTransactionStatus("failed");
            setTransactionMessage("Failed to transfer tokens");
            return;
        } finally {
            setIsConfirming(false);
        }
    };

    return (
        <div className="flex flex-col h-full">
            {/* Header */}
            <div className="flex items-center gap-3 p-4 border-b border-border">
                <button
                    onClick={() => setCurrentScreen("send")}
                    className="flex items-center justify-center w-8 h-8 rounded-full hover:bg-secondary transition-colors"
                >
                    <ChevronLeft className="w-5 h-5 text-foreground" />
                </button>
                <span className="font-semibold text-foreground">
                    Review & Confirm
                </span>
            </div>

            {/* Content */}
            <div className="flex-1 p-4 flex flex-col">
                {/* Amount Display */}
                <div className="text-center py-6">
                    <p className="text-sm text-muted-foreground mb-2">
                        You are sending
                    </p>
                    <div className="flex items-baseline justify-center gap-2">
                        <span className="text-4xl font-bold tracking-tight text-foreground">
                            {transaction.amount}
                        </span>
                        <span className="text-lg font-medium text-muted-foreground">
                            cUSD
                        </span>
                    </div>
                </div>

                {/* Transaction Details */}
                <Card className="bg-card border-border shadow-sm py-0">
                    <CardContent className="p-0 divide-y divide-border">
                        {/* To */}
                        <div className="p-4">
                            <p className="text-xs text-muted-foreground uppercase tracking-wider mb-1">
                                To
                            </p>
                            <p className="font-mono text-sm text-foreground">
                                {truncateString(transaction.recipient)}
                            </p>
                        </div>

                        {/* Network */}
                        <div className="p-4">
                            <p className="text-xs text-muted-foreground uppercase tracking-wider mb-1">
                                Network
                            </p>
                            <p className="text-sm font-medium text-foreground">
                                {transaction.network}
                            </p>
                        </div>

                        {/* <div className="p-4">
                            <p className="text-xs text-muted-foreground uppercase tracking-wider mb-1">
                                Estimated Fee
                            </p>
                            <p className="text-sm font-medium text-foreground">
                                ~${transaction.fee.toFixed(2)}
                            </p>
                        </div> */}
                    </CardContent>
                </Card>

                {/* Warning */}
                <div className="mt-4 p-3 rounded-lg bg-secondary/50 flex items-start gap-2">
                    <AlertCircle className="w-4 h-4 text-muted-foreground mt-0.5 shrink-0" />
                    <p className="text-xs text-muted-foreground leading-relaxed">
                        Please verify the recipient address. Transactions are
                        irreversible.
                    </p>
                </div>

                {/* Spacer */}
                <div className="flex-1" />

                {/* Actions */}
                <div className="space-y-3 mt-6">
                    <Button
                        className="w-full h-12 bg-primary hover:bg-primary/90 text-primary-foreground"
                        onClick={handleConfirm}
                        disabled={isConfirming}
                    >
                        <span>
                            {isConfirming ? (
                                "Confirming..."
                            ) : (
                                <div className="flex items-center justify-center">
                                    <span>Confirm Transfer</span>
                                    <ArrowRight className="w-4 h-4 ml-2" />
                                </div>
                            )}
                        </span>
                    </Button>
                    <Button
                        variant="ghost"
                        className="w-full h-10 text-muted-foreground hover:text-foreground"
                        onClick={() => setCurrentScreen("send")}
                    >
                        Cancel
                    </Button>
                </div>
            </div>
        </div>
    );
}
