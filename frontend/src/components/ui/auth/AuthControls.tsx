import { Button, Group, Loader, Text } from "@mantine/core";
import { Link } from "react-router-dom";

import LogoutButton from "@/components/ui/auth/LogoutButton";
import { useAuthQuery } from "@/lib/api/queries/auth";

export default function AuthControls() {
  const { data, isPending, isError, refetch } = useAuthQuery();

  if (isPending) return <Loader size="sm" aria-label="Checking session" />;
  if (isError) {
    return (
      <Button variant="subtle" onClick={() => void refetch()}>
        Retry session
      </Button>
    );
  }
  if (data.user && data.csrfToken) {
    return (
      <Group justify="center">
        <Text size="sm">{data.user.name}</Text>
        <LogoutButton csrfToken={data.csrfToken} />
      </Group>
    );
  }
  return (
    <Button component={Link} to="/login">
      Sign in
    </Button>
  );
}
