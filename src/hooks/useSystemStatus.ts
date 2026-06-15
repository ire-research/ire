import { useState, useEffect } from "react";
import { ipc } from "../ipc";
import type { SystemInfo, SystemMetrics } from "../types";

export function useSystemInfo(): SystemInfo | null {
  const [info, setInfo] = useState<SystemInfo | null>(null);

  useEffect(() => {
    ipc
      .getSystemInfo()
      .then(setInfo)
      .catch((e) => {
        console.error("Failed to load system info:", e);
        setInfo(null);
      });
  }, []);

  return info;
}

export function useSystemMetrics(): SystemMetrics | null {
  const [metrics, setMetrics] = useState<SystemMetrics | null>(null);

  useEffect(() => {
    const load = () =>
      ipc
        .getSystemMetrics()
        .then(setMetrics)
        .catch((e) => {
          console.error("Failed to load system metrics:", e);
          setMetrics(null);
        });
    load();
    const id = setInterval(load, 5000);
    return () => clearInterval(id);
  }, []);

  return metrics;
}
