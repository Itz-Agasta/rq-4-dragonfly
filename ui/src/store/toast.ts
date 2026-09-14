/**
 * Transient confirmations for commands the operator issued.
 *
 * Only for actions a person just took, never for anything the engine did. An
 * alarm belongs in the alert stack where it persists and can be acknowledged; a
 * toast that vanishes after a few seconds is the wrong home for a condition
 * somebody may need to act on, and putting one there would teach an operator to
 * watch a surface that forgets.
 *
 * Cold state, so it is ordinary zustand read through React. Nothing here runs at
 * frame rate and the render loop never touches it.
 */

import { create } from "zustand";

export type Tone = "ok" | "crit";

export interface Toast {
  id: number;
  /** One line, the command in the operator's words. */
  text: string;
  /** Optional second line: what happens next, when that is not immediate. */
  detail?: string;
  tone: Tone;
}

/**
 * How long a toast stays up, and the only way one leaves.
 *
 * Longer than the 4 s a web app would use. This display is read from across a
 * room, often by someone who pressed a button and then looked back at the
 * engine, and the whole point is that they catch the confirmation on the way
 * back. There is deliberately no dismiss control; see `Toasts`.
 */
const LIFETIME_MS = 7000;

/** Most toasts on screen at once. Older ones are dropped from the bottom. */
const MAX = 3;

interface ToastState {
  items: Toast[];
  push: (toast: Omit<Toast, "id">) => void;
}

let nextId = 0;

export const useToasts = create<ToastState>((set) => ({
  items: [],
  push: (next) => {
    const id = nextId++;
    set((state) => ({ items: [{ ...next, id }, ...state.items].slice(0, MAX) }));
    // No cleanup handle kept. The timer outliving an unmounted component is
    // harmless here: it removes an id that is already gone, and the store is a
    // module singleton that lives as long as the page.
    setTimeout(() => {
      set((state) => ({ items: state.items.filter((item) => item.id !== id) }));
    }, LIFETIME_MS);
  },
}));

/** Raise a toast from outside React. */
export function toast(text: string, detail?: string, tone: Tone = "ok"): void {
  useToasts.getState().push({ text, detail, tone });
}
