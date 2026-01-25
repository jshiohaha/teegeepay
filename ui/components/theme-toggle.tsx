"use client";

import { Button } from "@/components/ui/button";
import { Moon, Sun } from "lucide-react";
import { useEffect, useState } from "react";

export function ThemeToggle() {
    const [isDark, setIsDark] = useState(false);
    const [mounted, setMounted] = useState(false);

    useEffect(() => {
        const isDarkMode = document.documentElement.classList.contains("dark");
        setIsDark(isDarkMode);
        setMounted(true);
    }, []);

    const handleToggle = () => {
        const newIsDark = !isDark;
        setIsDark(newIsDark);

        if (newIsDark) {
            document.documentElement.classList.add("dark");
        } else {
            document.documentElement.classList.remove("dark");
        }
    };

    if (!mounted) {
        return (
            <Button
                size="icon"
                className="h-8 w-8 rounded-full bg-transparent hover:cursor-not-allowed hover:bg-transparent"
                disabled
            >
                <Sun className="h-4 w-4 text-muted-foreground" />
            </Button>
        );
    }

    return (
        <Button
            size="icon"
            onClick={handleToggle}
            className="h-8 w-8 rounded-full bg-transparent hover:cursor-pointer hover:bg-transparent"
            aria-label={`Switch to ${isDark ? "light" : "dark"} mode`}
        >
            {isDark ? (
                <Sun className="h-4 w-4 text-muted-foreground" />
            ) : (
                <Moon className="h-4 w-4 text-muted-foreground" />
            )}
        </Button>
    );
}
