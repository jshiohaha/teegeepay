const EXPLORER_BASE_URL = "https://explorer.solana.com";

export const openAddressInExplorer = (address: string) => {
    window.open(
        `${EXPLORER_BASE_URL}/address/${address}?cluster=custom&customUrl=${process.env.NEXT_PUBLIC_CLUSTER_URL}`,
        "_blank",
    );
};

export const openTransactionInExplorer = (transaction: string) => {
    window.open(
        `${EXPLORER_BASE_URL}/tx/${transaction}?cluster=custom&customUrl=${process.env.NEXT_PUBLIC_CLUSTER_URL}`,
        "_blank",
    );
};
