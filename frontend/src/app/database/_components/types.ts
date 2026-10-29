import type { useMemberForm } from "@/app/database/_components/hook";
import type {
  Member,
  MemberDetails,
  NewMember,
} from "@/lib/api/generated/types.gen";

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

export type EditMemberModalProps = {
  member: Member;
  onClose: () => void;
};

export type EditMemberFormProps = {
  member: MemberDetails;
  onClose: () => void;
};

export type MemberFieldsProps = {
  joinedAtReadOnly?: boolean;
  form: ReturnType<typeof useMemberForm>;
  pending: boolean;
  details: string | null;
  setDetails: (value: string | null) => void;
};

export type ArchiveMemberModalProps = {
  member: Member;
  onClose: () => void;
};
