import { Anchor, Box, Flex, Stack, Text } from "@mantine/core";
import { motion, useScroll, useTransform } from "motion/react";
import { ReactNode, useRef } from "react";
import { FaDiscord, FaGithub, FaInstagram } from "react-icons/fa";

import styles from "@/components/ui/footer/Footer.module.css";

export function Footer() {
  const containerRef = useRef<HTMLDivElement>(null);
  const { scrollYProgress } = useScroll({
    target: containerRef,
    offset: ["start end", "end end"],
  });
  const y = useTransform(scrollYProgress, [0, 1], ["-100%", "0%"]);

  return (
    <Box ref={containerRef} className={styles.footerWrapper}>
      <motion.div className={styles.footer} style={{ y }}>
        <Box className={styles.footerContents}>
          <Box visibleFrom="sm" className={styles.footerLeft}>
            <Flex align="center">
              <img src="/logo.png" width={45} alt="Logo" />
              <Text fw={550} size="md">
                Members Admin
              </Text>
            </Flex>
            <Text pl="xs" c="dimmed" size="sm">
              Member records for Patina Network
            </Text>
          </Box>
          <Box className={styles.footerRight}>
            <Box hiddenFrom="sm">
              <Flex align="center">
                <img src="/logo.png" width={45} alt="Logo" />
                <Text fw={550} size="md">
                  Members Admin
                </Text>
              </Flex>
              <Text c="dimmed" size="sm">
                Member records for Patina Network
              </Text>
            </Box>
            <Stack gap={4}>
              <Text fw={550}>About</Text>
              <AnchorLink href="https://github.com/Patina-Network/members-admin-dashboard">
                <Flex align="center" gap="xs">
                  <Text>View GitHub</Text>
                  <FaGithub size={16} color="white" />
                </Flex>
              </AnchorLink>
            </Stack>
            <Stack gap={4}>
              <Text fw={550}>Community</Text>
              <AnchorLink href="https://www.instagram.com/patinanetwork">
                <Flex align="center" gap="xs">
                  <Text>Follow on Instagram</Text>
                  <FaInstagram size={16} color="white" />
                </Flex>
              </AnchorLink>
              <AnchorLink href="https://discord.com/invite/jKaPfHtcaD">
                <Flex align="center" gap="xs">
                  <Text>Join our Discord</Text>
                  <FaDiscord size={16} color="white" />
                </Flex>
              </AnchorLink>
            </Stack>
          </Box>
        </Box>
      </motion.div>
    </Box>
  );
}

function AnchorLink({ href, children }: { href: string; children: ReactNode }) {
  return (
    <Anchor
      href={href}
      c="dimmed"
      size="sm"
      variant="subtle"
      target="_blank"
      rel="noopener noreferrer"
    >
      {children}
    </Anchor>
  );
}
