import { ConnectionStatus } from "@/types/camera";

interface StatusBarProps {
  connectionStatus: ConnectionStatus;
  cameraId: string | null;
}

export default function StatusBar({
  connectionStatus,
  cameraId,
}: StatusBarProps) {
  const getStatusConfig = (status: ConnectionStatus) => {
    switch (status) {
      case "connected":
        return { text: "Connected", class: "bg-green-500 shadow-green-500/50" };
      case "connecting":
        return {
          text: "Connecting...",
          class: "bg-yellow-500 shadow-yellow-500/50",
        };
      case "congested":
        return {
          text: "Connected (Congested)",
          class: "bg-orange-500 shadow-orange-500/50",
        };
      case "error":
        return { text: "Error", class: "bg-red-500 shadow-red-500/50" };
      default:
        return { text: "Disconnected", class: "bg-red-500 shadow-red-500/50" };
    }
  };

  const statusConfig = getStatusConfig(connectionStatus);

  return (
    <div className="flex flex-col lg:flex-row justify-between items-center glass-effect rounded-2xl p-4 mb-5 gap-3">
      <div className="flex items-center gap-3">
        <div
          className={`w-3 h-3 rounded-full shadow-lg ${statusConfig.class}`}
          style={{ animation: "pulse 2s infinite" }}
        />
        <span className="font-medium">{statusConfig.text}</span>
      </div>

      <div className="text-sm opacity-90">
        📡 Server: ws://100.78.140.50:3001
      </div>

      <div className="text-sm">
        {cameraId ? `Camera ID: ${cameraId}` : "Camera ID: Not connected"}
      </div>
    </div>
  );
}
