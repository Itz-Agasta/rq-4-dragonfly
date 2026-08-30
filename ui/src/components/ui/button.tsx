import { cva, type VariantProps } from "class-variance-authority";
import { Slot } from "radix-ui";
import type * as React from "react";

import { cn } from "@/lib/utils";

/**
 * Button variants.
 *
 * Adapted from the upstream shadcn button. Three deliberate departures, all of
 * which are load-bearing here:
 *
 * 1. **`outline` is the default variant, not `default`.** The stock `default`
 *    paints `--primary`, which in this product is the single alarm colour. A
 *    button is not an alarm, and giving the hue a second meaning is what makes a
 *    real alarm stop reading as one. `default` and `destructive` are kept because
 *    the upstream contract includes them, but neither belongs on an ordinary
 *    action: `destructive` paints `--crit`, which is reserved for a genuine
 *    warning and is currently on one element in the whole app.
 * 2. **Focus rings are 1px and square**, not a 3px halo. Everything else in the
 *    interface is a hairline, and a soft ring reads as a different design system.
 * 3. **11px uppercase, tracked.** Matches the label grammar the rest of the
 *    interface uses, and buttons here are commands rather than prose.
 *
 * Radius and shadow need no overrides: the theme sets every radius token to 0 and
 * every small shadow to none, so `rounded-md` and `shadow-xs` already resolve flat.
 */
const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-2 rounded-md text-[11px] font-medium tracking-[0.12em] whitespace-nowrap uppercase transition-colors duration-150 outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-40 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      variant: {
        // The primary action. Near-white hairline that inverts on hover, so the
        // strongest affordance on screen still spends no colour.
        outline:
          "border border-structure-hi text-foreground hover:border-foreground hover:bg-foreground hover:text-background active:bg-foreground/90",
        // Quieter than outline, for anything secondary in the same cluster.
        ghost: "text-muted-foreground hover:bg-popover hover:text-foreground",
        // Reserved. See the note above before reaching for either of these.
        default: "bg-primary text-primary-foreground hover:bg-primary/90",
        destructive: "bg-destructive text-primary-foreground hover:bg-destructive/90",
        link: "text-foreground underline-offset-4 hover:underline",
      },
      size: {
        default: "h-8 px-5",
        sm: "h-7 px-3",
        lg: "h-9 px-7",
        icon: "size-8",
        "icon-sm": "size-7",
      },
    },
    defaultVariants: {
      variant: "outline",
      size: "default",
    },
  },
);

function Button({
  className,
  variant,
  size,
  asChild = false,
  ...props
}: React.ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean;
  }) {
  const Comp = asChild ? Slot.Root : "button";

  return (
    <Comp
      data-slot="button"
      data-variant={variant}
      data-size={size}
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  );
}

export { Button, buttonVariants };
