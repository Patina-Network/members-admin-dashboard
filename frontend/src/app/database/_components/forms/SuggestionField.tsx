import type { FocusEvent } from "react";

import { Autocomplete } from "@mantine/core";
import { useDebouncedValue } from "@mantine/hooks";
import { useState } from "react";

import type { MemberFieldsProps } from "@/app/database/_components/shared/types";
import type { SuggestionKind } from "@/lib/api/queries/suggestions";

import { useSuggestionsQuery } from "@/lib/api/queries/suggestions";

type SuggestionFieldProps = {
  form: MemberFieldsProps["form"];
  field: "career" | "industry";
  kind: SuggestionKind;
  label: string;
  disabled: boolean;
};

export default function SuggestionField({
  form,
  field,
  kind,
  label,
  disabled,
}: SuggestionFieldProps) {
  const [focused, setFocused] = useState(false);
  const inputProps = form.getInputProps(field);
  const [query] = useDebouncedValue(String(inputProps.value).trim(), 250);
  const { data = [] } = useSuggestionsQuery(kind, query, focused);

  return (
    <Autocomplete
      {...inputProps}
      label={label}
      data={data}
      // The upstream already matched the query; filtering again here would hide its matches.
      filter={({ options }) => options}
      disabled={disabled}
      onFocus={(event: FocusEvent<HTMLInputElement>) => {
        setFocused(true);
        inputProps.onFocus?.(event);
      }}
      onBlur={(event: FocusEvent<HTMLInputElement>) => {
        setFocused(false);
        inputProps.onBlur?.(event);
      }}
    />
  );
}
