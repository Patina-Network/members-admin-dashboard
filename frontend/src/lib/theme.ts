import type { MantineThemeOverride } from "@mantine/core";

import {
  Card,
  DEFAULT_THEME,
  mergeMantineTheme,
  rem,
  Tooltip,
} from "@mantine/core";

export const themeOverride = {
  components: {
    Tooltip: Tooltip.extend({
      defaultProps: {
        withArrow: true,
        color: "dark.4",
        events: { hover: true, focus: true, touch: true },
      },
    }),
    Card: Card.extend({
      defaultProps: {
        shadow: "sm",
        padding: "lg",
        radius: "md",
        withBorder: true,
      },
    }),
  },
  autoContrast: true,
  primaryColor: "patina",
  colors: {
    patina: [
      "#e2fff3",
      "#cffae8",
      "#a4f4d0",
      "#75edb8",
      "#4DFFB0",
      "#33e495",
      "#20e28d",
      "#0ac87a",
      "#00b36b",
      "#009b59",
    ],
    dark: [
      "#FFFFFF",
      "#A6A7AB",
      "#909296",
      "#5C5F66",
      "#373A40",
      "#2C2E33",
      "#25262B",
      "#1A1B1E",
      "#141517",
      "#101113",
    ],
  },
  headings: {
    fontFamily: "Figtree, sans-serif",
    sizes: {
      h1: { fontSize: rem(64), fontWeight: "700" },
      h2: { fontSize: rem(48), fontWeight: "700" },
      h3: { fontSize: rem(34), fontWeight: "700" },
      h4: { fontSize: rem(22), fontWeight: "700" },
      h5: { fontSize: rem(16), fontWeight: "600" },
    },
  },
  fontFamily: "Figtree, sans-serif",
  fontSizes: {
    xs: rem(12),
    sm: rem(14),
    md: rem(16),
    lg: rem(20),
    xl: rem(24),
  },
  radius: {
    xs: rem(2),
    sm: rem(4),
    md: rem(8),
    lg: rem(16),
    xl: rem(32),
  },
  spacing: {
    xs: rem(10),
    sm: rem(12),
    md: rem(16),
    lg: rem(20),
    xl: rem(32),
  },
  other: {
    codebloomGray: "#303030",
    patinaGreenDark: "#03664D",
    patinaGreenLight: "#4DFFB0",
    contentContainerWidth: 1200,
  },
} as const satisfies MantineThemeOverride;

export const theme = mergeMantineTheme(DEFAULT_THEME, themeOverride);
