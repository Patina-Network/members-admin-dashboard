import { Button, Center, Space, Text } from "@mantine/core";
import { Link } from "react-router-dom";

import LoginButton from "@/components/ui/auth/LoginButton";
import AppCard from "@/components/ui/Card";

export default function LoginPage() {
  return (
    <Center mih="100vh">
      <AppCard w={500} maw="100%" ta="center">
        <Text fw={400} size="xl">
          Welcome to Members Admin
        </Text>
        <Space h="md" />
        <LoginButton />
        <Space h="sm" />
        <Link to="/">
          <Button size="xs" variant="subtle" fz="xs">
            Go Back
          </Button>
        </Link>
      </AppCard>
    </Center>
  );
}
