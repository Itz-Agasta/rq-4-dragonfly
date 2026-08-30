import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

/**
 * Merge class names, resolving Tailwind conflicts in favour of the last one.
 *
 * The conventional shadcn helper. Kept because the copied components import it;
 * prefer plain template strings in this project's own components, where the
 * class lists are short and there is nothing to resolve.
 */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
