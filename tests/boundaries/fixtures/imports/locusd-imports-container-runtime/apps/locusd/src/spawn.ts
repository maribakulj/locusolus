import Docker from "dockerode";

export const daemon = new Docker({ socketPath: "/var/run/docker.sock" });
