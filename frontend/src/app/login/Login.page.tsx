import { Alert, Button, Center, Loader, Space, Text } from "@mantine/core";
import { Link, Navigate, useSearchParams } from "react-router-dom";

import LoginButton from "@/components/ui/auth/LoginButton";
import LogoutButton from "@/components/ui/auth/LogoutButton";
import AppCard from "@/components/ui/Card";
import { useAuthQuery } from "@/lib/api/queries/auth";

const loginErrors: Record<string, string> = {
  unavailable:
    "Azure sign-in is currently unavailable. Please try again later.",
  cancelled: "Sign-in was cancelled. You can try again.",
  invalid_callback:
    "Your sign-in request expired or was invalid. Please try again.",
  login_failed: "We couldn't complete your Azure sign-in. Please try again.",
  forbidden:
    "You need admin access. Contact an administrator if you need access.",
};

export default function LoginPage() {
  const [params] = useSearchParams();
  const { data, isPending, isError, refetch } = useAuthQuery();
  const error = loginErrors[params.get("error") ?? ""];

  if (data?.user?.isAdmin && !isError)
    return <Navigate to="/database" replace />;

  return (
    <Center mih="100vh">
      <AppCard w={500} maw="100%" ta="center">
        <Text fw={400} size="xl">
          Welcome to Members Admin
        </Text>
        <Space h="md" />
        {isPending ?
          <Loader aria-label="Checking session" />
        : isError ?
          <>
            <Alert color="red" mb="md">
              Unable to check your session.
            </Alert>
            <Button onClick={() => void refetch()}>Try again</Button>
          </>
        : data.user && data.csrfToken ?
          <>
            <Alert color="red" title="Access denied" mb="md">
              {loginErrors.forbidden}
            </Alert>
            <LogoutButton csrfToken={data.csrfToken} />
          </>
        : <>
            {error && (
              <Alert color="red" mb="md">
                {error}
              </Alert>
            )}
            {!data.loginEnabled && (
              <Alert color="yellow" mb="md">
                Sign-in is currently unavailable.
              </Alert>
            )}
            <LoginButton disabled={!data.loginEnabled} />
          </>
        }
        <Space h="sm" />
        <Button component={Link} to="/" size="xs" variant="subtle" fz="xs">
          Go Back
        </Button>
      </AppCard>
    </Center>
  );
}
