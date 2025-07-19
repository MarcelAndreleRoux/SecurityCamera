import { ConnectionStatus, StreamStats, LogEntry } from "@/types/camera";

interface ControlsPanelProps {
  connectionStatus: ConnectionStatus;
  stats: StreamStats;
  logs: LogEntry[];
  onConnect: () => void;
  onDisconnect: () => void;
}

export default function ControlsPanel({
  connectionStatus,
  stats,
  logs,
  onConnect,
  onDisconnect,
}: ControlsPanelProps) {
  const isConnected =
    connectionStatus === "connected" || connectionStatus === "congested";
  const isConnecting = connectionStatus === "connecting";

  const formatDataSize = (bytes: number) => {
    return `${(bytes / 1024).toFixed(1)} KB`;
  };

  const formatTimestamp = (timestamp: number | null) => {
    if (!timestamp) return "Never";
    return new Date(timestamp).toLocaleTimeString();
  };

  return (
    <div className="glass-effect rounded-3xl p-6">
      {/* Statistics */}
      <div className="mb-6">
        <h3 className="text-lg font-semibold mb-4 text-blue-200">
          📊 Stream Statistics
        </h3>
        <div className="grid grid-cols-2 gap-3 mb-4">
          <div className="bg-white/5 p-3 rounded-xl text-center border border-white/10">
            <div className="text-2xl font-bold text-green-400">
              {stats.frameRate}
            </div>
            <div className="text-xs opacity-80 mt-1">FPS</div>
          </div>
          <div className="bg-white/5 p-3 rounded-xl text-center border border-white/10">
            <div className="text-2xl font-bold text-green-400">
              {stats.resolution}
            </div>
            <div className="text-xs opacity-80 mt-1">Resolution</div>
          </div>
          <div className="bg-white/5 p-3 rounded-xl text-center border border-white/10">
            <div className="text-2xl font-bold text-green-400">
              {stats.quality}
            </div>
            <div className="text-xs opacity-80 mt-1">Quality</div>
          </div>
          <div className="bg-white/5 p-3 rounded-xl text-center border border-white/10">
            <div className="text-2xl font-bold text-green-400">
              {stats.latency}
            </div>
            <div className="text-xs opacity-80 mt-1">Latency (ms)</div>
          </div>
        </div>
      </div>

      {/* Connection Controls */}
      <div className="mb-6">
        <h3 className="text-lg font-semibold mb-4 text-blue-200">
          🎛️ Connection Control
        </h3>
        <button
          onClick={onConnect}
          disabled={isConnected || isConnecting}
          className="w-full bg-gradient-to-r from-green-500 to-green-600 hover:from-green-600 hover:to-green-700 disabled:from-gray-600 disabled:to-gray-700 disabled:cursor-not-allowed text-white font-medium py-3 px-6 rounded-full transition-all duration-300 hover:shadow-lg hover:shadow-green-500/25 hover:-translate-y-0.5 disabled:hover:translate-y-0 disabled:hover:shadow-none mb-3"
        >
          {isConnecting ? "Connecting..." : "Connect to Camera"}
        </button>
        <button
          onClick={onDisconnect}
          disabled={!isConnected && !isConnecting}
          className="w-full bg-gradient-to-r from-red-500 to-red-600 hover:from-red-600 hover:to-red-700 disabled:from-gray-600 disabled:to-gray-700 disabled:cursor-not-allowed text-white font-medium py-3 px-6 rounded-full transition-all duration-300 hover:shadow-lg hover:shadow-red-500/25 hover:-translate-y-0.5 disabled:hover:translate-y-0 disabled:hover:shadow-none"
        >
          Disconnect
        </button>
      </div>

      {/* Connection Info */}
      <div className="mb-6">
        <h3 className="text-lg font-semibold mb-4 text-blue-200">
          📡 Connection Info
        </h3>
        <div className="bg-white/5 p-4 rounded-xl border border-white/10 font-mono text-sm space-y-2">
          <div>
            Status: <span className="text-blue-300">{connectionStatus}</span>
          </div>
          <div>
            Frames Received:{" "}
            <span className="text-green-300">{stats.frameCount}</span>
          </div>
          <div>
            Last Frame:{" "}
            <span className="text-green-300">
              {formatTimestamp(stats.lastFrameTime)}
            </span>
          </div>
          <div>
            Data Received:{" "}
            <span className="text-green-300">
              {formatDataSize(stats.dataReceived)}
            </span>
          </div>
        </div>
      </div>

      {/* Activity Log */}
      <div>
        <h3 className="text-lg font-semibold mb-4 text-blue-200">
          📝 Activity Log
        </h3>
        <div className="bg-black/30 rounded-xl p-4 max-h-48 overflow-y-auto border border-white/10">
          {logs.length === 0 ? (
            <div className="text-gray-400 text-sm">No activity yet...</div>
          ) : (
            <div className="space-y-1">
              {logs.map((log, index) => (
                <div key={index} className="text-sm font-mono opacity-90">
                  <span className="text-blue-300">
                    [{log.timestamp.toLocaleTimeString()}]
                  </span>{" "}
                  {log.message}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
