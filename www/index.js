import init, { App } from "../pkg/viber.js";

async function run() {
  // Ensure DOM is ready
  if (document.readyState === "loading") {
    await new Promise((resolve) =>
      document.addEventListener("DOMContentLoaded", resolve),
    );
  }

  await init();

  const canvas = document.getElementById("canvas");

  if (!canvas) {
    console.error("Canvas element not found");
    return;
  }

  const app = new App();
  await app.init("canvas");

  let startTime = 0;
  let lastTime = 0;
  let scaledTime = 0;
  let currentFrame = 0;
  let totalFrames = 0;
  let audioProcessed = false;
  let isPlaying = false;
  let audioElement = null;
  let smoothingFactor = 0.2;
  let selectedBinSize = 64;
  let audioVolume = 0.3;

  // Animation loop
  function animate(time) {
    if (startTime === 0) {
      startTime = time;
      lastTime = time;
    }

    const deltaTime = time - lastTime;
    scaledTime += deltaTime;
    lastTime = time;

    // Calculate current frame for 120fps only if audio is processed and playing
    if (audioProcessed && totalFrames > 0 && isPlaying) {
      // Use audio current time for synchronization
      const audioCurrentTime = audioElement ? audioElement.currentTime : 0;
      const frameTime = audioCurrentTime * 120.0; // Convert to frame index
      currentFrame = Math.floor(frameTime) % totalFrames;
    } else {
      currentFrame = 0;
    }

    app.render(scaledTime / 1000.0, currentFrame, smoothingFactor);
    requestAnimationFrame(animate);
  }
  requestAnimationFrame(animate);

  // Handle canvas resize
  function resizeCanvas() {
    if (!canvas) {
      console.error("Canvas element not available for resize");
      return;
    }

    const container = document.querySelector(".container");
    if (!container) {
      console.error("Container element not found");
      return;
    }

    const containerWidth = container.clientWidth;
    const maxWidth = Math.min(containerWidth * 0.95, 800);
    const height = maxWidth * 0.75; // 4:3 aspect ratio

    // Set CSS size
    canvas.style.width = maxWidth + "px";
    canvas.style.height = height + "px";

    // Set actual canvas resolution
    const pixelRatio = window.devicePixelRatio || 1;
    canvas.width = maxWidth * pixelRatio;
    canvas.height = height * pixelRatio;

    // Notify WASM app of resize
    if (app && typeof app.resize === "function") {
      try {
        app.resize(canvas.width, canvas.height);
      } catch (e) {
        console.error("Error resizing app:", e);
      }
    }
  }

  window.addEventListener("resize", resizeCanvas);
  window.addEventListener("orientationchange", () => {
    setTimeout(resizeCanvas, 100);
  });

  // Handle touch events for better mobile interaction
  canvas.addEventListener("touchstart", (e) => {
    e.preventDefault();
  });

  canvas.addEventListener("touchmove", (e) => {
    e.preventDefault();
  });

  canvas.addEventListener("touchend", (e) => {
    e.preventDefault();
  });

  // Initial canvas resize with delay to ensure DOM is ready
  setTimeout(() => {
    resizeCanvas();
  }, 0);

  // File upload handling (placeholder for now)
  const uploadBtn = document.getElementById("upload-btn");
  const audioFile = document.getElementById("audio-file");

  function triggerFileUpload() {
    audioFile.click();
  }

  uploadBtn.addEventListener("click", triggerFileUpload);
  uploadBtn.addEventListener("touchend", (e) => {
    e.preventDefault();
    triggerFileUpload();
  });

  audioFile.addEventListener("change", (e) => {
    const file = e.target.files[0];
    if (file) {
      // Validate file type
      if (!file.name.toLowerCase().endsWith(".wav")) {
        alert("Please select a WAV file (.wav)");
        audioFile.value = ""; // Clear the input
        uploadBtn.innerHTML = "<span>Upload WAV File</span>";
        return;
      }

      console.log("WAV file selected:", file.name);
      uploadBtn.innerHTML = `<span>Selected: ${file.name}</span>`;

      // Read the file as an ArrayBuffer
      const reader = new FileReader();
      reader.onload = function (e) {
        const arrayBuffer = e.target.result;
        const uint8Array = new Uint8Array(arrayBuffer);

        try {
          // Set bin size before processing
          app.set_bin_size(selectedBinSize);

          // Pass the audio data to WASM
          app.process_audio_file(uint8Array);
          totalFrames = app.get_total_frames();
          audioProcessed = true;

          // Create audio element for playback
          const audioBlob = new Blob([arrayBuffer], { type: "audio/wav" });
          const audioUrl = URL.createObjectURL(audioBlob);
          audioElement = new Audio(audioUrl);
          audioElement.loop = true;
          audioElement.volume = audioVolume;

          // Add audio event listeners
          audioElement.addEventListener("ended", () => {
            isPlaying = false;
            const playPauseBtn = document.getElementById("play-pause-btn");
            playPauseBtn.innerHTML = "<span>Play</span>";
          });

          audioElement.addEventListener("pause", () => {
            isPlaying = false;
            const playPauseBtn = document.getElementById("play-pause-btn");
            playPauseBtn.innerHTML = "<span>Play</span>";
          });

          audioElement.addEventListener("play", () => {
            isPlaying = true;
            const playPauseBtn = document.getElementById("play-pause-btn");
            playPauseBtn.innerHTML = "<span>Pause</span>";
          });

          // Enable play/pause button
          const playPauseBtn = document.getElementById("play-pause-btn");
          playPauseBtn.disabled = false;

          console.log("Audio file processed successfully");
          console.log("Total frames:", totalFrames);
          console.log("Audio visualization ready!");
        } catch (error) {
          console.error("Error processing audio file:", error);
          alert("Error processing audio file: " + error);
          audioProcessed = false;
          totalFrames = 0;
        }
      };

      reader.onerror = function (error) {
        console.error("Error reading file:", error);
        alert("Error reading file");
      };

      reader.readAsArrayBuffer(file);
    } else {
      uploadBtn.innerHTML = "<span>Upload WAV File</span>";
    }
  });

  // Bin size control handling
  const binSizeSelect = document.getElementById("bin-size-select");

  binSizeSelect.addEventListener("change", (e) => {
    selectedBinSize = parseInt(e.target.value);
    console.log("Selected bin size:", selectedBinSize);
  });

  // Smoothing control handling
  const smoothingSlider = document.getElementById("smoothing-slider");
  const smoothingValue = document.getElementById("smoothing-value");

  smoothingSlider.addEventListener("input", (e) => {
    smoothingFactor = parseFloat(e.target.value);
    smoothingValue.textContent = smoothingFactor.toFixed(1);
  });

  // Volume control handling
  const volumeSlider = document.getElementById("volume-slider");
  const volumeValue = document.getElementById("volume-value");

  volumeSlider.addEventListener("input", (e) => {
    audioVolume = parseFloat(e.target.value) / 100.0;
    volumeValue.textContent = e.target.value;
    if (audioElement) {
      audioElement.volume = audioVolume;
    }
  });

  // Play/Pause button handling
  const playPauseBtn = document.getElementById("play-pause-btn");

  function togglePlayPause() {
    if (!audioElement || !audioProcessed) return;

    if (isPlaying) {
      // Pause
      audioElement.pause();
      isPlaying = false;
      playPauseBtn.innerHTML = "<span>Play</span>";
    } else {
      // Play
      audioElement.play();
      isPlaying = true;
      playPauseBtn.innerHTML = "<span>Pause</span>";
    }
  }

  playPauseBtn.addEventListener("click", togglePlayPause);
  playPauseBtn.addEventListener("touchend", (e) => {
    e.preventDefault();
    togglePlayPause();
  });
}

run().catch(console.error);
