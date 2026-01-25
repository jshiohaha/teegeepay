"use client";

import { Button } from "@/components/ui/button";
import { openTransactionInExplorer } from "@/lib/explorer";
import { truncateString } from "@/lib/utils";
import { useWallet, type TransactionStep } from "@/lib/wallet-context";
import { Check, CheckCircle2, Copy, ExternalLink, XCircle } from "lucide-react";
import { useEffect, useState } from "react";

function TransactionItem({
    step,
    index,
}: {
    step: TransactionStep;
    index: number;
}) {
    const [copied, setCopied] = useState(false);

    const handleCopy = async () => {
        await navigator.clipboard.writeText(step.txId);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
    };

    return (
        <div className="flex items-start gap-3 p-3 rounded-lg bg-secondary/30">
            <div className="flex items-center justify-center w-5 h-5 rounded-full bg-success/20 text-success shrink-0 mt-0.5">
                <Check className="w-3 h-3" />
            </div>
            <div className="flex-1 min-w-0">
                <div className="flex items-center justify-between gap-2">
                    <span className="text-sm font-medium text-foreground">
                        {step.label}
                    </span>
                    <span className="text-xs text-muted-foreground">
                        {index + 1}
                    </span>
                </div>
                {step.description && (
                    <p className="text-xs text-muted-foreground mt-0.5">
                        {step.description}
                    </p>
                )}
                <div className="flex items-center gap-2 mt-2">
                    <button
                        onClick={handleCopy}
                        className="flex items-center gap-1.5 px-2 py-1 rounded bg-secondary/50 hover:bg-secondary transition-colors"
                    >
                        <span className="font-mono text-xs text-muted-foreground">
                            {truncateString(step.txId, 8, 6)}
                        </span>
                        {copied ? (
                            <Check className="w-3 h-3 text-success" />
                        ) : (
                            <Copy className="w-3 h-3 text-muted-foreground" />
                        )}
                    </button>
                    <button
                        onClick={() => openTransactionInExplorer(step.txId)}
                        className="p-1 rounded hover:bg-secondary/50 transition-colors"
                    >
                        <ExternalLink className="w-3 h-3 text-muted-foreground" />
                    </button>
                </div>
            </div>
        </div>
    );
}

export function StatusScreen() {
    const {
        transaction,
        transactionMessage,
        transactionStatus,
        resetTransaction,
        setCurrentScreen,
    } = useWallet();
    const [copied, setCopied] = useState(false);

    const isSuccess = transactionStatus === "success";
    const hasSteps = transaction.steps && transaction.steps.length > 0;
    const hasSingleTx = !hasSteps && transaction.txId;

    useEffect(() => {
        if (!transaction.txId && !hasSteps && transactionStatus === "pending") {
            setCurrentScreen("balance");
        }
    }, []);

    const handleCopyTxId = async () => {
        if (transaction.txId) {
            await navigator.clipboard.writeText(transaction.txId);
            setCopied(true);
            setTimeout(() => setCopied(false), 2000);
        }
    };

    return (
        <div className="flex flex-col h-full">
            {/* Header */}
            <div className="flex items-center justify-center p-4 border-b border-border">
                <span className="font-semibold text-foreground">
                    {isSuccess ? "Success!" : "Transaction Failed"}
                </span>
            </div>

            {/* Content */}
            <div className="flex-1 p-4 flex flex-col items-center overflow-y-auto">
                {/* Status Icon */}
                <div
                    className={`flex items-center justify-center w-16 h-16 rounded-full mb-4`}
                >
                    {isSuccess ? (
                        <CheckCircle2 className="w-8 h-8 text-success" />
                    ) : (
                        <XCircle className="w-8 h-8 text-destructive" />
                    )}
                </div>

                {/* Message */}
                <h2 className="text-lg font-semibold text-foreground mb-1 text-center">
                    {isSuccess ? transactionMessage : "Something went wrong."}
                </h2>
                {!isSuccess && (
                    <p className="text-sm text-destructive text-center max-w-[260px]">
                        Please try again or contact support if the issue
                        persists.
                    </p>
                )}

                {/* Multiple Transaction Steps */}
                {isSuccess && hasSteps && (
                    <div className="mt-6 w-full space-y-2">
                        <p className="text-xs text-muted-foreground uppercase tracking-wider mb-3">
                            Transactions ({transaction.steps!.length})
                        </p>
                        {transaction.steps!.map((step, index) => (
                            <TransactionItem
                                key={step.txId}
                                step={step}
                                index={index}
                            />
                        ))}
                    </div>
                )}

                {/* Single Transaction ID (fallback) */}
                {isSuccess && hasSingleTx && (
                    <div className="mt-6 w-full">
                        <p className="text-xs text-muted-foreground uppercase tracking-wider mb-2 text-center">
                            Transaction ID
                        </p>
                        <div className="flex items-center justify-center gap-2">
                            <button
                                onClick={handleCopyTxId}
                                className="flex items-center gap-2 px-3 py-2 rounded-lg bg-secondary/50 hover:bg-secondary transition-colors"
                            >
                                <span className="font-mono text-sm text-foreground">
                                    {truncateString(transaction.txId!)}
                                </span>
                                {copied ? (
                                    <Check className="w-3.5 h-3.5 text-success" />
                                ) : (
                                    <Copy className="w-3.5 h-3.5 text-muted-foreground" />
                                )}
                            </button>
                        </div>
                        <button
                            onClick={() =>
                                openTransactionInExplorer(transaction.txId!)
                            }
                            className="flex items-center justify-center gap-1.5 mt-3 w-full text-sm text-muted-foreground hover:text-foreground transition-colors"
                        >
                            <ExternalLink className="w-4 h-4" />
                            <span>View on Solscan</span>
                        </button>
                    </div>
                )}
            </div>

            {/* Done Button */}
            <div className="p-4 border-t border-border">
                <Button
                    className="w-full h-12 bg-primary hover:bg-primary/90 text-primary-foreground"
                    onClick={resetTransaction}
                >
                    Done
                </Button>
            </div>
        </div>
    );
}
