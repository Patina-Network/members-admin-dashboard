import "@mantine/core/styles.css";
import "@mantine/dropzone/styles.css";
import "@mantine/notifications/styles.css";

import "@/index.css";
import { MantineProvider } from "@mantine/core";
import { Notifications } from "@mantine/notifications";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { RouterProvider } from "react-router-dom";

import { setupApi } from "@/lib/api/setup";
import ReactQueryProvider from "@/lib/queryProvider";
import { router } from "@/lib/router";
import { themeOverride } from "@/lib/theme";

setupApi();

const root = document.getElementById("root");
if (!root) throw new Error("Missing root element");

createRoot(root).render(
  <StrictMode>
    <ReactQueryProvider>
      <MantineProvider theme={themeOverride} forceColorScheme="dark">
        <RouterProvider router={router} />
        <Notifications />
      </MantineProvider>
    </ReactQueryProvider>
  </StrictMode>,
);
