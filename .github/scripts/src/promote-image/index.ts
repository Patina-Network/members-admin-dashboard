import { DockerClient } from "@tahminator/pipeline";
import yargs from "yargs";
import { hideBin } from "yargs/helpers";

import { ARCHITECTURES, dockerRepository, requiredEnv, shortSha } from "../consts";

const { releaseTag, sha, arch } = await yargs(hideBin(process.argv))
  .option("releaseTag", {
    type: "string",
    describe: "For example, 1.2.3",
    demandOption: true,
  })
  .option("sha", {
    type: "string",
    describe: "Full SHA of the tagged commit; its image must already exist",
    demandOption: true,
  })
  .option("arch", {
    choices: ARCHITECTURES,
    describe: "Image architecture to promote. Must match the runner's architecture",
    default: "amd64" as const,
  })
  .strict()
  .parse();

async function main() {
  await using dockerClient = await DockerClient.create(
    requiredEnv("DOCKER_HUB_USERNAME"),
    requiredEnv("DOCKER_HUB_PAT"),
  );

  await dockerClient.promoteDockerImage({
    originalTag: shortSha(sha),
    newGithubTags: [releaseTag, "latest"],
    repository: dockerRepository(arch),
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
