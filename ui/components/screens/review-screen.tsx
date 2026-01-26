"use client";

import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
    Tooltip,
    TooltipContent,
    TooltipTrigger,
} from "@/components/ui/tooltip";
import { openAddressInExplorer } from "@/lib/explorer";
import { truncateString } from "@/lib/utils";
import { useWallet } from "@/lib/wallet-context";
import {
    AlertCircle,
    ArrowRight,
    ChevronLeft,
    ExternalLink,
    User,
} from "lucide-react";
import { useState } from "react";

export function ReviewScreen() {
    const {
        transfer,
        transferByTelegram,
        transaction,
        setTransactionMessage,
        setCurrentScreen,
        setTransaction,
        setTransactionStatus,
        refreshBalance,
    } = useWallet();

    const [isConfirming, setIsConfirming] = useState(false);
    const [transferError, setTransferError] = useState<string | null>(null);
    const [copied, setCopied] = useState(false);

    const handleCopy = async () => {
        await navigator.clipboard.writeText(transaction.recipient);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
    };

    const handleConfirm = async () => {
        setIsConfirming(true);
        setTransferError(null);

        try {
            const adjustedAmount =
                Number.parseFloat(transaction.amount) * 10 ** 9;
            
            let signatures;
            if (transaction.transferType === "telegram") {
                const username = transaction.recipient.replace(/^@/, "");
                signatures = await transferByTelegram(
                    username,
                    adjustedAmount.toString(),
                );
            } else {
                signatures = await transfer(
                    transaction.recipient,
                    adjustedAmount.toString(),
                );
            }

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
            const message =
                error instanceof Error
                    ? error.message
                    : "Failed to transfer tokens";
            const formattedMessage = message.replace(
                /API error \(\d+\):\s*/u,
                "",
            );

            setTransferError(formattedMessage);
            setTransactionStatus("failed");
            setTransactionMessage(formattedMessage);
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
                    className={`flex items-center justify-center w-8 h-8 rounded-full ${isConfirming ? "opacity-50 cursor-not-allowed" : ""}`}
                    disabled={isConfirming}
                >
                    <ChevronLeft className="w-5 h-5 text-foreground" />
                </button>
                <span className="font-semibold text-foreground">
                    Review & Confirm
                </span>
            </div>

            {/* Warning */}
            <div className="px-4 py-2 flex flex-col">
                <div className="p-3 rounded-lg bg-secondary/50 flex items-start gap-2">
                    <AlertCircle className="w-4 h-4 text-muted-foreground mt-0.5 shrink-0" />
                    <p className="text-xs text-muted-foreground leading-relaxed">
                        Please verify the recipient address. Transactions are
                        irreversible.
                    </p>
                </div>
            </div>

            {/* Content */}
            <div className="flex-1 px-4 flex flex-col">
                {/* <div className="text-center py-2">
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
                </div> */}

                {/* Transaction Details */}
                <Card className="bg-card border-border shadow-sm py-0">
                    <CardContent className="p-0 divide-y divide-border">
                        {/* Amount */}
                        <div className="p-4">
                            <p className="text-xs text-muted-foreground uppercase tracking-wider mb-1">
                                Amount
                            </p>
                            <p className="text-sm font-medium text-foreground">
                                {transaction.amount} cUSD
                            </p>
                        </div>

                        {/* To */}
                        <div className="p-4">
                            <p className="text-xs text-muted-foreground uppercase tracking-wider mb-1">
                                To
                            </p>
                            <div className="flex items-center gap-2">
                                {transaction.transferType === "telegram" ? (
                                    <div className="flex-1 flex items-center gap-2">
                                        <User className="w-4 h-4 text-muted-foreground" />
                                        <span className="text-sm font-medium text-foreground">
                                            {transaction.recipient}
                                        </span>
                                    </div>
                                ) : (
                                    <>
                                        <button className="flex-1 flex items-center justify-between rounded-lg">
                                            <span className="font-mono text-sm text-foreground">
                                                {truncateString(transaction.recipient)}
                                            </span>
                                        </button>
                                        <Tooltip>
                                            <TooltipTrigger asChild>
                                                <Button
                                                    variant="outline"
                                                    size="icon"
                                                    className="h-[32px] w-[32px] shrink-0 hover:cursor-pointer"
                                                    onClick={() =>
                                                        openAddressInExplorer(
                                                            transaction.recipient,
                                                        )
                                                    }
                                                >
                                                    <ExternalLink className="w-4 h-4" />
                                                </Button>
                                            </TooltipTrigger>
                                            <TooltipContent>
                                                View on Explorer
                                            </TooltipContent>
                                        </Tooltip>
                                    </>
                                )}
                            </div>
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
                    </CardContent>
                </Card>

                {/* Error */}
                <div className={`${transferError ? "mt-4 space-y-2" : ""}`}>
                    {transferError ? (
                        <div className="p-3 rounded-lg bg-destructive/10 flex items-start gap-2">
                            <AlertCircle className="w-4 h-4 text-destructive mt-0.5 shrink-0" />
                            <p className="text-xs text-destructive leading-relaxed">
                                {transferError}
                            </p>
                        </div>
                    ) : null}
                </div>

                {/* Spacer */}
                <div className="flex-1" />

                {/* Actions */}
                <div className="space-y-3 my-2">
                    <Button
                        className="w-full h-12 bg-primary hover:bg-primary/80 text-primary-foreground hover:cursor-pointer"
                        onClick={handleConfirm}
                        disabled={isConfirming || transferError !== null}
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
                        className={`w-full h-10 bg-transparent text-foreground hover:bg-transparent hover:cursor-pointer hover:border hover:border-[#ececec] hover:border-opacity-50 ${isConfirming ? "opacity-50 cursor-not-allowed" : ""}`}
                        onClick={() => setCurrentScreen("send")}
                        disabled={isConfirming}
                    >
                        Cancel
                    </Button>
                </div>
            </div>
        </div>
    );
}
