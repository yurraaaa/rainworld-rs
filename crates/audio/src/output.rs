//! A kira backend on cpal like kira's `CpalBackend`, except that dropping it stops and
//! joins the thread owning the stream.
//!
//! kira's backend only flags its stream thread, which notices within 500 ms and only
//! then closes the stream. A process exiting meanwhile ran ALSA's `snd_pcm_close` (it
//! `dlclose`s PipeWire modules) concurrently with glibc's exit-time teardown, which
//! unmapped the MangoHud Vulkan layer under its own polling thread: SIGSEGV on quit.
//!
//! Mirrors: - (kira/cpal backend glue)

use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use kira::backend::cpal::cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use kira::backend::cpal::cpal::{self, Device, ErrorKind, Stream, StreamConfig};
use kira::backend::{Backend, Renderer};

/// How often the stream thread checks for stream errors and default-device changes
/// (kira's `CHECK_STREAM_INTERVAL`).
const CHECK_INTERVAL: Duration = Duration::from_millis(500);

/// The default output device, played from a thread that is joined on drop.
pub struct CpalOutput {
    /// The device and config chosen by `setup`, until `start` hands them to the thread.
    pending: Option<(Device, StreamConfig)>,
    /// Dropping the sender stops the thread.
    thread: Option<(Sender<()>, JoinHandle<()>)>,
}

impl CpalOutput {
    fn default_device() -> Result<(Device, StreamConfig), String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("no default audio output device")?;
        let config = device
            .default_output_config()
            .map_err(|e| e.to_string())?
            .config();
        Ok((device, config))
    }
}

impl Backend for CpalOutput {
    type Settings = ();
    type Error = String;

    fn setup(_settings: (), _internal_buffer_size: usize) -> Result<(Self, u32), String> {
        let (device, config) = Self::default_device()?;
        let sample_rate = config.sample_rate;
        Ok((
            Self {
                pending: Some((device, config)),
                thread: None,
            },
            sample_rate,
        ))
    }

    fn start(&mut self, renderer: Renderer) -> Result<(), String> {
        let (device, config) = self.pending.take().ok_or("audio output already started")?;
        let (stop, stopped) = mpsc::channel();
        let (ready, started) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("rw-audio out".into())
            .spawn(move || StreamThread::run(renderer, &device, config, &ready, &stopped))
            .map_err(|e| e.to_string())?;
        let result = started
            .recv()
            .unwrap_or_else(|_| Err("the audio output thread died".into()));
        self.thread = Some((stop, thread));
        result
    }
}

impl Drop for CpalOutput {
    fn drop(&mut self) {
        if let Some((stop, thread)) = self.thread.take() {
            drop(stop);
            if thread.join().is_err() {
                log::warn!("the audio output thread panicked");
            }
        }
    }
}

/// The stream and what it needs to be rebuilt; lives on (and is dropped by) the
/// output thread, as cpal streams need not be `Send`.
struct StreamThread {
    renderer: Arc<Mutex<Renderer>>,
    stream: Option<Stream>,
    errors: Receiver<cpal::Error>,
    error_sender: Sender<cpal::Error>,
    device_id: String,
    sample_rate: u32,
}

impl StreamThread {
    fn run(
        renderer: Renderer,
        device: &Device,
        config: StreamConfig,
        ready: &Sender<Result<(), String>>,
        stopped: &Receiver<()>,
    ) {
        let (error_sender, errors) = mpsc::channel();
        let mut this = Self {
            renderer: Arc::new(Mutex::new(renderer)),
            stream: None,
            errors,
            error_sender,
            device_id: String::new(),
            sample_rate: config.sample_rate,
        };
        let started = this.start_stream(device, config);
        let failed = started.is_err();
        // The caller waits for this; if it is gone there is nobody to play for.
        if ready.send(started).is_err() || failed {
            return;
        }
        while let Err(RecvTimeoutError::Timeout) = stopped.recv_timeout(CHECK_INTERVAL) {
            this.check_stream();
        }
    }

    fn device_id(device: &Device) -> String {
        device
            .id()
            .map_or_else(|_| "device id unavailable".into(), |id| id.to_string())
    }

    /// kira's `StreamManager::check_stream`: rebuilds the stream on the default device
    /// when the device went away, or when the default device or its rate changed.
    fn check_stream(&mut self) {
        let mut lost = self.stream.is_none();
        while let Ok(error) = self.errors.try_recv() {
            lost |= matches!(
                error.kind(),
                ErrorKind::DeviceNotAvailable | ErrorKind::StreamInvalidated
            );
        }
        let Ok((device, config)) = CpalOutput::default_device() else {
            return;
        };
        if lost
            || Self::device_id(&device) != self.device_id
            || config.sample_rate != self.sample_rate
        {
            self.stream = None;
            if let Err(e) = self.start_stream(&device, config) {
                log::warn!("restarting the audio output: {e}");
            }
        }
    }

    fn start_stream(&mut self, device: &Device, config: StreamConfig) -> Result<(), String> {
        if config.sample_rate != self.sample_rate {
            self.renderer
                .lock()
                .on_change_sample_rate(config.sample_rate);
        }
        self.device_id = Self::device_id(device);
        self.sample_rate = config.sample_rate;
        let renderer = Arc::clone(&self.renderer);
        let errors = self.error_sender.clone();
        let channels = config.channels;
        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    // Only this callback locks it while the stream plays.
                    if let Some(mut renderer) = renderer.try_lock() {
                        renderer.on_start_processing();
                        renderer.process(data, channels);
                    } else {
                        data.fill(0.0);
                    }
                },
                move |error| {
                    errors.send(error).ok();
                },
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        self.stream = Some(stream);
        Ok(())
    }
}
