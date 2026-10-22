import {
  Card,
  type CardProps,
  createPolymorphicComponent,
} from "@mantine/core";
import { forwardRef } from "react";

const _AppCard = forwardRef<HTMLDivElement, CardProps>(
  (
    {
      children,
      withBorder = true,
      padding = "md",
      radius = "md",
      shadow = "sm",
      ...props
    },
    ref,
  ) => (
    <Card
      ref={ref}
      withBorder={withBorder}
      padding={padding}
      radius={radius}
      shadow={shadow}
      {...props}
    >
      {children}
    </Card>
  ),
);

_AppCard.displayName = "AppCard";

const AppCard = createPolymorphicComponent<"div", CardProps>(_AppCard);
export default AppCard;
