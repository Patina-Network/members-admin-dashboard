import { Alert, Button, Group, Modal, Stack } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { useRef, useState } from "react";

import type { AddMembersModalProps } from "@/app/database/_components/types";
import type { NewMember } from "@/lib/api/generated/types.gen";

import { useMemberForm } from "@/app/database/_components/hook";
import MemberFields from "@/app/database/_components/MemberFields";
import { useAddMemberMutation } from "@/lib/api/queries/members";

export default function AddMembersModal({
  opened,
  onClose,
}: AddMembersModalProps) {
  const [details, setDetails] = useState<string | null>(null);
  const addMember = useAddMemberMutation();
  const form = useMemberForm();
  const submittingRef = useRef(false);
  const close = () => {
    if (submittingRef.current) return;
    onClose();
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
      title="Add member"
      size="lg"
      centered
      closeOnClickOutside={!addMember.isPending}
      closeOnEscape={!addMember.isPending}
      withCloseButton={!addMember.isPending}
    >
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
              onClick={close}
            >
              Cancel
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
    </Modal>
  );
}
