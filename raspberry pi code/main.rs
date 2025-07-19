use tokio::process::Command;
use base64::prelude::*;
use tokio::io::AsyncReadExt;  // This is actually used in process_frames
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use uuid::Uuid;
use std::{sync::{Arc, atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering}}, time::Duration};
use tokio::{sync::mpsc, time::sleep};

struct NetworkState {
    is_congested: bool,
    congestion_level: u8,       // 0-10 scale, higher means more congested
    stability_counter: u32,     // counts stable measurements before allowing changes
    last_resolution_change: std::time::Instant, // prevent rapid resolution changes
}

impl NetworkState {
    fn new() -> Self {
        Self { 
            is_congested: false, 
            congestion_level: 0,
            stability_counter: 0,
            last_resolution_change: std::time::Instant::now(),
        }
    }

    // Update congestion state with hysteresis
    fn update_congestion(&mut self, queue_size: u64, consecutive_failures: u32, server_congestion: bool) -> (bool, u32, u32) {
        // Combine multiple congestion indicators
        let new_congestion_indicators = 
            (if queue_size > 20 { 2 } else if queue_size > 10 { 1 } else { 0 }) +
            (if consecutive_failures > 3 { 3 } else if consecutive_failures > 0 { 1 } else { 0 }) +
            (if server_congestion { 3 } else { 0 });
        
        // Gradually adjust congestion level (with inertia)
        if new_congestion_indicators > (self.congestion_level as u32) {
            self.congestion_level = (self.congestion_level + 1).min(10);
        } else if new_congestion_indicators < (self.congestion_level as u32) && self.stability_counter > 5 {
            self.congestion_level = self.congestion_level.saturating_sub(1);
        }
        
        // Reset stability counter if indicators changed significantly
        if (new_congestion_indicators as i32 - self.congestion_level as i32).abs() > 2 {
            self.stability_counter = 0;
        } else {
            self.stability_counter += 1;
        }
        
        // Determine if we should change resolution and quality based on congestion level
        // and how long since the last change
        let now = std::time::Instant::now();
        let time_since_last_change = now.duration_since(self.last_resolution_change);
        
        let should_reduce = self.congestion_level > 6 && 
                           time_since_last_change > Duration::from_secs(2) && 
                           !self.is_congested;
                           
        let should_increase = self.congestion_level < 3 && 
                              time_since_last_change > Duration::from_secs(15) && 
                              self.is_congested && 
                              self.stability_counter > 20;
        
        // Calculate target quality and resolution
        let (width, height, quality) = if should_reduce || self.is_congested {
            self.is_congested = true;
            self.last_resolution_change = now;
            (640, 480, 50 - self.congestion_level as u32 * 2)
        } else if should_increase {
            self.is_congested = false;
            self.last_resolution_change = now;
            (1280, 720, 70)
        } else if self.is_congested {
            // Maintain lower resolution but adjust quality based on current congestion
            (640, 480, 50 - self.congestion_level as u32 * 2)
        } else {
            // Maintain higher resolution but adjust quality based on current congestion
            (1280, 720, 70 - self.congestion_level as u32 * 3)
        };
        
        // Log meaningful state changes
        if should_reduce {
            println!("Network congestion detected (level {}). Reducing resolution to {}x{}, quality to {}", 
                    self.congestion_level, width, height, quality);
        } else if should_increase {
            println!("Network stable (level {}) for {} frames. Increasing resolution to {}x{}, quality to {}",
                    self.congestion_level, self.stability_counter, width, height, quality);
        }
        
        (self.is_congested, width, quality.max(20))
    }
}

// Define process_frames first so it's in scope when called
async fn process_frames(
    mut stdout: tokio::process::ChildStdout,
    tx: mpsc::Sender<Vec<u8>>,
    queue_size: Arc<AtomicU64>
) {
    tokio::spawn(async move {
        let mut accumulated_data = Vec::new();
        let mut buffer = vec![0; 512 * 1024]; // 512KB buffer
        
        loop {
            match stdout.read(&mut buffer).await {
                Ok(0) => {
                    println!("End of GStreamer stream");
                    break;
                },
                Ok(bytes_read) => {
                    // Append the new data to our accumulated buffer
                    accumulated_data.extend_from_slice(&buffer[..bytes_read]);
                    
                    // Process all complete JPEG frames in the accumulated data
                    let mut position = 0;
                    while position + 4 < accumulated_data.len() {
                        // Look for JPEG start marker
                        if accumulated_data[position] == 0xFF && accumulated_data[position + 1] == 0xD8 {
                            // Found start of JPEG, now look for end marker
                            let mut end_pos = position + 2;
                            let mut found_end = false;
                            
                            while end_pos + 1 < accumulated_data.len() {
                                if accumulated_data[end_pos] == 0xFF && accumulated_data[end_pos + 1] == 0xD9 {
                                    // Found end of JPEG
                                    found_end = true;
                                    
                                    // Extract the complete JPEG frame (including the end marker)
                                    let frame = accumulated_data[position..=end_pos+1].to_vec();
                                    
                                    // Get current queue size
                                    let current_queue = queue_size.load(Ordering::Relaxed);
                                    
                                    // Only send if queue isn't too full
                                    if current_queue < 50 {
                                        // Send frame and update queue size
                                        match tx.try_send(frame) {
                                            Ok(_) => {
                                                queue_size.fetch_add(1, Ordering::Relaxed);
                                            },
                                            Err(mpsc::error::TrySendError::Full(_)) => {
                                                println!("Channel full, skipping frame");
                                            },
                                            Err(e) => {
                                                eprintln!("Failed to send frame: {}", e);
                                            }
                                        }
                                    } else {
                                        // Skip frame if queue is too full
                                        println!("Network congested, skipping frame");
                                    }
                                    
                                    // Move position past this frame
                                    position = end_pos + 2;
                                    break;
                                }
                                end_pos += 1;
                            }
                            
                            if !found_end {
                                // Didn't find the end marker yet, need more data
                                break;
                            }
                        } else {
                            // Not a start marker, move to next byte
                            position += 1;
                        }
                    }
                    
                    // Keep only the unprocessed data
                    if position > 0 {
                        accumulated_data = accumulated_data[position..].to_vec();
                    }
                    
                    // Safety measure: if accumulated buffer gets too large without finding complete frames,
                    // clear part of it to avoid memory issues
                    if accumulated_data.len() > 10 * 1024 * 1024 {  // 10MB limit
                        println!("Buffer too large, discarding old data");
                        // Keep the last 1MB which might contain a partial frame
                        let keep_size = 1024 * 1024.min(accumulated_data.len());
                        accumulated_data = accumulated_data[accumulated_data.len() - keep_size..].to_vec();
                    }
                },
                Err(e) => {
                    eprintln!("Error reading GStreamer output: {}", e);
                    break;
                }
            }
            
            // Small yield to avoid hogging the CPU
            sleep(Duration::from_millis(1)).await;
        }
    });
}

async fn start_gstreamer(width: u32, height: u32, quality: u32) -> tokio::process::Child {
    println!("Starting GStreamer with resolution {}x{} and quality {}", width, height, quality);
    
    Command::new("gst-launch-1.0")
        .args(&[
            "libcamerasrc",
            "!",
            &format!("video/x-raw,width={},height={}", width, height),
            "!",
            "videoconvert",
            "!",
            "jpegenc",
            &format!("quality={}", quality),
            "!",
            "fdsink",
        ])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to start GStreamer with libcamerasrc")
}

async fn start_websocket_handler(
    _tx: mpsc::Sender<Vec<u8>>,
    mut rx: mpsc::Receiver<Vec<u8>>,
    quality: Arc<AtomicU32>,
    width: Arc<AtomicU32>,
    height: Arc<AtomicU32>,
    network_congested: Arc<AtomicBool>,
    queue_size: Arc<AtomicU64>,
    _camera_id: String
) {
    // Generate a unique camera ID
    let camera_id = generate_camera_id();
    let mut consecutive_failures = 0;
    let mut consecutive_successes = 0;
    
    tokio::spawn(async move {
        // Connect to the WebSocket server
        let url = url::Url::parse("ws://100.78.140.50:3001").expect("Failed to parse URL");
        match connect_async(url.clone()).await {
            Ok((ws_stream, _)) => {
                println!("Connected to WebSocket server");
                
                // Create a channel for communication between the two WebSocket tasks
                let (pong_tx, mut pong_rx) = mpsc::channel::<Message>(10);
                
                let (mut write, mut read) = ws_stream.split();
                
                // Send join message
                let join_message = json!({
                    "join": camera_id,
                    "capabilities": {
                        "adaptive_quality": true,
                        "min_quality": 20,
                        "max_quality": 90,
                        "resolutions": ["640x480", "1280x720"]
                    }
                }).to_string();
                
                if let Err(e) = write.send(Message::Text(join_message)).await {
                    eprintln!("Failed to send join message: {}", e);
                    return;
                }
                println!("Join message sent successfully");
                
                // Handle incoming messages (for server feedback)
                let pong_tx_clone = pong_tx.clone();
                let quality_clone = quality.clone();
                let width_clone = width.clone();
                let height_clone = height.clone();
                let network_congested_clone = network_congested.clone();
                
                // Spawn a task to handle incoming messages
                tokio::spawn(async move {
                    while let Some(msg) = read.next().await {
                        match msg {
                            Ok(Message::Text(text)) => {
                                // Parse server feedback for network conditions
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                    // Check if feedback contains network_feedback
                                    if let Some(feedback) = json.get("network_feedback") {
                                        // Explicitly set congestion state based on feedback
                                        if let Some(congestion) = feedback.get("congested") {
                                            if let Some(congested) = congestion.as_bool() {
                                                // Update the congestion flag
                                                network_congested_clone.store(congested, Ordering::Relaxed);
                                                
                                                // If server suggests quality change
                                                if let Some(suggested_quality) = feedback.get("suggested_quality") {
                                                    if let Some(q) = suggested_quality.as_u64() {
                                                        quality_clone.store(q as u32, Ordering::Relaxed);
                                                    }
                                                }
                                                
                                                // If server suggests resolution change
                                                if let Some(suggested_res) = feedback.get("suggested_resolution") {
                                                    if let Some(res) = suggested_res.as_str() {
                                                        if res == "640x480" {
                                                            width_clone.store(640, Ordering::Relaxed);
                                                            height_clone.store(480, Ordering::Relaxed);
                                                        } else if res == "1280x720" {
                                                            width_clone.store(1280, Ordering::Relaxed);
                                                            height_clone.store(720, Ordering::Relaxed);
                                                        }
                                                    }
                                                }
                                            }
                                        } else {
                                            // If "congested" field is missing, assume network is fine
                                            network_congested_clone.store(false, Ordering::Relaxed);
                                        }
                                    } else {
                                        // If no network_feedback, assume network is fine
                                        network_congested_clone.store(false, Ordering::Relaxed);
                                    }
                                }
                            },
                            Ok(Message::Ping(ping_data)) => {
                                // Send a pong message via the channel
                                let _ = pong_tx_clone.send(Message::Pong(ping_data)).await;
                            },
                            Err(e) => {
                                eprintln!("Error receiving message: {}", e);
                                break;
                            },
                            _ => {}
                        }
                    }
                });
                
                // Spawn a task to process frames and handle pongs
                tokio::spawn(async move {
                    // Process and send frames 
                    let capture_timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;
                    
                    loop {
                        tokio::select! {
                            Some(pong_msg) = pong_rx.recv() => {
                                if let Err(e) = write.send(pong_msg).await {
                                    eprintln!("Failed to send pong: {}", e);
                                    consecutive_failures += 1;
                                    consecutive_successes = 0;
                                } else {
                                    consecutive_successes += 1;
                                    if consecutive_successes > 4 {
                                        // After 4 successful messages, assume network is good
                                        network_congested.store(false, Ordering::Relaxed);
                                        consecutive_failures = 0;
                                    }
                                }
                            }
                            Some(frame) = rx.recv() => {
                                queue_size.fetch_sub(1, Ordering::Relaxed);
                                
                                let current_width = width.load(Ordering::Relaxed);
                                let current_height = height.load(Ordering::Relaxed);
                                let current_quality = quality.load(Ordering::Relaxed);
                                let current_queue = queue_size.load(Ordering::Relaxed);
                                
                                let encoded_frame = BASE64_STANDARD.encode(&frame);
                                let payload = json!({
                                    "camera_id": camera_id,
                                    "data": encoded_frame,
                                    "timestamp": capture_timestamp,
                                    "stats": {
                                        "resolution": format!("{}x{}", current_width, current_height),
                                        "quality": current_quality
                                    }
                                }).to_string();
                                
                                match write.send(Message::Text(payload)).await {
                                    Ok(_) => {
                                        // Frame sent successfully
                                        consecutive_successes += 1;
                                        consecutive_failures = 0;
                                        
                                        // If we have several successful sends, assume network is good
                                        if consecutive_successes > 10 {
                                            if network_congested.load(Ordering::Relaxed) {
                                                network_congested.store(false, Ordering::Relaxed);
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        eprintln!("Failed to send frame: {}", e);
                                        consecutive_failures += 1;
                                        consecutive_successes = 0;

                                        // If we have several failures in a row, mark network as congested
                                        if consecutive_failures > 3 {
                                            network_congested.store(true, Ordering::Relaxed);
                                        }
                                        
                                        // Connection might be down, retry after a delay
                                        sleep(Duration::from_secs(5)).await;
                                        
                                        // Try to reconnect
                                        match connect_async(url.clone()).await {
                                            Ok((new_ws_stream, _)) => {
                                                let (new_write, _) = new_ws_stream.split();
                                                write = new_write;
                                                
                                                // Send join message again
                                                let rejoin_message = json!({
                                                    "join": camera_id
                                                }).to_string();
                                                
                                                if let Err(e) = write.send(Message::Text(rejoin_message)).await {
                                                    eprintln!("Failed to send rejoin message: {}", e);
                                                    break;
                                                }
                                            },
                                            Err(e) => {
                                                eprintln!("Failed to reconnect: {}", e);
                                                break;
                                            }
                                        }
                                    }
                                }
                                
                                // Dynamic delay based on network conditions
                                let congestion_state = network_congested.load(Ordering::Relaxed);
                                let delay = if congestion_state {
                                    Duration::from_millis(100)  // More delay when congested
                                } else {
                                    Duration::from_millis(10)   // Less delay when network is good
                                };
                                
                                // Backoff based on queue size too
                                let queue_delay = if current_queue > 30 {
                                    Duration::from_millis(50)  // Additional delay when queue is building up
                                } else {
                                    Duration::from_millis(0)   // No additional delay when queue is small
                                };
                                
                                sleep(delay + queue_delay).await;
                            }
                            else => break,
                        }
                    }
                });
            },
            Err(e) => {
                eprintln!("Failed to connect to WebSocket server: {}", e);
            }
        }
    });
}

/// Generate a unique camera ID using UUID
fn generate_camera_id() -> String {
    let camera_id = Uuid::new_v4().to_string();
    format!("camera-rust-{}", camera_id)
}

#[tokio::main]
async fn main() {
    let quality = Arc::new(AtomicU32::new(70));
    let resolution_width = Arc::new(AtomicU32::new(1280));
    let resolution_height = Arc::new(AtomicU32::new(720));
    let network_congested = Arc::new(AtomicBool::new(false));
    let queue_size = Arc::new(AtomicU64::new(0));
    let mut network_state = NetworkState::new();
    
    let camera_id = generate_camera_id();
    println!("Generated camera ID: {}", camera_id);

    let quality_for_manager = quality.clone();
    let width_for_manager = resolution_width.clone();
    let height_for_manager = resolution_height.clone();
    let network_congested_for_manager = network_congested.clone();
    let queue_size_for_manager = queue_size.clone();

    let process_manager = tokio::spawn(async move {
        let mut current_quality = quality_for_manager.load(Ordering::Relaxed);
        let mut current_width = width_for_manager.load(Ordering::Relaxed);
        let mut current_height = height_for_manager.load(Ordering::Relaxed);
        let mut gstreamer_process = start_gstreamer(current_width, current_height, current_quality).await;
        let mut network_state = NetworkState::new();
        let mut consecutive_failures: u32 = 0;
        let mut consecutive_successes: u32 = 0;
    
        let mut stdout = gstreamer_process.stdout.take().expect("Failed to capture GStreamer stdout");
        let (tx, rx) = mpsc::channel::<Vec<u8>>(60);
    
        let tx_clone = tx.clone();
        
        // Fix: Use the original atomic references
        start_websocket_handler(
            tx_clone,
            rx,
            quality_for_manager.clone(),
            width_for_manager.clone(),
            height_for_manager.clone(),
            network_congested_for_manager.clone(),
            queue_size_for_manager.clone(),
            camera_id.clone()
        ).await;
        
        process_frames(stdout, tx.clone(), queue_size_for_manager.clone()).await;
        
        loop {
            // Get current metrics
            let queue_size_now = queue_size_for_manager.load(Ordering::Relaxed);
            let server_congestion = network_congested_for_manager.load(Ordering::Relaxed);
            
            // Update local metrics tracking
            if server_congestion || queue_size_now > 15 {
                consecutive_failures = (consecutive_failures + 1).min(10);
                consecutive_successes = 0;
            } else {
                consecutive_successes = (consecutive_successes + 1).min(30);
                if consecutive_failures > 0 {
                    consecutive_failures -= 1;
                }
            }
            
            // Get resolution and quality recommendations from network state
            let (is_congested, recommended_width, recommended_quality) = 
                network_state.update_congestion(queue_size_now, consecutive_failures, server_congestion);
            
            // Calculate recommended height based on width (16:9 or 4:3 aspect ratio)
            let recommended_height = if recommended_width == 1280 { 720 } else { 480 };
            
            // Update atomic values for other threads
            network_congested_for_manager.store(is_congested, Ordering::Relaxed);
            
            // Check if we need to change GStreamer settings
            let significant_change = recommended_quality.abs_diff(current_quality) > 5 || 
                                    recommended_width != current_width || 
                                    recommended_height != current_height;
                                    
            if significant_change {
                println!("Adjusting camera: Quality={}, Resolution={}x{}, Queue={}, Congestion={}", 
                        recommended_quality, recommended_width, recommended_height, queue_size_now, is_congested);
                        
                // Update atomic values
                quality_for_manager.store(recommended_quality, Ordering::Relaxed);
                width_for_manager.store(recommended_width, Ordering::Relaxed);
                height_for_manager.store(recommended_height, Ordering::Relaxed);
                
                // Restart GStreamer with new settings
                let _ = gstreamer_process.kill().await;
                gstreamer_process = start_gstreamer(recommended_width, recommended_height, recommended_quality).await;
                stdout = gstreamer_process.stdout.take().expect("Failed to capture GStreamer stdout");
                process_frames(stdout, tx.clone(), queue_size_for_manager.clone()).await;
                
                // Update current values
                current_quality = recommended_quality;
                current_width = recommended_width;
                current_height = recommended_height;
            }
            
            // Check less frequently when stable
            let check_interval = if network_state.stability_counter > 15 {
                Duration::from_secs(5)
            } else {
                Duration::from_secs(2)
            };
            
            sleep(check_interval).await;
        }
    });
    
    let _ = process_manager.await;
}
