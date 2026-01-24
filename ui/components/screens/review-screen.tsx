"use client"

import { ChevronLeft, ArrowRight, AlertCircle } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { useWallet } from "@/lib/wallet-context"

export function ReviewScreen() {
  const {
    wallet,
    transaction,
    setCurrentScreen,
    setTransaction,
    setTransactionStatus,
  } = useWallet()

  const handleConfirm = () => {
    // Simulate transaction - Solana signatures are base58 encoded, ~88 chars
    const chars = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    let sig = ""
    for (let i = 0; i < 88; i++) {
      sig += chars[Math.floor(Math.random() * chars.length)]
    }
    const txId = `${sig.slice(0, 8)}...${sig.slice(-4)}`
    setTransaction({ txId })
    setTransactionStatus("success")
    setCurrentScreen("status")
  }

  const truncateAddress = (address: string) => {
    if (address.length <= 13) return address
    return `${address.slice(0, 6)}...${address.slice(-4)}`
  }

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
        <span className="font-semibold text-foreground">Review & Confirm</span>
      </div>

      {/* Content */}
      <div className="flex-1 p-4 flex flex-col">
        {/* Amount Display */}
        <div className="text-center py-6">
          <p className="text-sm text-muted-foreground mb-2">You are sending</p>
          <div className="flex items-baseline justify-center gap-2">
            <span className="text-4xl font-bold tracking-tight text-foreground">
              {transaction.amount}
            </span>
            <span className="text-lg font-medium text-muted-foreground">
              {wallet.currency}
            </span>
          </div>
        </div>

        {/* Transaction Details */}
        <Card className="bg-card border-border shadow-sm">
          <CardContent className="p-0 divide-y divide-border">
            {/* To */}
            <div className="p-4">
              <p className="text-xs text-muted-foreground uppercase tracking-wider mb-1">
                To
              </p>
              <p className="font-mono text-sm text-foreground">
                {truncateAddress(transaction.recipient)}
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

            {/* Fee */}
            <div className="p-4">
              <p className="text-xs text-muted-foreground uppercase tracking-wider mb-1">
                Estimated Fee
              </p>
              <p className="text-sm font-medium text-foreground">
                ~${transaction.fee.toFixed(2)}
              </p>
            </div>
          </CardContent>
        </Card>

        {/* Warning */}
        <div className="mt-4 p-3 rounded-lg bg-secondary/50 flex items-start gap-2">
          <AlertCircle className="w-4 h-4 text-muted-foreground mt-0.5 flex-shrink-0" />
          <p className="text-xs text-muted-foreground leading-relaxed">
            Please verify the recipient address. Transactions on the blockchain
            are irreversible.
          </p>
        </div>

        {/* Spacer */}
        <div className="flex-1" />

        {/* Actions */}
        <div className="space-y-3 mt-6">
          <Button
            className="w-full h-12 bg-primary hover:bg-primary/90 text-primary-foreground"
            onClick={handleConfirm}
          >
            <span>Confirm Transfer</span>
            <ArrowRight className="w-4 h-4 ml-2" />
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
  )
}
