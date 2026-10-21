import { Button } from "@mantine/core";

export default function LoginButton({ className }: { className?: string }) {
  return (
    <Button
      type="button"
      color="#0078D4"
      fullWidth
      radius="md"
      className={className}
    >
      Login with Azure
    </Button>
  );
}
