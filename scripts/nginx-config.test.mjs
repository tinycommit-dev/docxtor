// The generated nginx rules and the Docker image's server block pass `nginx -t` inside the pinned nginx image. Needs Docker (skipped, with
// the reason, when there is none); the release workflow and CI have it. Also: the Dockerfile pins every base image by digest.
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { test } from "node:test";

const dockerfile = readFileSync("deploy/Dockerfile", "utf8");

test("every base image in the Dockerfile is pinned by digest", () => {
  const images = [...dockerfile.matchAll(/^FROM (\S+)/gm), ...dockerfile.matchAll(/^COPY --from=(\S+)/gm)].map((m) => m[1]).filter((i) => i !== "build");
  assert.ok(images.length >= 4, `found ${images.join(", ")}`);
  for (const i of images) assert.match(i, /@sha256:[0-9a-f]{64}$/, `${i} has a digest`);
});

const docker = spawnSync("docker", ["info"], { stdio: "ignore" }).status === 0;
test("nginx -t passes on the generated headers and the image's server block", { skip: docker ? false : "Docker is not running" }, () => {
  const final = [...dockerfile.matchAll(/^FROM (\S+)/gm)].map((m) => m[1]).at(-1);
  // nginx -t exits non-zero (and execFileSync throws with nginx's message) when the configuration is wrong.
  execFileSync(
    "docker",
    ["run", "--rm", "-v", `${resolve("deploy/nginx.conf")}:/etc/nginx/docxtor-headers.conf:ro`, "-v", `${resolve("deploy/docker/default.conf")}:/etc/nginx/conf.d/default.conf:ro`, "--entrypoint", "nginx", final, "-t"],
    { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
  );
});
