import Image from "next/image";

interface VideoDisplayProps {
  currentFrame: string | null;
  isConnected: boolean;
}

export default function VideoDisplay({
  currentFrame,
  isConnected,
}: VideoDisplayProps) {
  return (
    <div className="bg-black/30 rounded-3xl p-5 glass-effect">
      {currentFrame ? (
        <div className="relative w-full">
          <Image
            src={currentFrame}
            alt="Camera Feed"
            width={1280}
            height={720}
            className="w-full h-auto rounded-xl shadow-2xl object-contain bg-gray-900"
            priority
            unoptimized // Since we're using data URLs
          />
        </div>
      ) : (
        <div className="flex items-center justify-center min-h-[400px] bg-gradient-to-br from-gray-800 to-gray-900 rounded-xl border-2 border-dashed border-gray-600">
          <div className="text-center">
            <div className="text-6xl mb-4">📹</div>
            <div className="text-xl text-gray-400">
              {isConnected
                ? "Waiting for camera feed..."
                : "Not connected to camera"}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
