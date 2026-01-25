"use client";

import {
    createContext,
    useContext,
    useEffect,
    useState,
    type ReactNode,
} from "react";

import { useSimpleAuth } from "@/lib/auth-context-simple";

export type Screen =
    | "onboarding"
    | "balance"
    | "send"
    | "review"
    | "status"
    | "convert";
export type ConversionDirection = "toPrivate" | "toPublic";
export type TransactionStatus = "pending" | "success" | "failed";

interface TokenBalance {
    private: number;
    public: number;
    total: number;
}

interface WalletData {
    address: string;
    solBalance: number;
    cusd: TokenBalance;
}

export interface TransactionStep {
    label: string;
    description?: string;
    txId: string;
}

interface TransactionData {
    recipient: string;
    amount: string;
    network: string;
    fee: number;
    txId?: string;
    steps?: TransactionStep[];
}

interface ConversionData {
    direction: ConversionDirection;
    amount: string;
}

interface WalletContextType {
    currentScreen: Screen;
    setCurrentScreen: (screen: Screen) => void;
    wallet: WalletData;
    isLoading: boolean;
    isWalletCreated: boolean;
    createWallet: () => Promise<void>;
    requestAirdrop: () => Promise<void>;
    refreshBalance: () => Promise<void>;
    mint: () => Promise<string>;
    transfer: (
        recipient: string,
        amount: string,
    ) => Promise<TransactionResult[]>;
    transaction: TransactionData;
    setTransaction: (data: Partial<TransactionData>) => void;
    transactionMessage: string;
    setTransactionMessage: (message: string) => void;
    transactionStatus: TransactionStatus;
    setTransactionStatus: (status: TransactionStatus) => void;
    resetTransaction: () => void;
    conversion: ConversionData;
    setConversion: (data: Partial<ConversionData>) => void;
    startConversion: (direction: ConversionDirection) => void;
    executeConversion: (amount: string) => Promise<TransactionResult[]>;
}

const emptyWallet: WalletData = {
    address: "",
    solBalance: 0,
    cusd: {
        private: 0,
        public: 0,
        total: 0,
    },
};

const initialTransaction: TransactionData = {
    recipient: "",
    amount: "",
    network: "Solana Surfpool",
    fee: 0.000005,
};

const initialConversion: ConversionData = {
    direction: "toPrivate",
    amount: "",
};

function generateWalletAddress(): string {
    const chars = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let address = "";
    for (let i = 0; i < 44; i++) {
        address += chars[Math.floor(Math.random() * chars.length)];
    }
    return address;
}

const WalletContext = createContext<WalletContextType | undefined>(undefined);

export type ApiResponse<T> = {
    data: T;
};

export type CreateWalletResponse = {
    pubkey: string;
};

export type AirdropResponse = {
    signature: string;
    amount: string;
};

export type EncryptedBalance = {
    pending: string;
    available: string;
};

export type BalanceResponse = {
    owner: string;
    mint: string;
    tokenAccount: string;
    publicBalance: string;
    encryptedBalance: EncryptedBalance;
};

export type SolanaBalanceResponse = {
    lamports: string;
};

export type ListWalletsResponse = {
    pubkeys: string[];
};

export type TransactionResult = {
    label: string;
    signature: string;
};

export type MintResponse = {
    mint: string;
    signature: string;
};

export type TransferResponse = {
    transactions: TransactionResult[];
};

export function WalletProvider({ children }: { children: ReactNode }) {
    const [currentScreen, setCurrentScreen] = useState<Screen>("onboarding");
    const [isLoading, setIsLoading] = useState(true);
    const [isWalletCreated, setIsWalletCreated] = useState(false);
    const [wallet, setWallet] = useState<WalletData>(emptyWallet);
    const [transaction, setTransactionData] =
        useState<TransactionData>(initialTransaction);
    const [transactionStatus, setTransactionStatus] =
        useState<TransactionStatus>("pending");
    const [transactionMessage, setTransactionMessage] = useState<string>("");
    const [conversion, setConversionData] =
        useState<ConversionData>(initialConversion);

    const { token, status } = useSimpleAuth();

    // Simple authenticated fetch - always check localStorage as fallback
    const authFetch = async <T,>(path: string, init?: RequestInit): Promise<T> => {
        // Get token from context or localStorage (context might be stale)
        const currentToken = token ?? localStorage.getItem("tg_auth_token");
        
        if (!currentToken) {
            throw new Error("Not authenticated");
        }
        
        const res = await fetch(path, {
            ...init,
            headers: {
                ...init?.headers,
                "Content-Type": "application/json",
                Authorization: `Bearer ${currentToken}`,
            },
        });
        
        if (!res.ok) {
            const text = await res.text().catch(() => "");
            throw new Error(`API error (${res.status}): ${text}`);
        }
        
        return res.json();
    };

    const fetchBalances = async (
        address: string,
    ): Promise<{ solBalance: number; cusd: TokenBalance }> => {
        let solBalance = 0;
        let cusd: TokenBalance = { private: 0, public: 0, total: 0 };

        try {
            const [solResponse, tokenResponse] = await Promise.allSettled([
                authFetch<ApiResponse<SolanaBalanceResponse>>(
                    `/api/wallets/${address}/balance/solana`,
                    {
                        method: "GET",
                        headers: { "Content-Type": "application/json" },
                    },
                ),
                authFetch<ApiResponse<BalanceResponse>>(
                    `/api/wallets/${address}/balance?mint=${process.env.NEXT_PUBLIC_CUSD_MINT}`,
                    {
                        method: "GET",
                        headers: { "Content-Type": "application/json" },
                    },
                ).catch((error) => {
                    // console.warn("Failed to fetch token balance:", error);
                    return {
                        data: {
                            publicBalance: "0",
                            encryptedBalance: { pending: "0", available: "0" },
                        },
                    };
                }),
            ]);

            if (solResponse.status === "fulfilled") {
                solBalance =
                    Number.parseFloat(solResponse.value.data.lamports) /
                    10 ** 9;
            }

            if (tokenResponse.status === "fulfilled") {
                const publicBalance =
                    Number.parseFloat(
                        tokenResponse.value.data?.publicBalance ?? "0",
                    ) /
                    10 ** 9;
                const privateBalance =
                    Number.parseFloat(
                        tokenResponse.value.data?.encryptedBalance.available ??
                            "0",
                    ) +
                    Number.parseFloat(
                        tokenResponse.value.data?.encryptedBalance.pending ??
                            "0",
                    );
                const availableBalance = privateBalance / 10 ** 9;
                cusd = {
                    public: publicBalance,
                    private: availableBalance,
                    total: publicBalance + availableBalance,
                };
            }
        } catch (error) {
            console.error("Failed to fetch balances:", error);
        }

        return { solBalance, cusd };
    };

    useEffect(() => {
        const checkExistingWallet = async () => {
            if (status !== "authenticated") return;

            try {
                const response = await authFetch<
                    ApiResponse<ListWalletsResponse>
                >("/api/wallets", { method: "GET" });

                if (response.data.pubkeys.length > 0) {
                    const address = response.data.pubkeys[0];
                    const { solBalance, cusd } = await fetchBalances(address);

                    setWallet({
                        address,
                        solBalance,
                        cusd,
                    });
                    setIsWalletCreated(true);
                    setCurrentScreen("balance");
                }
            } catch (error) {
                console.error("Failed to check existing wallets:", error);
            } finally {
                setIsLoading(false);
            }
        };

        checkExistingWallet();
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [status]);

    const createWallet = async () => {
        console.log("[WALLET] createWallet called, token:", !!token);
        try {
            const response = await authFetch<ApiResponse<CreateWalletResponse>>(
                "/api/wallets",
                {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                },
            );
            console.log("[WALLET] Wallet created:", response);

            const address = response.data.pubkey;
            setWallet({
                address,
                solBalance: 0,
                cusd: { private: 0, public: 0, total: 0 },
            });
            setIsWalletCreated(true);
            setCurrentScreen("balance");
        } catch (error) {
            console.error("[WALLET] createWallet error:", error);
            throw error;
        }
    };

    const requestAirdrop = async () => {
        console.log("Requesting airdrop for wallet", wallet.address);
        const response = await authFetch<ApiResponse<AirdropResponse>>(
            `/api/wallets/${wallet.address}/airdrop`,
            {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ amount: "1" }),
            },
        );

        const airdropAmount = Number.parseFloat(response.data.amount);
        setWallet((prev) => ({
            ...prev,
            solBalance: prev.solBalance + airdropAmount,
        }));
    };

    const refreshBalance = async () => {
        console.log("Refreshing balance for wallet", wallet.address);
        const { solBalance, cusd } = await fetchBalances(wallet.address);
        console.log("Refreshed balance", { solBalance, cusd });
        setWallet((prev) => ({
            ...prev,
            solBalance,
            cusd,
        }));
    };

    const mint = async () => {
        const mint = process.env.NEXT_PUBLIC_CUSD_MINT;
        console.log(`Minting ${mint} for wallet=${wallet.address}`);
        const response = await authFetch<ApiResponse<MintResponse>>(
            `/api/tokens/${wallet.address}/mint`,
            {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ mint, amount: "1.0" }),
            },
        );

        return response.data.signature;
    };

    // TODO: set failed if actually fails?
    const transfer = async (recipient: string, amount: string) => {
        const response = await authFetch<ApiResponse<TransferResponse>>(
            `/api/transfers`,
            {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    source: wallet.address,
                    recipient,
                    mint: process.env.NEXT_PUBLIC_CUSD_MINT,
                    amount,
                }),
            },
        );

        console.log("Transfer response", response.data);

        return response.data.transactions;
    };

    const setTransaction = (data: Partial<TransactionData>) => {
        setTransactionData((prev) => ({ ...prev, ...data }));
    };

    const resetTransaction = () => {
        setTransactionData(initialTransaction);
        setTransactionStatus("pending");
        setCurrentScreen("balance");
    };

    const setConversion = (data: Partial<ConversionData>) => {
        setConversionData((prev) => ({ ...prev, ...data }));
    };

    const startConversion = (direction: ConversionDirection) => {
        setConversionData({ direction, amount: "" });
        setCurrentScreen("convert");
    };

    type WithdrawResponse = {
        transactions: TransactionResult[];
    };

    type DepositResponse = {
        transactions: TransactionResult[];
    };

    const executeConversion = async (
        amount: string,
    ): Promise<TransactionResult[]> => {
        const mint = process.env.NEXT_PUBLIC_CUSD_MINT;
        const amountValue = Number.parseFloat(amount);
        if (!Number.isFinite(amountValue) || amountValue <= 0) {
            throw new Error("Conversion amount must be a positive number.");
        }
        const amountInBaseUnits = Math.floor(amountValue * 10 ** 9);

        if (conversion.direction === "toPublic") {
            // Withdraw: private -> public
            const response = await authFetch<ApiResponse<WithdrawResponse>>(
                `/api/wallets/${wallet.address}/withdraw`,
                {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({
                        mint,
                        amount: amountInBaseUnits.toString(),
                        decimals: 9,
                    }),
                },
            );
            console.log("Withdraw response", response.data.transactions);
            return response.data.transactions;
        } else {
            // Deposit: public -> private
            const response = await authFetch<ApiResponse<DepositResponse>>(
                `/api/wallets/${wallet.address}/deposit`,
                {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({
                        mint,
                        amount: amountInBaseUnits.toString(),
                        decimals: 9,
                    }),
                },
            );
            console.log("Withdraw response", response);
            return response.data.transactions;
        }
    };

    return (
        <WalletContext.Provider
            value={{
                currentScreen,
                setCurrentScreen,
                wallet,
                isLoading,
                isWalletCreated,
                mint,
                transfer,
                createWallet,
                requestAirdrop,
                refreshBalance,
                transaction,
                setTransaction,
                transactionMessage,
                setTransactionMessage,
                transactionStatus,
                setTransactionStatus,
                resetTransaction,
                conversion,
                setConversion,
                startConversion,
                executeConversion,
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
