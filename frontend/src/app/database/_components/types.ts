export type AddMembersView = "choose" | "one" | "bulk-upload";

export type AddMembersModalProps = {
  opened: boolean;
  onClose: () => void;
};
