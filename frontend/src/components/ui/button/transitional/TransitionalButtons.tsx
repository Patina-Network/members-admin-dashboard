import { Box, Button, type ButtonProps, Group } from "@mantine/core";
import { useMediaQuery } from "@mantine/hooks";
import { useEffect, useRef, useState } from "react";
import { Link, useLocation } from "react-router-dom";
import { useImmer } from "use-immer";

import classes from "@/components/ui/button/transitional/TransitionalButtons.module.css";
import { theme } from "@/lib/theme";

interface StateProps {
  background?: string;
  color?: string;
}

interface IndicatorState {
  left: number;
  width: number;
  opacity: number;
  textColor?: string;
}

type LinkLabel = string;
type LinkTo = string;

interface Links {
  to: LinkTo;
  label: LinkLabel;
}

interface TransitionalButtonsProps {
  buttons: Links[];
  buttonProps?: ButtonProps;
  active?: StateProps;
  hover?: StateProps;
  default?: StateProps;
}

export default function TransitionalButtons({
  buttons,
  buttonProps,
  active = { background: theme.colors.patina[8], color: theme.colors.dark[9] },
  hover = { background: "white", color: theme.colors.dark[9] },
  default: defaultStyle = { color: "white" },
}: TransitionalButtonsProps) {
  const isMobile = useMediaQuery("(max-width: 768px)");
  const location = useLocation();

  const [hoverStyle, setHoverStyle] = useImmer<IndicatorState>({
    left: 0,
    width: 0,
    opacity: 0,
    textColor: defaultStyle.color,
  });
  const [activeStyle, setActiveStyle] = useImmer<IndicatorState>({
    left: 0,
    width: 0,
    opacity: 0,
    textColor: defaultStyle.color,
  });

  const [hoveredButton, setHoveredButton] = useState<LinkTo | null>(null);
  const parentRef = useRef<HTMLDivElement>(null);
  const buttonRefs = useRef<Map<LinkTo, HTMLAnchorElement>>(new Map());

  const handleMouseEnter = (
    e: React.MouseEvent<HTMLAnchorElement>,
    to: LinkTo,
  ) => {
    const button = e.currentTarget;
    const parent = parentRef.current;
    if (!parent) return;

    setHoveredButton(to);

    const buttonRect = button.getBoundingClientRect();
    const parentRect = parent.getBoundingClientRect();

    setHoverStyle((prev) => {
      prev.left = buttonRect.left - parentRect.left;
      prev.width = buttonRect.width;
      prev.opacity = 1;
      prev.textColor = hover.color;
    });
  };

  const handleParentMouseLeave = () => {
    setHoveredButton(null);
    setHoverStyle((prev) => {
      prev.opacity = 0;
      prev.textColor = defaultStyle.color;
    });
  };

  useEffect(() => {
    const parent = parentRef.current;
    if (!parent) return;

    const activeButton = buttonRefs.current.get(location.pathname);
    if (activeButton) {
      const buttonRect = activeButton.getBoundingClientRect();
      const parentRect = parent.getBoundingClientRect();

      setActiveStyle((prev) => {
        prev.left = buttonRect.left - parentRect.left;
        prev.width = buttonRect.width;
        prev.opacity = 1;
        prev.textColor = active.color;
      });
    } else {
      setActiveStyle((prev) => {
        prev.opacity = 0;
        prev.textColor = defaultStyle.color;
      });
    }
  }, [
    location.pathname,
    setActiveStyle,
    active.background,
    active.color,
    defaultStyle.color,
    isMobile,
  ]);

  return (
    <Group
      ref={parentRef}
      className={classes.navGroup}
      onMouseLeave={handleParentMouseLeave}
      wrap="nowrap"
    >
      <Box
        className={classes.hoverIndicator}
        style={{
          ...hoverStyle,
          backgroundColor: hover.background,
        }}
      />
      <Box
        className={classes.activeIndicator}
        style={{
          ...activeStyle,
          backgroundColor: active.background,
        }}
      />
      {buttons.map(({ to, label }) => {
        const isActive = location.pathname === to;
        const isHovered = hoveredButton === to;

        const textColor = (() => {
          if (isActive) return activeStyle.textColor;
          if (isHovered) return hoverStyle.textColor;
          return defaultStyle.color;
        })();

        return (
          <Button
            variant="transparent"
            component={Link}
            to={to}
            key={to}
            className={classes.navButton}
            onMouseEnter={(e) => handleMouseEnter(e, to)}
            ref={(el) => {
              if (el) {
                buttonRefs.current.set(to, el);
              } else {
                buttonRefs.current.delete(to);
              }
            }}
            style={{
              color: textColor,
              backgroundColor: defaultStyle.background,
            }}
            {...buttonProps}
          >
            {label}
          </Button>
        );
      })}
    </Group>
  );
}
