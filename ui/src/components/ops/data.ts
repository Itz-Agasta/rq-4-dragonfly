/**
 * Authored values for the parts of OPS that nothing computes yet.
 *
 * MOCK: every export here is replaced by real output. The alert stack comes from
 * real residual excursions once there is a rule that raises them; the advisory's
 * diagnosis, remaining useful life and risk come from the diagnosis and prognosis
 * layers. Nothing here is a *measurement*: the strips, the schematic callouts,
 * the mission bar and now the health rail are all live, so removing this file
 * removes authored judgements, not authored physics.
 *
 * The subsystem health indices used to live here and no longer do: they are the
 * twin's, read from the frame inside the render loop.
 *
 * The numbers are not arbitrary. They serve one story: a single degrading
 * subsystem, injector coking on cylinder 3, with everything else quiet. Exactly
 * one subsystem may look degraded, and a channel's apparent divergence has to
 * match the sigma it claims.
 */

import type { Alert } from "@/store/app";

/**
 * MOCK: replaced by real residual excursions.
 *
 * The two advisories are acknowledged and historical. Only the caution is live,
 * which is what keeps this consistent with a single-fault diagnosis: an
 * acknowledged past event is not present divergence.
 */
export const ALERTS: readonly Alert[] = [
  {
    id: "egt3-3sigma",
    t_s: 15_128, // T+04:12:08
    severity: "caution",
    subsystem: "FUEL/INJECTION",
    message: "EGT cyl-3 residual −67 K, exceeds 3σ",
    screen: "analysis",
  },
  {
    id: "coolant-drift",
    t_s: 13_731, // T+03:48:51
    severity: "advisory",
    subsystem: "THERMAL",
    message: "Coolant residual drift +1.8 K sustained",
    screen: "twin",
  },
  {
    id: "wastegate-step",
    t_s: 3974, // T+01:06:14
    severity: "advisory",
    subsystem: "AIR PATH",
    message: "Wastegate duty step, twin re-locked",
    screen: "twin",
  },
];

/** Alerts that arrive already acknowledged. Only the caution needs action. */
export const PRE_ACKED: readonly string[] = ["coolant-drift", "wastegate-step"];

/** MOCK: replaced by the diagnosis and prognosis layers. */
export const ADVISORY = {
  diagnosis: "Probable injector-3 coking",
  confidencePct: 91,
  rulHours: 6.2,
  rulLowHours: 4.8,
  rulHighHours: 7.9,
  coveragePct: 90,
  recommendation: "Return to base. Est. maintenance 45 min.",
  continueRiskPct: 34,
} as const;

/** Cylinder the fault sits on, 1-based. Drives the schematic's accented bore. */
export const FAULT_CYLINDER = 3;
