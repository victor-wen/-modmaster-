import { useEffect, useRef } from "react";
import { createClient, type IpcClient } from "../ipc/client";
import { useStore } from "../store";
import type { TagUpdate } from "../ipc/bindings";
import type { UnlistenFn } from "@tauri-apps/api/event";
const client: IpcClient = createClient();
export function useTagSubscription() {
  const rafRef = useRef<number>(0);
  const pendingRef = useRef<TagUpdate[]>([]);
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    (async () => {
      unlisten = await client.onTagUpdate((updates) => {
        pendingRef.current.push(...updates);
        if (!rafRef.current) {
          rafRef.current = requestAnimationFrame(() => {
            const batch = pendingRef.current.splice(0);
            useStore.getState().updateTagValues(batch);
            rafRef.current = 0;
          });
        }
      });
    })();
    return () => { unlisten?.(); if (rafRef.current) cancelAnimationFrame(rafRef.current); };
  }, []);
}
