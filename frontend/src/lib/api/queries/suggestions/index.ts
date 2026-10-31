import { keepPreviousData, useQuery } from "@tanstack/react-query";

import {
  careerSuggestionsQueryKey,
  industrySuggestionsQueryKey,
} from "@/lib/api/generated/@tanstack/react-query.gen";
import {
  careerSuggestions,
  industrySuggestions,
} from "@/lib/api/generated/sdk.gen";

export type SuggestionKind = "careers" | "industries";

const sources = {
  careers: { fetch: careerSuggestions, key: careerSuggestionsQueryKey },
  industries: { fetch: industrySuggestions, key: industrySuggestionsQueryKey },
} as const;

/**
 * Upstream values matching `query`. Suggestions are optional help, so failures
 * surface as an empty list instead of blocking the form.
 */
export const useSuggestionsQuery = (
  kind: SuggestionKind,
  query: string,
  enabled: boolean,
) => {
  const source = sources[kind];
  const options = { query: { query } };

  return useQuery({
    queryKey: source.key(options),
    enabled,
    retry: false,
    staleTime: 5 * 60 * 1000,
    placeholderData: keepPreviousData,
    queryFn: async ({ signal }) => {
      const result = await source.fetch({ ...options, signal });
      return result.data?.suggestions ?? [];
    },
  });
};
