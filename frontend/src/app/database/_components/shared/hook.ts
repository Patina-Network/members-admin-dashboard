import { useForm } from "@mantine/form";
import { useEffect, useState } from "react";
import {
  useLocation,
  useNavigationType,
  useSearchParams,
} from "react-router-dom";

import type {
  BulkImportPhase,
  BulkRowStatus,
  CsvMemberRow,
  MemberFormValues,
} from "@/app/database/_components/shared/types";
import type {
  BulkResult,
  Member,
  MemberDetails,
  MemberUpdate,
  NewMember,
} from "@/lib/api/generated/types.gen";

import {
  optionalColumns,
  parseMembersCsv,
  requiredColumns,
} from "@/app/database/_components/import/csv";
import { isMemberRole } from "@/app/database/_components/shared/memberRoles";
import {
  validateDiscordId,
  validateEmail,
  validateSocialUsername,
} from "@/app/database/_components/shared/memberValidation";
import {
  useBulkImportMembersMutation,
  useMembersQuery,
} from "@/lib/api/queries/members";

const initialValues: MemberFormValues = {
  email: "",
  firstName: "",
  lastName: "",
  joinedAt: "",
  discordId: "",
  discordUsername: "",
  linkedinUsername: "",
  instagramUsername: "",
  career: "",
  industry: "",
  profile: "",
  role: "",
};

export const useMemberForm = (
  initial?: Partial<MemberFormValues>,
  editing = false,
) => {
  const discordChanged = (values: MemberFormValues) =>
    !editing ||
    values.discordId !== initial?.discordId ||
    values.discordUsername !== initial?.discordUsername;

  const form = useForm<
    MemberFormValues,
    (values: MemberFormValues) => NewMember
  >({
    initialValues: { ...initialValues, ...initial },
    onValuesChange: (current, previous) => {
      if (
        current.discordId !== previous.discordId ||
        current.discordUsername !== previous.discordUsername
      ) {
        form.clearFieldError("discordId");
        form.clearFieldError("discordUsername");
      }
    },
    validate: {
      discordId: (value, values) => {
        if (!discordChanged(values)) return null;
        return validateDiscordId(value, values.discordUsername);
      },
      discordUsername: (value, values) =>
        discordChanged(values) && values.discordId.trim() && !value.trim() ?
          "Discord username is required with an ID"
        : null,
      linkedinUsername: validateSocialUsername,
      instagramUsername: validateSocialUsername,
      email: validateEmail,
      firstName: (value) => (value.trim() ? null : "First name is required"),
      lastName: (value) => (value.trim() ? null : "Last name is required"),
      joinedAt: (value) => {
        if (editing) return null;
        if (!value) return null;

        const date = new Date(value);
        if (Number.isNaN(date.getTime())) return "Enter a valid date and time";

        return date.getTime() > Date.now() ?
            "Joined at must not be in the future"
          : null;
      },
    },
    transformValues: (values): NewMember => ({
      email: values.email.trim(),
      first_name: values.firstName.trim(),
      last_name: values.lastName.trim(),
      joined_at:
        values.joinedAt ? new Date(values.joinedAt).toISOString() : null,
      discord_id: values.discordId.trim() || null,
      discord_username: values.discordUsername.trim() || null,
      linkedin_username: values.linkedinUsername.trim() || null,
      instagram_username: values.instagramUsername.trim() || null,
      career: values.career.trim() || null,
      industry: values.industry.trim() || null,
      profile: values.profile.trim() || null,
      role: values.role || null,
    }),
  });

  return form;
};

export const useEditMemberForm = (member: MemberDetails) => {
  const date = member.joined_at ? new Date(member.joined_at) : null;
  const joinedAt =
    date ?
      new Date(date.getTime() - date.getTimezoneOffset() * 60000)
        .toISOString()
        .slice(0, 19)
    : "";

  const form = useMemberForm(
    {
      email: member.email,
      firstName: member.first_name,
      lastName: member.last_name,
      joinedAt,
      linkedinUsername: member.linkedin_username ?? "",
      instagramUsername: member.instagram_username ?? "",
      career: member.career ?? "",
      industry: member.industry ?? "",
      profile: member.profile ?? "",
      role: member.role ?? "",
      discordId: member.discord_id ?? "",
      discordUsername: member.discord_username ?? "",
    },
    true,
  );

  const changedFields = (values: NewMember): MemberUpdate => {
    const body: MemberUpdate = { expected_updated_at: member.updated_at };

    if (form.isDirty("email")) body.email = values.email;
    if (form.isDirty("firstName")) body.first_name = values.first_name;
    if (form.isDirty("lastName")) body.last_name = values.last_name;

    if (form.isDirty("discordId") || form.isDirty("discordUsername")) {
      body.discord_id = values.discord_id;
      body.discord_username = values.discord_username;
    }

    if (form.isDirty("linkedinUsername"))
      body.linkedin_username = values.linkedin_username;
    if (form.isDirty("instagramUsername"))
      body.instagram_username = values.instagram_username;
    if (form.isDirty("career")) body.career = values.career;
    if (form.isDirty("industry")) body.industry = values.industry;
    if (form.isDirty("profile")) body.profile = values.profile;
    if (form.isDirty("role")) body.role = values.role;

    return body;
  };

  return { form, changedFields };
};

export const membersPerPage = 10;

/** Keeps member search, role filters, and pagination in the URL without resetting a saved page during loading. */
export function useMemberTable() {
  const members = useMembersQuery();
  const [createdMemberId, setCreatedMemberId] = useState<number | null>(null);
  const [searchParams, setSearchParams] = useSearchParams();
  const location = useLocation();
  const navigationType = useNavigationType();

  useEffect(() => {
    if (navigationType === "POP") setCreatedMemberId(null);
  }, [location.key, navigationType]);

  const search = searchParams.get("search") ?? "";
  const setSearch = (value: string) => {
    setCreatedMemberId(null);
    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current);
        if (value) next.set("search", value);
        else next.delete("search");

        next.set("page", "1");
        return next;
      },
      { replace: true },
    );
  };

  const roles = searchParams.getAll("role").filter(isMemberRole);
  const setRoles = (value: string[]) => {
    setCreatedMemberId(null);
    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current);
        next.delete("role");

        for (const role of value) {
          if (isMemberRole(role)) next.append("role", role);
        }

        next.set("page", "1");
        return next;
      },
      { replace: true },
    );
  };

  const requestedPage = Number(searchParams.get("page") ?? 1);
  const page =
    Number.isSafeInteger(requestedPage) && requestedPage > 0 ?
      requestedPage
    : 1;
  const setPage = (nextPage: number) => {
    setCreatedMemberId(null);
    setSearchParams((current) => {
      const next = new URLSearchParams(current);
      next.set("page", String(nextPage));
      return next;
    });
  };

  const allMembers = members.data?.members ?? [];
  const searchPrefix = search.trim().toLocaleLowerCase();
  const selectedRoles = new Set(roles);

  const filteredMembers = allMembers.filter((member) => {
    const nameMatches = member.first_name
      .toLocaleLowerCase()
      .startsWith(searchPrefix);
    const roleMatches =
      selectedRoles.size === 0 ||
      (member.role != null && selectedRoles.has(member.role));

    return nameMatches && roleMatches;
  });

  const pageCount = Math.max(
    1,
    Math.ceil(filteredMembers.length / membersPerPage),
  );
  const createdIndex =
    navigationType === "POP" ? -1 : (
      filteredMembers.findIndex(
        (member) => member.member_id === createdMemberId,
      )
    );
  const currentPage =
    createdIndex >= 0 ?
      Math.floor(createdIndex / membersPerPage) + 1
    : Math.min(page, pageCount);

  const showCreatedMember = (member: Member) => {
    setCreatedMemberId(member.member_id);
    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current);
        next.delete("search");
        next.delete("role");
        next.set("page", "1");
        return next;
      },
      { replace: true },
    );
  };

  useEffect(() => {
    if (!members.isSuccess || members.isFetching) return;
    setCreatedMemberId(null);

    const rawPage = searchParams.get("page");
    if (rawPage === null || rawPage === String(currentPage)) return;

    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current);
        next.set("page", String(currentPage));
        return next;
      },
      { replace: true },
    );
  }, [
    createdMemberId,
    currentPage,
    members.isSuccess,
    members.isFetching,
    searchParams,
    setSearchParams,
  ]);

  const visibleMembers = filteredMembers.slice(
    (currentPage - 1) * membersPerPage,
    currentPage * membersPerPage,
  );

  return {
    members,
    search,
    setSearch,
    roles,
    setRoles,
    setPage,
    currentPage,
    pageCount,
    visibleMembers,
    matchingCount: filteredMembers.length,
    hasSearch: searchPrefix.length > 0,
    showCreatedMember,
  };
}

type Outcome = BulkResult["outcome"];

const outcomeStyles: Record<
  Outcome,
  { label: string; tally: string; color: string }
> = {
  created: { label: "Added", tally: "added", color: "green" },
  updated: { label: "Updated", tally: "updated", color: "blue" },
  unchanged: { label: "Unchanged", tally: "unchanged", color: "gray" },
  failed: { label: "Failed", tally: "failed", color: "red" },
};

const unreportedRowLabels: Record<BulkImportPhase, string> = {
  ready: "Ready",
  pending: "Importing…",
  done: "Unconfirmed",
  unconfirmed: "Unconfirmed",
};

/** CSV import: parses one file, submits it once, and reports each row's result. */
export function useBulkImport(onPending: (pending: boolean) => void) {
  const [rows, setRows] = useState<CsvMemberRow[]>([]);
  const [fileName, setFileName] = useState("");
  const [csvError, setCsvError] = useState("");
  const [reading, setReading] = useState(false);
  const [upsert, setUpsert] = useState(false);
  const mutation = useBulkImportMembersMutation();
  const [submitting, setSubmitting] = useState(false);

  // Keeps the modal open while a file is parsing or an import is running.
  useEffect(() => {
    onPending(reading || mutation.isPending);
  }, [onPending, reading, mutation.isPending]);

  const upload = async ([file]: File[]) => {
    if (!file) return;

    setReading(true);
    setCsvError("");
    try {
      setRows(parseMembersCsv(await file.text()));
      setFileName(file.name);
    } catch (cause) {
      setCsvError(
        cause instanceof Error ? cause.message : "Unable to read CSV.",
      );
    } finally {
      setReading(false);
    }
  };

  const rejectFile = () =>
    setCsvError(
      "Choose one .csv file of 1 MB or less. Split larger files into smaller imports.",
    );

  const submit = () => {
    setSubmitting(true);
    mutation.mutate({
      members: rows.map((row) => row.member),
      mode: upsert ? "upsert" : "create",
    });
  };

  const reset = () => {
    setSubmitting(false);
    setRows([]);
    setFileName("");
    setCsvError("");
    mutation.reset();
  };

  // `submitting` flips on the click's own render, so the Import button is
  // disabled before a second click; `mutation.isPending` flips a tick later.
  const phase: BulkImportPhase =
    mutation.data ? "done"
    : mutation.error ? "unconfirmed"
    : submitting ? "pending"
    : "ready";

  const results = mutation.data?.results ?? [];
  const resultsByRow = new Map(results.map((result) => [result.index, result]));

  const rowStatus = (index: number): BulkRowStatus => {
    const result = resultsByRow.get(index);
    if (!result) return { label: unreportedRowLabels[phase], color: "dimmed" };

    const { label, color } = outcomeStyles[result.outcome];
    return { label: result.outcome === "failed" ? result.error : label, color };
  };

  const tallies = Object.entries(outcomeStyles)
    .map(([outcome, { tally, color }]) => ({
      label: tally,
      color,
      count: results.filter((result) => result.outcome === outcome).length,
    }))
    .filter(({ count }) => count > 0);

  const status = {
    ready: `Ready to import ${rows.length} members. Click "Import" to confirm.`,
    pending: "Importing members… Please keep this window open.",
    done: tallies.map(({ count, label }) => `${count} ${label}`).join(", "),
    unconfirmed:
      "Results are unconfirmed. Refresh the member table before retrying.",
  }[phase];

  const alert =
    csvError ? { title: "Check your CSV", message: csvError }
    : mutation.error ?
      {
        title: "Import could not be confirmed",
        message: mutation.error.message,
      }
    : null;

  const previewColumns = [
    ...requiredColumns,
    ...optionalColumns.filter((column) =>
      rows.some(({ member }) => member[column] != null),
    ),
  ];

  return {
    rows,
    fileName,
    reading,
    upsert,
    setUpsert,
    phase,
    alert,
    status,
    tallies,
    previewColumns,
    rowStatus,
    upload,
    rejectFile,
    submit,
    reset,
  };
}
