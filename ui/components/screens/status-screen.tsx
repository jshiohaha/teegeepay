"use client"

import { CheckCircle2, XCircle, Copy, ExternalLink } from "lucide-react"
import { Button } from "@/components/ui/button"
import { useWallet } from "@/lib/wallet-context"
import { useState } from "react"

export function StatusScreen() {
  const { transaction, transactionStatus, resetTransaction } = useWallet()
  const [copied, setCopied] = useState(false)

  const isSuccess = transactionStatus === "success"

  const handleCopyTxId = async () => {
    if (transaction.txId) {
      await navigator.clipboard.writeText(transaction.txId)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    }
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-center p-4 border-b border-border">
        <span className="font-semibold text-foreground">
          {isSuccess ? "Success!" : "Transaction Failed"}
        </span>
      </div>

      {/* Content */}
      <div className="flex-1 p-4 flex flex-col items-center justify-center">
        {/* Status Icon */}
        <div
          className={`flex items-center justify-center w-20 h-20 rounded-full mb-6 ${
            isSuccess ? "bg-success/10" : "bg-destructive/10"
          }`}
        >
          {isSuccess ? (
            <CheckCircle2 className="w-10 h-10 text-success" />
          ) : (
            <XCircle className="w-10 h-10 text-destructive" />
          )}
        </div>

        {/* Message */}
        <h2 className="text-xl font-semibold text-foreground mb-2 text-center">
          {isSuccess
            ? "Your transfer is complete."
            : "Something went wrong."}
        </h2>
        <p className="text-sm text-muted-foreground text-center max-w-[260px]">
          {isSuccess
            ? "Your crypto has been sent successfully to the recipient."
            : "Please try again or contact support if the issue persists."}
        </p>

        {/* Transaction ID */}
        {isSuccess && transaction.txId && (
          <div className="mt-8 w-full">
            <p className="text-xs text-muted-foreground uppercase tracking-wider mb-2 text-center">
              Transaction ID
            </p>
            <div className="flex items-center justify-center gap-2">
              <button
                onClick={handleCopyTxId}
                className="flex items-center gap-2 px-3 py-2 rounded-lg bg-secondary/50 hover:bg-secondary transition-colors"
              >
                <span className="font-mono text-sm text-foreground">
                  {transaction.txId}
                </span>
                <Copy className="w-3.5 h-3.5 text-muted-foreground" />
              </button>
            </div>
            {copied && (
              <p className="text-xs text-accent text-center mt-2">
                Copied to clipboard!
              </p>
            )}
          </div>
        )}

        {/* View on Explorer */}
        {isSuccess && (
          <button className="flex items-center gap-1.5 mt-4 text-sm text-muted-foreground hover:text-foreground transition-colors">
            <ExternalLink className="w-4 h-4" />
            <span>View on Solscan</span>
          </button>
        )}
      </div>

      {/* Done Button */}
      <div className="p-4">
        <Button
          className="w-full h-12 bg-primary hover:bg-primary/90 text-primary-foreground"
          onClick={resetTransaction}
        >
          Done
        </Button>
      </div>
    </div>
  )
}
