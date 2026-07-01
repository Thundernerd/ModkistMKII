const CHANNEL_OFFSET = { alpha: 10000, beta: 20000, rc: 30000 };

function assertMsiRange(label, value, max) {
  if (!Number.isInteger(value) || value < 0 || value > max) {
    throw new Error(`MSI ${label} must be an integer between 0 and ${max}`);
  }
}

function formatMsiVersion(major, minor, patch, build) {
  assertMsiRange("major", major, 255);
  assertMsiRange("minor", minor, 255);
  assertMsiRange("patch", patch, 65535);

  if (build === undefined) {
    return `${major}.${minor}.${patch}`;
  }

  assertMsiRange("build", build, 65535);
  return `${major}.${minor}.${patch}.${build}`;
}

export function toMsiVersion(version) {
  const stable = /^(\d+)\.(\d+)\.(\d+)$/;
  const numericPre = /^(\d+)\.(\d+)\.(\d+)-(\d+)$/;
  const channelPre = /^(\d+)\.(\d+)\.(\d+)-(alpha|beta|rc)\.(\d+)$/;
  const numericBuild = /^(\d+)\.(\d+)\.(\d+)\+(\d+)$/;

  let match;
  if ((match = version.match(stable))) {
    const [, major, minor, patch] = match;
    return formatMsiVersion(Number(major), Number(minor), Number(patch));
  }

  if ((match = version.match(numericPre))) {
    const [, major, minor, patch, pre] = match;
    return formatMsiVersion(
      Number(major),
      Number(minor),
      Number(patch),
      Number(pre),
    );
  }

  if ((match = version.match(channelPre))) {
    const [, major, minor, patch, channel, number] = match;
    const build = CHANNEL_OFFSET[channel] + Number(number);
    return formatMsiVersion(
      Number(major),
      Number(minor),
      Number(patch),
      build,
    );
  }

  if ((match = version.match(numericBuild))) {
    const [, major, minor, patch, build] = match;
    return formatMsiVersion(
      Number(major),
      Number(minor),
      Number(patch),
      Number(build),
    );
  }

  throw new Error(
    `Cannot convert version for MSI: ${version}. WiX requires major.minor.patch or major.minor.patch.build with numeric-only fields.`,
  );
}
