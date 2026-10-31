import { Alert, Button, Center, Loader, Stack } from "@mantine/core";
import { type ReactNode } from "react";
import { Link, Navigate } from "react-router-dom";

import { useAuthQuery } from "@/lib/api/queries/auth";

export default function RequireAdmin({ children }: { children: ReactNode }) {
  const { data, isPending, isError, refetch } = useAuthQuery();

  if (isPending) {
    return (
      <Center mih="50vh">
        <Loader aria-label="Checking admin access" />
      </Center>
    );
  }

  if (isError) {
    return (
      <Stack align="center">
        <Alert color="red">Unable to check your session.</Alert>
        <Button onClick={() => void refetch()}>Try again</Button>
      </Stack>
    );
  }

  if (!data.user) return <Navigate to="/login" replace />;

  if (!data.user.isAdmin) {
    return (
      <Stack align="center">
        <Alert color="red" title="Access denied">
          You need admin access to view the member database. Contact an
          administrator if you need access.
        </Alert>
        <Button component={Link} to="/" variant="subtle">
          Go Home
        </Button>
      </Stack>
    );
  }

  return children;
}
