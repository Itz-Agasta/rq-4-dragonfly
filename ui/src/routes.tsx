/**
 * Route table.
 *
 * Six screens, one shell. The shell holds the socket, so these are swapped
 * beneath a feed that never restarts.
 */

import { createBrowserRouter, Navigate } from "react-router";

import { Analysis } from "@/components/analysis/Analysis";
import { NotBuilt } from "@/components/app/NotBuilt";
import { Shell } from "@/components/app/Shell";
import { Ops } from "@/components/ops/Ops";
import { Replay } from "@/components/replay/Replay";
import { Twin } from "@/components/twin/Twin";

export const router = createBrowserRouter([
  {
    path: "/",
    element: <Shell />,
    children: [
      { index: true, element: <Navigate to="/ops" replace /> },
      { path: "ops", element: <Ops /> },
      { path: "twin", element: <Twin /> },
      { path: "analysis", element: <Analysis /> },
      {
        path: "simulate",
        element: <NotBuilt title="SIMULATE" note="mission projection · not built" />,
      },
      { path: "replay", element: <Replay /> },
      {
        path: "fleet",
        element: <NotBuilt title="FLEET" note="fleet health · not built" />,
      },
    ],
  },
]);
