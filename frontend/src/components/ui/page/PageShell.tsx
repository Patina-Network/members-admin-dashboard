import { Box, Flex } from "@mantine/core";
import { ReactNode } from "react";

import { Footer } from "@/components/ui/footer/Footer";
import Header from "@/components/ui/header/Header";

export default function PageShell({
  children,
  hideHeader = false,
  hideFooter = false,
}: {
  children: ReactNode;
  hideHeader?: boolean;
  hideFooter?: boolean;
}) {
  return (
    <Flex direction="column" mih="100vh">
      {!hideHeader && <Header />}
      <Box p={hideHeader ? undefined : "lg"} flex={1}>
        {children}
      </Box>
      {!hideFooter && <Footer />}
    </Flex>
  );
}
