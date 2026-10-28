import { Button } from "@mantine/core";
import { notifications } from "@mantine/notifications";

import { useLogoutMutation } from "@/lib/api/queries/auth";

export default function LogoutButton({ csrfToken }: { csrfToken: string }) {
  const { mutate, isPending } = useLogoutMutation();

  return (
    <Button
      type="button"
      loading={isPending}
      onClick={() =>
        mutate(csrfToken, {
          onError: () =>
            notifications.show({
              color: "red",
              message: "Unable to log out. Please try again.",
            }),
        })
      }
    >
      Logout
    </Button>
  );
}
