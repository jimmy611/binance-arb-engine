/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** WebSocket URL for the streaming hub. When unset, the simulation feed runs. */
  readonly VITE_STREAM_WS?: string;
  /** Base URL for the gRPC-Web/JSON control API. When unset, controls are local. */
  readonly VITE_CONTROL_API?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
