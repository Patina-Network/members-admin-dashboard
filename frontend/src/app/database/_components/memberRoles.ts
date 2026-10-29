import type { MemberRole } from "@/lib/api/generated/types.gen";

export const roleLabels: Record<MemberRole, string> = {
  board: "Board",
  mentor: "Mentor",
  mentee: "Mentee",
};

export const roleOptions = [
  { value: "", label: "Select a role" },
  ...Object.entries(roleLabels).map(([value, label]) => ({ value, label })),
];
