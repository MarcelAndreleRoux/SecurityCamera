"use client";

import { useState, useEffect, useRef } from "react";
import StatusBar from "@/components/StatusBar";
import VideoDisplay from "@/components/VideoDisplay";
import ControlsPanel from "@/components/ControlsPanel";
import {
  ConnectionStatus,
  StreamStats,
  LogEntry,
  FrameData,
  NetworkFeedback,
} from "@/types/camera";

export default function CameraStreamPage() {
  const [socket, setSocket] = useState<WebSocket | null>(null);
  const [connectionStatus, setConnectionStatus] =
    useState<ConnectionStatus>("disconnected");
  const [currentCameraId, setCurrentCameraId] = useState<string | null>(null);
  const [stats, setStats] = useState<StreamStats>({
    frameCount: 0,
    frameRate: 0,
    resolution: "-",
    quality: "-",
    latency: "-",
    dataReceived: 0,
    lastFrameTime: null,
  });
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [currentFrame, setCurrentFrame] = useState<string | null>(null);

  const fpsCounterRef = useRef<number[]>([]);
  const reconnectTimeoutRef = useRef<NodeJS.Timeout | null>(null);

  const addLog = (message: string) => {
    const newLog: LogEntry = {
      timestamp: new Date(),
      message,
    };
    setLogs((prev) => [...prev.slice(-19), newLog]); // Keep last 20 logs
  };

  const updateStats = () => {
    setStats((prev) => {
      const now = Date.now();
      fpsCounterRef.current = fpsCounterRef.current.filter(
        (time) => now - time < 1000
      );

      return {
        ...prev,
        frameRate: fpsCounterRef.current.length,
      };
    });
  };

  const handleFrame = (data: FrameData) => {
    const now = Date.now();

    setStats((prev) => ({
      ...prev,
      frameCount: prev.frameCount + 1,
      lastFrameTime: now,
      dataReceived: prev.dataReceived + data.data.length,
      resolution: data.stats?.resolution || prev.resolution,
      quality: data.stats?.quality ? `${data.stats.quality}%` : prev.quality,
      latency: data.timestamp
        ? Math.max(0, now - data.timestamp).toString()
        : prev.latency,
    }));

    // Update FPS counter
    fpsCounterRef.current.push(now);

    // Set camera ID if this is the first frame from this camera
    if (currentCameraId !== data.camera_id) {
      setCurrentCameraId(data.camera_id);
      addLog(`📹 Receiving from camera: ${data.camera_id}`);
    }

    // Display the frame
    const imageUrl = `data:image/jpeg;base64,${data.data}`;
    setCurrentFrame(imageUrl);
  };

  const handleNetworkFeedback = (feedback: NetworkFeedback) => {
    if (feedback.congested !== undefined) {
      setConnectionStatus(feedback.congested ? "congested" : "connected");

      if (feedback.congested) {
        addLog("⚠️ Network congestion detected");
      } else {
        addLog("✅ Network conditions improved");
      }
    }

    if (feedback.suggested_quality) {
      addLog(`🎚️ Server suggests quality: ${feedback.suggested_quality}%`);
    }

    if (feedback.suggested_resolution) {
      addLog(`📐 Server suggests resolution: ${feedback.suggested_resolution}`);
    }
  };

  const connect = () => {
    if (socket) {
      socket.close();
    }

    addLog("🔌 Attempting to connect...");
    setConnectionStatus("connecting");

    try {
      const newSocket = new WebSocket("ws://100.78.140.50:3001");

      newSocket.onopen = () => {
        addLog("✅ Connected to WebSocket server");
        setConnectionStatus("connected");
      };

      newSocket.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);

          if (data.camera_id && data.data) {
            handleFrame(data);
          }

          if (data.network_feedback) {
            handleNetworkFeedback(data.network_feedback);
          }
        } catch (e) {
          console.error("Error parsing message:", e);
        }
      };

      newSocket.onclose = (event) => {
        addLog(`🔌 Connection closed (Code: ${event.code})`);
        setConnectionStatus("disconnected");
        setCurrentFrame(null);
        setCurrentCameraId(null);

        // Auto-reconnect after 5 seconds if not manually disconnected
        if (event.code !== 1000) {
          reconnectTimeoutRef.current = setTimeout(() => {
            addLog("🔄 Attempting to reconnect...");
            connect();
          }, 5000);
        }
      };

      newSocket.onerror = (error) => {
        addLog("❌ WebSocket error occurred");
        console.error("WebSocket error:", error);
        setConnectionStatus("error");
      };

      setSocket(newSocket);
    } catch (error) {
      addLog("❌ Failed to create WebSocket connection");
      console.error("Connection error:", error);
      setConnectionStatus("error");
    }
  };

  const disconnect = () => {
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
    }

    if (socket) {
      addLog("🔌 Disconnecting...");
      socket.close(1000); // Normal closure
      setSocket(null);
    }
  };

  // Update stats every second
  useEffect(() => {
    const interval = setInterval(updateStats, 1000);
    return () => clearInterval(interval);
  }, [socket]);

  // Handle page visibility change
  useEffect(() => {
    const handleVisibilityChange = () => {
      if (document.hidden) {
        addLog("📱 Page hidden - maintaining connection");
      } else {
        addLog("📱 Page visible - resuming normal operation");
      }
    };

    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () =>
      document.removeEventListener("visibilitychange", handleVisibilityChange);
  }, []);

  // Cleanup on unmount
  useEffect(() => {
    addLog("🚀 Camera viewer initialized");
    addLog('💡 Click "Connect to Camera" to start viewing');

    return () => {
      if (socket) {
        socket.close();
      }
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
    };
  }, []);

  return (
    <div className="max-w-7xl mx-auto p-5">
      <div className="text-center mb-8">
        <h1 className="text-4xl font-bold mb-3 gradient-text">
          🎥 Live Camera Stream
        </h1>
        <p className="text-lg opacity-90">
          Real-time video feed with adaptive quality
        </p>
      </div>

      <StatusBar
        connectionStatus={connectionStatus}
        cameraId={currentCameraId}
      />

      <div className="grid lg:grid-cols-[1fr_300px] gap-5 items-start">
        <VideoDisplay
          currentFrame={currentFrame}
          isConnected={
            connectionStatus === "connected" || connectionStatus === "congested"
          }
        />

        <ControlsPanel
          connectionStatus={connectionStatus}
          stats={stats}
          logs={logs}
          onConnect={connect}
          onDisconnect={disconnect}
        />
      </div>
    </div>
  );
}
