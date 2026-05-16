import { useState, useEffect } from "react";
import { ipc } from "../ipc";
import type { SystemStatus } from "../types";

export function useSystemStatus(): SystemStatus | null {
  const [status, setStatus] = useState<SystemStatus | null>(null);

  useEffect(() => {
    const load = () => ipc.getSystemStatus().then(setStatus).catch(() => {});
    load();
    const id = setInterval(load, 5000);
    return () => clearInterval(id);
  }, []);

  return status;
}
