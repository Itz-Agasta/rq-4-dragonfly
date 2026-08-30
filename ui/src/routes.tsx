/**
 * Route table.
 *
 * Six screens, one shell. The shell holds the socket, so these are swapped
 * beneath a feed that never restarts.
 */

import { createBrowserRouter, Navigate } from "react-router";

import { NotBuilt } from "@/components/app/NotBuilt";
import { Shell } from "@/components/app/Shell";
import { Ops } from "@/components/ops/Ops";

export const router = createBrowserRouter([
  {
    path: "/",
    element: <Shell />,
    children: [
      { index: true, element: <Navigate to="/ops" replace /> },
      { path: "ops", element: <Ops /> },
      {
        path: "twin",
        element: <NotBuilt title="TWIN" note="measured against physics · not built" />,
      },
      {
        path: "analysis",
        element: <NotBuilt title="ANALYSIS" note="diagnosis and prognosis · not built" />,
      },
      {
        path: "simulate",
        element: <NotBuilt title="SIMULATE" note="mission projection · not built" />,
      },
      {
        path: "replay",
        element: <NotBuilt title="REPLAY" note="mission scrubbing · not built" />,
      },
      {
        path: "fleet",
        element: <NotBuilt title="FLEET" note="fleet health · not built" />,
      },
    ],
  },
]);
