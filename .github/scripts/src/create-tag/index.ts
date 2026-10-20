import { GitHubClient, VersioningClient, VersionUpdatingStrategy } from "@tahminator/pipeline";

import { requiredEnv } from "../consts";

async function main() {
  const ghClient = await GitHubClient.createWithGithubAppToken({
    appId: requiredEnv("_GITHUB_APP_APP_ID"),
    installationId: requiredEnv("_GITHUB_APP_INSTALLATION_ID"),
    privateKey: requiredEnv("_GITHUB_APP_PEM_CONTENT"),
  });

  const versioningClient = new VersioningClient(ghClient, VersionUpdatingStrategy.NONE);

  await ghClient.createTag({
    nextTag: await versioningClient.next(),
    onPreTagCreate: async (tag) => {
      await versioningClient.update(tag);
    },
  });
}

main()
  .then(() => {
    process.exit(0);
  })
  .catch((e) => {
    console.error(e);
    process.exit(1);
  });
