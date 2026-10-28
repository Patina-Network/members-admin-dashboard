import {
  Alert,
  Button,
  FileInput,
  Group,
  Modal,
  Paper,
  Stack,
  Text,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconFileSpreadsheet } from "@tabler/icons-react";
import { useRef, useState } from "react";

import type {
  AddMembersModalProps,
  AddMembersView,
} from "@/app/database/_components/types";
import type { NewMember } from "@/lib/api/generated/types.gen";

import { useMemberForm } from "@/app/database/_components/hook";
import MemberFields from "@/app/database/_components/MemberFields";
import { useAddMemberMutation } from "@/lib/api/queries/members";

export default function AddMembersModal({
  opened,
  onClose,
}: AddMembersModalProps) {
  const [details, setDetails] = useState<string | null>(null);
  const [view, setView] = useState<AddMembersView>("choose");
  const addMember = useAddMemberMutation();
  const form = useMemberForm();
  const submittingRef = useRef(false);
  const close = () => {
    if (submittingRef.current) return;
    onClose();
    setView("choose");
    setDetails(null);
    form.reset();
    addMember.reset();
  };
  const onSubmit = (values: NewMember) => {
    if (submittingRef.current) return;
    submittingRef.current = true;
    addMember.mutate(values, {
      onSuccess: () => {
        notifications.show({ message: "Member added.", color: "green" });
        form.reset();
        addMember.reset();
        setView("choose");
        setDetails(null);
        onClose();
      },
      onSettled: () => {
        submittingRef.current = false;
      },
    });
  };

  return (
    <Modal
      opened={opened}
      onClose={close}
      title="Add Member(s)"
      size="lg"
      centered
      closeOnClickOutside={!addMember.isPending}
      closeOnEscape={!addMember.isPending}
      withCloseButton={!addMember.isPending}
    >
      {view === "choose" && (
        <Stack>
          <Button onClick={() => setView("one")}>Add One Member</Button>
          <Button variant="light" onClick={() => setView("bulk-upload")}>
            Upload Bulk Member(s)
          </Button>
        </Stack>
      )}
      {view === "one" && (
        <form
          noValidate
          onSubmit={form.onSubmit(onSubmit, (errors) => {
            if (
              [
                "discordId",
                "discordUsername",
                "linkedinUsername",
                "instagramUsername",
                "career",
                "industry",
              ].some((field) => errors[field])
            ) {
              setDetails("additional-details");
            }
          })}
        >
          <Stack>
            {addMember.error && (
              <Alert color="red" role="alert">
                {addMember.error.message}
              </Alert>
            )}
            <MemberFields
              form={form}
              pending={addMember.isPending}
              details={details}
              setDetails={setDetails}
            />
            <Group justify="flex-end">
              <Button
                variant="default"
                disabled={addMember.isPending}
                onClick={() => {
                  if (submittingRef.current) return;
                  addMember.reset();
                  setView("choose");
                }}
              >
                Back
              </Button>
              <Button
                type="submit"
                loading={addMember.isPending}
                disabled={addMember.isPending}
              >
                Add
              </Button>
            </Group>
          </Stack>
        </form>
      )}
      {view === "bulk-upload" && (
        <Stack>
          <Text size="sm" c="dimmed">
            Upload a CSV with these columns:
            email,first_name,last_name,joined_at,discord_id,discord_username
          </Text>
          <Paper withBorder p="xl">
            <Group justify="center" gap="xl" mih={140}>
              <IconFileSpreadsheet size={40} />
              <Stack gap="xs">
                <Text size="lg">Drag a CSV here or click to select one</Text>
                <Text size="sm" c="dimmed">
                  One member per row. Header row required.
                </Text>
              </Stack>
            </Group>
          </Paper>
          <FileInput disabled placeholder="Or choose a file" />
          <Group justify="flex-end">
            <Button
              type="button"
              variant="default"
              onClick={() => setView("choose")}
            >
              Back
            </Button>
          </Group>
        </Stack>
      )}
    </Modal>
  );
}
