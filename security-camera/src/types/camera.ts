export type ConnectionStatus =
  | "disconnected"
  | "connecting"
  | "connected"
  | "congested"
  | "error";

export interface StreamStats {
  frameCount: number;
  frameRate: number;
  resolution: string;
  quality: string;
  latency: string;
  dataReceived: number;
  lastFrameTime: number | null;
}

export interface LogEntry {
  timestamp: Date;
  message: string;
}

export interface NetworkFeedback {
  congested?: boolean;
  suggested_quality?: number;
  suggested_resolution?: string;
}

export interface FrameData {
  camera_id: string;
  data: string;
  timestamp?: number;
  stats?: {
    resolution?: string;
    quality?: number;
  };
}
