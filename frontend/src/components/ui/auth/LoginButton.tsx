import { Button } from "@mantine/core";
import { FaMicrosoft } from "react-icons/fa";

export default function LoginButton({ className }: { className?: string }) {
  return (
    <Button
      component="a"
      href="/api/auth/login"
      leftSection={<FaMicrosoft size={14} />}
      fullWidth
      radius="md"
      className={className}
    >
      Sign in with Microsoft
    </Button>
  );
}
