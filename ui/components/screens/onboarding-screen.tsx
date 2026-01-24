"use client"

import { useState } from "react"
import { Wallet, Shield, Zap, Sparkles } from "lucide-react"
import { Button } from "@/components/ui/button"
import { useWallet } from "@/lib/wallet-context"

export function OnboardingScreen() {
  const { createWallet } = useWallet()
  const [isCreating, setIsCreating] = useState(false)

  const handleCreate = () => {
    setIsCreating(true)
    setTimeout(() => {
      createWallet()
    }, 1500)
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-center p-4 border-b border-border">
        <div className="flex items-center gap-2">
          <div className="flex items-center justify-center w-8 h-8 rounded-full bg-primary">
            <Wallet className="w-4 h-4 text-primary-foreground" />
          </div>
          <span className="font-semibold text-foreground">Crypto Wallet</span>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 p-6 flex flex-col">
        {/* Hero Section */}
        <div className="flex-1 flex flex-col items-center justify-center text-center">
          <div className="w-20 h-20 rounded-2xl bg-gradient-to-br from-primary/10 to-primary/5 flex items-center justify-center mb-6">
            <Sparkles className="w-10 h-10 text-primary" />
          </div>
          
          <h1 className="text-2xl font-bold text-foreground mb-2 text-balance">
            Welcome to Your New Wallet
          </h1>
          <p className="text-muted-foreground text-sm leading-relaxed max-w-[260px]">
            Create a secure wallet to send and receive cryptocurrency in seconds.
          </p>
        </div>

        {/* Features */}
        <div className="space-y-3 mb-8">
          <div className="flex items-center gap-3 p-3 rounded-lg bg-secondary/50">
            <div className="w-9 h-9 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
              <Shield className="w-4 h-4 text-primary" />
            </div>
            <div className="text-left">
              <p className="text-sm font-medium text-foreground">Secure by Default</p>
              <p className="text-xs text-muted-foreground">End-to-end encryption</p>
            </div>
          </div>
          
          <div className="flex items-center gap-3 p-3 rounded-lg bg-secondary/50">
            <div className="w-9 h-9 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
              <Zap className="w-4 h-4 text-primary" />
            </div>
            <div className="text-left">
              <p className="text-sm font-medium text-foreground">Lightning Fast</p>
              <p className="text-xs text-muted-foreground">Instant transactions</p>
            </div>
          </div>
        </div>

        {/* CTA */}
        <Button
          className="w-full h-12 bg-primary hover:bg-primary/90 text-primary-foreground"
          onClick={handleCreate}
          disabled={isCreating}
        >
          {isCreating ? (
            <span className="flex items-center gap-2">
              <span className="w-4 h-4 border-2 border-primary-foreground/30 border-t-primary-foreground rounded-full animate-spin" />
              Creating Wallet...
            </span>
          ) : (
            "Create Wallet"
          )}
        </Button>
        
        <p className="text-xs text-muted-foreground text-center mt-4">
          By continuing, you agree to our Terms of Service
        </p>
      </div>
    </div>
  )
}
