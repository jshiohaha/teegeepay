import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

export const truncateString = (str: string, prefixLen = 6, suffixLen = 4) => {
    const minLength = prefixLen + suffixLen + 3;
    if (str.length <= minLength) return str;
    return `${str.slice(0, prefixLen)}...${str.slice(-suffixLen)}`;
};
