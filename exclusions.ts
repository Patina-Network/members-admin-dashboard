/**
 * See https://github.com/Patina-Network/members-admin-dashboard/blob/main/.github/scripts/src/test/index.ts
 * for test exclusion usages.
 */

const baseDir = "src";

export const exclusions = [
  `${baseDir}/main.rs`,
  `${baseDir}/config.rs`,
  `${baseDir}/grpc/mod.rs`,
  `${baseDir}/http/mod.rs`,
  `${baseDir}/http/metrics.rs`,
  `${baseDir}/http/version.rs`,
  `${baseDir}/http/health.rs`,
  `${baseDir}/http/spa.rs`,
  `${baseDir}/http/state.rs`,
];
