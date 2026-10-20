import { $ } from "bun";

async function main() {
  await $`cargo fmt --check`;
  await $`cargo clippy --locked --all-targets -- -D warnings`;
}

main()
  .then(() => {
    process.exit(0);
  })
  .catch((e) => {
    console.error(e);
    process.exit(1);
  });
