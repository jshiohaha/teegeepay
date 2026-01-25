"use client";

import { Badge } from "@/components/ui/badge";
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
    ArrowUpRight,
    Check,
    Coins,
    Copy,
    ExternalLink,
    Eye,
    EyeOff,
    RefreshCw,
    Wallet,
} from "lucide-react";
import { useState } from "react";

export function BalanceScreen() {
    const {
        wallet,
        setCurrentScreen,
        requestAirdrop,
        refreshBalance,
        mint,
        setTransaction,
        setTransactionStatus,
        setTransactionMessage,
        startConversion,
    } = useWallet();
    const [copied, setCopied] = useState(false);
    const [isRefreshing, setIsRefreshing] = useState(false);
    const [isRequestingAirdrop, setIsRequestingAirdrop] = useState(false);
    const [isMinting, setIsMinting] = useState(false);

    const handleCopy = async () => {
        await navigator.clipboard.writeText(wallet.address);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
    };

    const handleRefresh = async () => {
        setIsRefreshing(true);

        try {
            await refreshBalance();
        } catch (error) {
            console.error("Error refreshing balance", error);
        } finally {
            setIsRefreshing(false);
        }
    };

    const handleRequestAirdrop = async () => {
        setIsRequestingAirdrop(true);

        try {
            await requestAirdrop();
            setIsRequestingAirdrop(false);
        } catch (error) {
            console.error("Error requesting airdrop", error);
        } finally {
            setIsRequestingAirdrop(false);
        }
    };

    const handleMint = async () => {
        setIsMinting(true);

        try {
            const signature = await mint();
            setTransaction({ txId: signature });
            setTransactionStatus("success");
            setCurrentScreen("status");
            setTransactionMessage("Your mint is complete.");

            await refreshBalance();
        } catch (error) {
            console.error("Error minting", error);
        } finally {
            setIsMinting(false);
        }
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
            <div className="flex items-center justify-between p-4 border-b border-border">
                <div className="flex items-center gap-2">
                    <div className="flex items-center justify-center w-8 h-8 rounded-full bg-primary">
                        <Wallet className="w-4 h-4 text-primary-foreground" />
                    </div>
                    <span className="font-semibold text-foreground">
                        My Wallet
                    </span>
                </div>
            </div>

            <div className="flex-1 p-4 flex flex-col gap-4 overflow-y-auto">
                {/* Address */}
                <div>
                    <p className="text-xs text-muted-foreground mb-2 uppercase tracking-wider font-medium">
                        Wallet Address
                    </p>
                    <div className="flex items-center gap-2">
                        <button
                            onClick={handleCopy}
                            className="flex-1 flex items-center justify-between p-3 rounded-lg bg-secondary/50 hover:bg-secondary transition-colors"
                        >
                            <span className="font-mono text-sm text-foreground">
                                {truncateString(wallet.address)}
                            </span>
                            {copied ? (
                                <Check className="w-4 h-4 text-green-500" />
                            ) : (
                                <Copy className="w-4 h-4 text-muted-foreground" />
                            )}
                        </button>
                        <Tooltip>
                            <TooltipTrigger asChild>
                                <Button
                                    variant="outline"
                                    size="icon"
                                    className="h-[46px] w-[46px] shrink-0"
                                    onClick={() =>
                                        openAddressInExplorer(wallet.address)
                                    }
                                >
                                    <ExternalLink className="w-4 h-4" />
                                </Button>
                            </TooltipTrigger>
                            <TooltipContent>
                                View on Solana Explorer
                            </TooltipContent>
                        </Tooltip>
                    </div>
                </div>

                {/* cUSD Balance Card - Primary */}
                <Card className="bg-card border-border shadow-sm py-2">
                    <CardContent className="p-5">
                        <div className="flex items-center justify-between mb-3">
                            <div className="flex items-center gap-2">
                                <span className="text-sm font-medium text-muted-foreground">
                                    cUSD Balance
                                </span>
                                <Tooltip>
                                    <TooltipTrigger asChild>
                                        <button
                                            onClick={handleRefresh}
                                            disabled={isRefreshing || isMinting}
                                            className="p-1 rounded-md hover:bg-secondary/50 transition-colors disabled:opacity-50"
                                        >
                                            <RefreshCw
                                                className={`w-3.5 h-3.5 text-muted-foreground ${isRefreshing ? "animate-spin" : ""}`}
                                            />
                                        </button>
                                    </TooltipTrigger>
                                    <TooltipContent>
                                        Refresh balance
                                    </TooltipContent>
                                </Tooltip>
                            </div>
                            <Badge variant="secondary" className="text-xs">
                                Confidential
                            </Badge>
                        </div>

                        {/* Total Balance */}
                        <div className="flex items-baseline gap-2 mb-4">
                            <span className="text-4xl font-bold tracking-tight text-card-foreground">
                                {formatBalance(wallet.cusd.total)}
                            </span>
                            <span className="text-lg font-medium text-muted-foreground">
                                cUSD
                            </span>
                        </div>

                        {/* Private/Public Breakdown */}
                        <div className="grid grid-cols-2 gap-3">
                            <div className="flex flex-col gap-2">
                                <div className="flex items-center gap-2 p-3 rounded-lg bg-secondary/50">
                                    <EyeOff className="w-4 h-4 text-muted-foreground" />
                                    <div className="flex flex-col">
                                        <span className="text-xs text-muted-foreground">
                                            Private
                                        </span>
                                        <span className="text-sm font-semibold text-foreground">
                                            {formatBalance(wallet.cusd.private)}
                                        </span>
                                    </div>
                                </div>
                                <button
                                    onClick={() => startConversion("toPublic")}
                                    disabled={wallet.cusd.private === 0}
                                    className="flex gap-1.5 px-1 text-xs font-medium text-muted-foreground hover:text-foreground hover:bg-secondary/50 rounded-md transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
                                >
                                    Make Public
                                </button>
                            </div>
                            <div className="flex flex-col gap-2">
                                <div className="flex items-center gap-2 p-3 rounded-lg bg-secondary/50">
                                    <Eye className="w-4 h-4 text-muted-foreground" />
                                    <div className="flex flex-col">
                                        <span className="text-xs text-muted-foreground">
                                            Public
                                        </span>
                                        <span className="text-sm font-semibold text-foreground">
                                            {formatBalance(wallet.cusd.public)}
                                        </span>
                                    </div>
                                </div>
                                <button
                                    onClick={() => startConversion("toPrivate")}
                                    disabled={wallet.cusd.public === 0}
                                    className="flex gap-1.5 px-1 text-xs font-medium text-muted-foreground hover:text-foreground rounded-md transition-colors disabled:opacity-40 disabled:cursor-not-allowed hover:cursor-pointer"
                                >
                                    Make Private
                                </button>
                            </div>
                        </div>
                    </CardContent>
                </Card>

                {/* SOL Balance - Secondary */}
                <div className="flex items-center justify-between p-3 rounded-lg bg-secondary/30 border border-border">
                    <div className="flex items-center gap-2">
                        <img
                            className="w-6 h-6 rounded-full object-cover"
                            src="https://wsrv.nl/?w=24&h=24&url=https%3A%2F%2Fraw.githubusercontent.com%2Fsolana-labs%2Ftoken-list%2Fmain%2Fassets%2Fmainnet%2FSo11111111111111111111111111111111111111112%2Flogo.png&dpr=2&quality=80"
                            alt="SOL"
                            width={24}
                            height={24}
                            draggable={false}
                        />
                        <span className="text-sm text-muted-foreground">
                            SOL
                        </span>
                    </div>
                    <span className="text-sm font-medium text-foreground">
                        {formatBalance(wallet.solBalance, 4)}
                    </span>
                </div>

                {/* Spacer */}
                <div className="flex-1" />

                {/* Actions */}
                <div className="flex gap-3">
                    <Button
                        variant="outline"
                        className="flex-1 h-12 bg-transparent"
                        onClick={handleMint}
                        disabled={isMinting}
                    >
                        {isMinting ? (
                            <span className="flex items-center gap-2">
                                <span className="w-4 h-4 border-2 border-foreground/30 border-t-foreground rounded-full animate-spin" />
                                Minting...
                            </span>
                        ) : (
                            <>
                                <Coins className="w-4 h-4 mr-2" />
                                Mint cUSD
                            </>
                        )}
                    </Button>
                    <Button
                        className="flex-1 h-12 bg-primary hover:bg-primary/90 text-primary-foreground"
                        onClick={() => setCurrentScreen("send")}
                    >
                        <ArrowUpRight className="w-4 h-4 mr-2" />
                        Send cUSD
                    </Button>
                </div>
            </div>
        </div>
    );
}
