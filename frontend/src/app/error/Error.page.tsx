import { Button, Center, Stack, Text, Title } from "@mantine/core";
import { Link } from "react-router-dom";

export default function ErrorPage() {
  return (
    <Center mih="100vh">
      <Stack align="center">
        <Title order={2}>Something went wrong</Title>
        <Text c="dimmed">This page could not be loaded.</Text>
        <Button component={Link} to="/">
          Go home
        </Button>
      </Stack>
    </Center>
  );
}
