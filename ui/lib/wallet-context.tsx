"use client";

import { createContext, useContext, useState, type ReactNode } from "react";

export type Screen = "onboarding" | "balance" | "send" | "review" | "status";
export type TransactionStatus = "pending" | "success" | "failed";

interface WalletData {
    balance: number;
    balanceUsd: number;
    address: string;
    currency: string;
}

interface TransactionData {
    recipient: string;
    amount: string;
    network: string;
    fee: number;
    txId?: string;
}

interface WalletContextType {
    currentScreen: Screen;
    setCurrentScreen: (screen: Screen) => void;
    wallet: WalletData;
    isWalletCreated: boolean;
    createWallet: () => void;
    requestAirdrop: () => void;
    transaction: TransactionData;
    setTransaction: (data: Partial<TransactionData>) => void;
    transactionStatus: TransactionStatus;
    setTransactionStatus: (status: TransactionStatus) => void;
    resetTransaction: () => void;
}

const emptyWallet: WalletData = {
    balance: 0,
    balanceUsd: 0,
    address: "",
    currency: "SOL",
};

const initialTransaction: TransactionData = {
    recipient: "",
    amount: "",
    network: "Solana Mainnet",
    fee: 0.000005,
};

function generateWalletAddress(): string {
    const chars = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let address = "";
    for (let i = 0; i < 44; i++) {
        address += chars[Math.floor(Math.random() * chars.length)];
    }
    return address;
}

function truncateAddress(address: string): string {
    if (address.length <= 12) return address;
    return `${address.slice(0, 6)}...${address.slice(-4)}`;
}

const WalletContext = createContext<WalletContextType | undefined>(undefined);

export function WalletProvider({ children }: { children: ReactNode }) {
    const [currentScreen, setCurrentScreen] = useState<Screen>("onboarding");
    const [isWalletCreated, setIsWalletCreated] = useState(false);
    const [wallet, setWallet] = useState<WalletData>(emptyWallet);
    const [transaction, setTransactionData] =
        useState<TransactionData>(initialTransaction);
    const [transactionStatus, setTransactionStatus] =
        useState<TransactionStatus>("pending");

    const createWallet = () => {
        const newAddress = generateWalletAddress();
        setWallet({
            balance: 0,
            balanceUsd: 0,
            address: newAddress,
            currency: "SOL",
        });
        setIsWalletCreated(true);
        setCurrentScreen("balance");
    };

    const requestAirdrop = () => {
        setWallet((prev) => ({
            ...prev,
            balance: prev.balance + 1.5,
            balanceUsd: prev.balanceUsd + 4500,
        }));
    };

    const setTransaction = (data: Partial<TransactionData>) => {
        setTransactionData((prev) => ({ ...prev, ...data }));
    };

    const resetTransaction = () => {
        setTransactionData(initialTransaction);
        setTransactionStatus("pending");
        setCurrentScreen("balance");
    };

    const walletDisplay: WalletData = {
        ...wallet,
        address: truncateAddress(wallet.address),
    };

    return (
        <WalletContext.Provider
            value={{
                currentScreen,
                setCurrentScreen,
                wallet: walletDisplay,
                isWalletCreated,
                createWallet,
                requestAirdrop,
                transaction,
                setTransaction,
                transactionStatus,
                setTransactionStatus,
                resetTransaction,
            }}
        >
            {children}
        </WalletContext.Provider>
    );
}

export function useWallet() {
    const context = useContext(WalletContext);
    if (context === undefined) {
        throw new Error("useWallet must be used within a WalletProvider");
    }
    return context;
}
