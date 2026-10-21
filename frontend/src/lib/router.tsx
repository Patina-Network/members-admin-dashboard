import { createBrowserRouter } from "react-router-dom";

import DatabasePage from "@/app/database/Database.page";
import ErrorPage from "@/app/error/Error.page";
import HomePage from "@/app/home/Home.page";
import LoginPage from "@/app/login/Login.page";
import PageShell from "@/components/ui/page/PageShell";

export const router = createBrowserRouter([
  {
    path: "/",
    element: (
      <PageShell>
        <HomePage />
      </PageShell>
    ),
    errorElement: <ErrorPage />,
  },
  {
    path: "/database",
    element: (
      <PageShell>
        <DatabasePage />
      </PageShell>
    ),
    errorElement: <ErrorPage />,
  },
  {
    path: "/login",
    element: (
      <PageShell hideHeader hideFooter>
        <LoginPage />
      </PageShell>
    ),
    errorElement: <ErrorPage />,
  },
]);
