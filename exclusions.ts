/**
 * See https://github.com/Patina-Network/hello-world-grpc-client-rust/blob/main/.github/scripts/src/test/index.ts
 * for test exclusion usages.
 */

const baseDir = "src";

export const exclusions = [
  `${baseDir}/main.rs`,
  `${baseDir}/config.rs`,
  `${baseDir}/http/mod.rs`,
  `${baseDir}/http/metrics.rs`,
  `${baseDir}/http/version.rs`,
  `${baseDir}/http/health.rs`,
];
