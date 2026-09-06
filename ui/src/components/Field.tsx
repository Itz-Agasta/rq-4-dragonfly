/**
 * A label and its value, side by side.
 *
 * The grammar the SIMULATE run panel and REPLAY's environment block already
 * use, and what keeps a confirmation card from reading as a web dialogue.
 *
 * The label column is fixed, not intrinsic, so values line up down the card.
 * 74px holds the longest label either card uses; a longer one wraps.
 */
export function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex gap-3 py-[5px]">
      <span className="label-micro w-[74px] shrink-0 pt-[3px]">{label}</span>
      <div className="min-w-0 flex-1 text-[12px] leading-[1.5]">{children}</div>
    </div>
  );
}
