import { Button } from "@mantine/core";
import { FaMicrosoft } from "react-icons/fa";

export default function LoginButton({
  className,
  disabled = false,
}: {
  className?: string;
  disabled?: boolean;
}) {
  return (
    <Button
      component="a"
      href={disabled ? undefined : "/api/auth/login"}
      disabled={disabled}
      leftSection={<FaMicrosoft size={14} />}
      fullWidth
      radius="md"
      className={className}
    >
      Sign in with Microsoft
    </Button>
  );
}
