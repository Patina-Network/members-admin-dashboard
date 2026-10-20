export const GITHUB_OWNER = "Patina-Network";
export const GITHUB_REPOSITORY = "hello-world-grpc-client-rust";
const DOCKER_REPOSITORY = "hello-world-client-rust";

/** Repo under GITHUB_OWNER that `deploy` opens image-tag PRs against. */
export const K8S_MANIFESTS_REPOSITORY = "k8s-manifests";

export const SONAR_ORGANIZATION = "patina-network";
export const SONAR_PROJECT_KEY = `${GITHUB_OWNER}_${GITHUB_REPOSITORY}`;

export const ARCHITECTURES = ["amd64", "arm64"] as const;
export type Architecture = (typeof ARCHITECTURES)[number];

export function dockerRepository(arch: Architecture = "amd64") {
  return arch === "arm64" ? `${DOCKER_REPOSITORY}-arm` : DOCKER_REPOSITORY;
}

export function shortSha(sha: string) {
  if (!/^[0-9a-f]{40}$/.test(sha)) {
    throw new Error(`Expected a full commit SHA, got "${sha}"`);
  }
  return sha.slice(0, 7);
}

export function requiredEnv(name: string) {
  const v = process.env[name];
  if (!v) {
    throw new Error(`Missing ${name} from env`);
  }
  return v;
}
