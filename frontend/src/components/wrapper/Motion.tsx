import { Box, type BoxProps, Text, type TextProps } from "@mantine/core";
import { motion } from "motion/react";
import { type ComponentPropsWithoutRef, forwardRef } from "react";

type MotionBoxProps = BoxProps &
  Omit<ComponentPropsWithoutRef<"div">, keyof BoxProps>;

type MotionTextProps = TextProps &
  Omit<ComponentPropsWithoutRef<"p">, keyof TextProps>;

const BoxRef = forwardRef<HTMLDivElement, MotionBoxProps>((props, ref) => (
  <Box {...props} ref={ref} />
));

const TextRef = forwardRef<HTMLParagraphElement, MotionTextProps>(
  (props, ref) => <Text {...props} ref={ref} />,
);

export const MotionBox = motion.create(BoxRef);
export const MotionText = motion.create(TextRef);
