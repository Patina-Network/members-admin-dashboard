import type { NewMember } from "@/lib/api/generated/types.gen";

export type AddMembersView = "choose" | "one" | "bulk-upload";

export type AddMembersModalProps = {
  opened: boolean;
  onClose: () => void;
};

export type MemberFormValues = {
  email: string;
  firstName: string;
  lastName: string;
  joinedAt: string;
  discordId: string;
  discordUsername: string;
  linkedinUsername: string;
  instagramUsername: string;
  career: string;
  industry: string;
  profile: string;
  role: NonNullable<NewMember["role"]> | "";
};
