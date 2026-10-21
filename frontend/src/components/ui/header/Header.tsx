import { Box, Burger, Button, Drawer, Flex, Group, Title } from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { motion } from "motion/react";
import { Link } from "react-router-dom";

import TransitionalButtons from "@/components/ui/button/transitional/TransitionalButtons";
import HeaderContainer from "@/components/ui/header/container/HeaderContainer";
import { MotionBox, MotionText } from "@/components/wrapper/Motion";

const homeButton = { to: "/", label: "Home" };
const databaseButton = { to: "/database", label: "Database" };

export default function Header() {
  const [drawerOpened, { toggle: toggleDrawer, close: closeDrawer }] =
    useDisclosure(false);
  const navButtons = [homeButton, databaseButton];

  const authButton = <Button type="button">Login</Button>;

  return (
    <>
      <HeaderContainer>
        {({ logoSize, textOpacity, textWidth, fontSize }) => (
          <>
            <Link to="/">
              <Group>
                <motion.img
                  src="/logo.png"
                  style={{ width: logoSize, height: logoSize }}
                  alt="Logo"
                />
                <MotionBox
                  style={{
                    opacity: textOpacity,
                    maxWidth: textWidth,
                    overflow: "hidden",
                  }}
                >
                  <Title>
                    <MotionText
                      inherit
                      variant="gradient"
                      gradient={{ from: "patina.4", to: "patina.8" }}
                      style={{ fontSize }}
                    >
                      Members Admin
                    </MotionText>
                  </Title>
                </MotionBox>
              </Group>
            </Link>
            <Box visibleFrom="sm">
              <TransitionalButtons buttons={navButtons} />
            </Box>
            <Group visibleFrom="sm">{authButton}</Group>
            <Burger
              opened={drawerOpened}
              onClick={toggleDrawer}
              hiddenFrom="sm"
              aria-label="Menu button"
            />
          </>
        )}
      </HeaderContainer>
      <Drawer
        opened={drawerOpened}
        onClose={closeDrawer}
        withCloseButton={false}
        size="50%"
        title="Navigation"
        styles={{
          header: {
            justifyContent: "center",
          },
          title: {
            textAlign: "center",
            fontWeight: 700,
            fontSize: "var(--mantine-font-size-lg)",
            marginBottom: "var(--mantine-spacing-xs)",
          },
        }}
      >
        <Flex direction="column" align="center" gap="xs" w="100%">
          {navButtons.map(({ to, label }) => (
            <Button
              component={Link}
              to={to}
              size="compact-md"
              variant="transparent"
              fullWidth
              key={to}
              onClick={closeDrawer}
            >
              {label}
            </Button>
          ))}
          <Flex justify="center" w="100%" mt="xs">
            {authButton}
          </Flex>
        </Flex>
      </Drawer>
    </>
  );
}
