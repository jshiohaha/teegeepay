"use client"

import { useState } from "react"
import { Copy, Check, ArrowUpRight, RefreshCw, Wallet, Gift } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { useWallet } from "@/lib/wallet-context"

export function BalanceScreen() {
  const { wallet, setCurrentScreen, requestAirdrop } = useWallet()
  const [copied, setCopied] = useState(false)
  const [isRefreshing, setIsRefreshing] = useState(false)
  const [isRequestingAirdrop, setIsRequestingAirdrop] = useState(false)
  
  const hasBalance = wallet.balance > 0

  const handleCopy = async () => {
    await navigator.clipboard.writeText(wallet.address)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  const handleRefresh = () => {
    setIsRefreshing(true)
    setTimeout(() => setIsRefreshing(false), 1000)
  }

  const handleRequestAirdrop = () => {
    setIsRequestingAirdrop(true)
    setTimeout(() => {
      requestAirdrop()
      setIsRequestingAirdrop(false)
    }, 2000)
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-border">
        <div className="flex items-center gap-2">
          <div className="flex items-center justify-center w-8 h-8 rounded-full bg-primary">
            <Wallet className="w-4 h-4 text-primary-foreground" />
          </div>
          <span className="font-semibold text-foreground">My Wallet</span>
        </div>
      </div>

      {/* Balance Card */}
      <div className="flex-1 p-4 flex flex-col">
        <Card className="bg-card border-border shadow-sm">
          <CardContent className="p-6">
            <p className="text-sm text-muted-foreground mb-1">Total Balance</p>
            <div className="flex items-baseline gap-2 mb-1">
              <span className="text-4xl font-bold tracking-tight text-card-foreground">
                {wallet.balance}
              </span>
              <span className="text-lg font-medium text-muted-foreground">
                {wallet.currency}
              </span>
            </div>
            <p className="text-sm text-muted-foreground">
              ~${wallet.balanceUsd.toLocaleString()} USD
            </p>
          </CardContent>
        </Card>

        {/* Address */}
        <div className="mt-4">
          <p className="text-xs text-muted-foreground mb-2 uppercase tracking-wider font-medium">
            Wallet Address
          </p>
          <button
            onClick={handleCopy}
            className="flex items-center justify-between w-full p-3 rounded-lg bg-secondary/50 hover:bg-secondary transition-colors"
          >
            <span className="font-mono text-sm text-foreground">
              {wallet.address}
            </span>
            {copied ? (
              <Check className="w-4 h-4 text-accent" />
            ) : (
              <Copy className="w-4 h-4 text-muted-foreground" />
            )}
          </button>
        </div>

        {/* Spacer */}
        <div className="flex-1" />

        {/* Actions */}
        <div className="flex gap-3 mt-6">
          <Button
            variant="outline"
            className="flex-1 h-12 bg-transparent"
            onClick={handleRefresh}
            disabled={isRefreshing}
          >
            <RefreshCw
              className={`w-4 h-4 mr-2 ${isRefreshing ? "animate-spin" : ""}`}
            />
            Refresh
          </Button>
          {hasBalance ? (
            <Button
              className="flex-1 h-12 bg-primary hover:bg-primary/90 text-primary-foreground"
              onClick={() => setCurrentScreen("send")}
            >
              <ArrowUpRight className="w-4 h-4 mr-2" />
              Send Crypto
            </Button>
          ) : (
            <Button
              className="flex-1 h-12 bg-accent hover:bg-accent/90 text-accent-foreground"
              onClick={handleRequestAirdrop}
              disabled={isRequestingAirdrop}
            >
              {isRequestingAirdrop ? (
                <span className="flex items-center gap-2">
                  <span className="w-4 h-4 border-2 border-accent-foreground/30 border-t-accent-foreground rounded-full animate-spin" />
                  Requesting...
                </span>
              ) : (
                <>
                  <Gift className="w-4 h-4 mr-2" />
                  Request Airdrop
                </>
              )}
            </Button>
          )}
        </div>
      </div>
    </div>
  )
}
