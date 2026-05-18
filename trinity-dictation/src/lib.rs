//! Trinity Dictation - voice input module.
//!
//! Records microphone audio while the dictation hotkey is held, sends the
//! resulting WAV audio to ElevenLabs Speech to Text, and injects recognized
//! text into the currently focused input.

use std::{
    cell::RefCell,
    fmt,
    rc::Rc,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use cpal::{
    SampleFormat, SampleRate, Stream,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use enigo::{Enigo, Keyboard, Settings};
use reqwest::blocking::{Client, multipart};
use serde::Deserialize;
use trinity_util::DictationConfig;

const ELEVENLABS_SPEECH_TO_TEXT_URL: &str = "https://api.elevenlabs.io/v1/speech-to-text";

/// Initialize the dictation module.
pub fn init() {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictationStatus {
    Idle,
    Recording,
    Transcribing,
    Error(String),
}

#[derive(Clone)]
pub struct DictationManager {
    state: Arc<Mutex<DictationManagerState>>,
    recorder: Rc<RefCell<Option<AudioRecorder>>>,
    transcriber: Arc<dyn SpeechToText>,
    injector: Arc<dyn TextInjector>,
}

struct DictationManagerState {
    config: DictationConfig,
    status: DictationStatus,
}

impl DictationManager {
    #[must_use]
    pub fn new(config: DictationConfig) -> Self {
        Self::with_services(
            config,
            Arc::new(ElevenLabsSpeechToText::default()),
            Arc::new(EnigoTextInjector),
        )
    }

    #[must_use]
    pub fn with_services(
        config: DictationConfig,
        transcriber: Arc<dyn SpeechToText>,
        injector: Arc<dyn TextInjector>,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(DictationManagerState {
                config: config.normalized(),
                status: DictationStatus::Idle,
            })),
            recorder: Rc::new(RefCell::new(None)),
            transcriber,
            injector,
        }
    }

    pub fn reload_config(&self, config: DictationConfig) {
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .config = config.normalized();
    }

    #[must_use]
    pub fn status(&self) -> DictationStatus {
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .status
            .clone()
    }

    pub fn start_recording(&self) -> Result<(), DictationError> {
        let mut state = self.state.lock().unwrap_or_else(|err| err.into_inner());
        match state.status {
            DictationStatus::Recording => return Err(DictationError::AlreadyRecording),
            DictationStatus::Transcribing => return Err(DictationError::Busy),
            DictationStatus::Idle | DictationStatus::Error(_) => {}
        }

        if state.config.provider != DictationConfig::DEFAULT_PROVIDER {
            let message = format!("unsupported dictation provider: {}", state.config.provider);
            state.status = DictationStatus::Error(message.clone());
            return Err(DictationError::Config(message));
        }

        if state.config.api_key.is_empty() {
            let message = "ElevenLabs API key is required".to_string();
            state.status = DictationStatus::Error(message.clone());
            return Err(DictationError::Config(message));
        }

        let recorder = AudioRecorder::start()?;
        *self.recorder.borrow_mut() = Some(recorder);
        state.status = DictationStatus::Recording;
        Ok(())
    }

    pub fn stop_and_transcribe(&self) -> Result<(), DictationError> {
        let recorder = {
            let mut recorder = self.recorder.borrow_mut();
            let Some(recorder) = recorder.take() else {
                return Err(DictationError::NotRecording);
            };
            recorder
        };
        let (audio, config) = {
            let mut state = self.state.lock().unwrap_or_else(|err| err.into_inner());
            state.status = DictationStatus::Transcribing;
            (recorder.stop()?, state.config.clone())
        };

        let state = self.state.clone();
        let transcriber = self.transcriber.clone();
        let injector = self.injector.clone();
        thread::spawn(move || {
            let result = transcriber
                .transcribe(audio, &config)
                .and_then(|text| inject_text(injector.as_ref(), text));
            finish_transcription_state(&state, result);
        });

        Ok(())
    }

    #[cfg(test)]
    fn finish_transcription(&self, result: Result<(), DictationError>) {
        finish_transcription_state(&self.state, result);
    }
}

fn finish_transcription_state(
    state: &Arc<Mutex<DictationManagerState>>,
    result: Result<(), DictationError>,
) {
    let mut state = state.lock().unwrap_or_else(|err| err.into_inner());
    state.status = match result {
        Ok(()) => DictationStatus::Idle,
        Err(err) => DictationStatus::Error(err.to_string()),
    };
}

fn inject_text(injector: &dyn TextInjector, text: String) -> Result<(), DictationError> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(());
    }
    injector.input_text(text)
}

pub trait SpeechToText: Send + Sync {
    fn transcribe(
        &self,
        audio: AudioClip,
        config: &DictationConfig,
    ) -> Result<String, DictationError>;
}

pub trait TextInjector: Send + Sync {
    fn input_text(&self, text: &str) -> Result<(), DictationError>;
}

#[derive(Debug, Clone)]
pub struct AudioClip {
    bytes: Vec<u8>,
    file_name: String,
}

impl AudioClip {
    #[must_use]
    pub fn wav(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            file_name: "trinity-dictation.wav".to_string(),
        }
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

struct AudioRecorder {
    stream: Stream,
    samples: Arc<Mutex<Vec<i16>>>,
    sample_rate: u32,
    channels: u16,
}

impl AudioRecorder {
    fn start() -> Result<Self, DictationError> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(DictationError::NoInputDevice)?;
        let supported_config = device
            .default_input_config()
            .map_err(|err| DictationError::Audio(err.to_string()))?;
        let sample_format = supported_config.sample_format();
        let sample_rate = supported_config.sample_rate().0;
        let channels = supported_config.channels();
        let config = supported_config.into();
        let samples = Arc::new(Mutex::new(Vec::new()));
        let stream = build_input_stream(&device, &config, sample_format, &samples)?;
        stream
            .play()
            .map_err(|err| DictationError::Audio(err.to_string()))?;

        Ok(Self {
            stream,
            samples,
            sample_rate,
            channels,
        })
    }

    fn stop(self) -> Result<AudioClip, DictationError> {
        drop(self.stream);
        thread::sleep(Duration::from_millis(40));
        let samples = self
            .samples
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone();
        if samples.is_empty() {
            return Err(DictationError::Audio(
                "no microphone samples captured".to_string(),
            ));
        }
        encode_wav(samples, self.sample_rate, self.channels)
    }
}

fn build_input_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_format: SampleFormat,
    samples: &Arc<Mutex<Vec<i16>>>,
) -> Result<Stream, DictationError> {
    let err_fn = |err| log::warn!("dictation audio stream error: {err}");
    match sample_format {
        SampleFormat::F32 => {
            let samples = samples.clone();
            device.build_input_stream(
                config,
                move |data: &[f32], _| {
                    push_samples(
                        &samples,
                        data.iter()
                            .map(|sample| (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16),
                    )
                },
                err_fn,
                None,
            )
        }
        SampleFormat::I16 => {
            let samples = samples.clone();
            device.build_input_stream(
                config,
                move |data: &[i16], _| push_samples(&samples, data.iter().copied()),
                err_fn,
                None,
            )
        }
        SampleFormat::U16 => {
            let samples = samples.clone();
            device.build_input_stream(
                config,
                move |data: &[u16], _| {
                    push_samples(
                        &samples,
                        data.iter()
                            .map(|sample| (*sample as i32 - i16::MAX as i32 - 1) as i16),
                    );
                },
                err_fn,
                None,
            )
        }
        _ => {
            return Err(DictationError::Audio(format!(
                "unsupported microphone sample format: {sample_format:?}"
            )));
        }
    }
    .map_err(|err| DictationError::Audio(err.to_string()))
}

fn push_samples<I>(samples: &Arc<Mutex<Vec<i16>>>, data: I)
where
    I: IntoIterator<Item = i16>,
{
    samples
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .extend(data);
}

fn encode_wav(
    samples: Vec<i16>,
    sample_rate: u32,
    channels: u16,
) -> Result<AudioClip, DictationError> {
    let data_len = samples
        .len()
        .checked_mul(2)
        .and_then(|len| u32::try_from(len).ok())
        .ok_or_else(|| DictationError::Audio("recording is too large for WAV".to_string()))?;
    let byte_rate = sample_rate
        .checked_mul(u32::from(channels))
        .and_then(|rate| rate.checked_mul(2))
        .ok_or_else(|| DictationError::Audio("invalid WAV byte rate".to_string()))?;
    let block_align = channels
        .checked_mul(2)
        .ok_or_else(|| DictationError::Audio("invalid WAV block align".to_string()))?;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&SampleRate(sample_rate).0.to_le_bytes());
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    Ok(AudioClip::wav(bytes))
}

#[derive(Default)]
struct ElevenLabsSpeechToText {
    client: ElevenLabsClient,
}

impl SpeechToText for ElevenLabsSpeechToText {
    fn transcribe(
        &self,
        audio: AudioClip,
        config: &DictationConfig,
    ) -> Result<String, DictationError> {
        self.client.transcribe(audio, config)
    }
}

#[derive(Clone)]
pub struct ElevenLabsClient {
    client: Client,
    endpoint: String,
}

impl Default for ElevenLabsClient {
    fn default() -> Self {
        Self {
            client: Client::new(),
            endpoint: ELEVENLABS_SPEECH_TO_TEXT_URL.to_string(),
        }
    }
}

impl ElevenLabsClient {
    #[must_use]
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            endpoint: endpoint.into(),
        }
    }

    pub fn transcribe(
        &self,
        audio: AudioClip,
        config: &DictationConfig,
    ) -> Result<String, DictationError> {
        let file_part = multipart::Part::bytes(audio.bytes)
            .file_name(audio.file_name)
            .mime_str("audio/wav")
            .map_err(|err| DictationError::Http(err.to_string()))?;
        let mut form = multipart::Form::new()
            .part("file", file_part)
            .text("model_id", config.model_id.clone());
        if let Some(language_code) = &config.language_code {
            form = form.text("language_code", language_code.clone());
        }

        let response = self
            .client
            .post(&self.endpoint)
            .header("xi-api-key", &config.api_key)
            .multipart(form)
            .send()
            .map_err(|err| DictationError::Http(err.to_string()))?;
        let status = response.status();
        let body = response
            .text()
            .map_err(|err| DictationError::Http(err.to_string()))?;
        if !status.is_success() {
            return Err(DictationError::Http(format!(
                "ElevenLabs request failed with {status}: {body}"
            )));
        }

        parse_elevenlabs_text(&body)
    }
}

#[derive(Debug, Deserialize)]
struct ElevenLabsTranscriptionResponse {
    text: String,
}

fn parse_elevenlabs_text(body: &str) -> Result<String, DictationError> {
    serde_json::from_str::<ElevenLabsTranscriptionResponse>(body)
        .map(|response| response.text)
        .map_err(|err| DictationError::Http(format!("invalid ElevenLabs response: {err}")))
}

struct EnigoTextInjector;

impl TextInjector for EnigoTextInjector {
    fn input_text(&self, text: &str) -> Result<(), DictationError> {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|err| DictationError::TextInjection(err.to_string()))?;
        enigo
            .text(text)
            .map_err(|err| DictationError::TextInjection(err.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictationError {
    AlreadyRecording,
    Busy,
    NotRecording,
    NoInputDevice,
    Config(String),
    Audio(String),
    Http(String),
    TextInjection(String),
}

impl fmt::Display for DictationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRecording => write!(f, "dictation is already recording"),
            Self::Busy => write!(f, "dictation is already transcribing"),
            Self::NotRecording => write!(f, "dictation is not recording"),
            Self::NoInputDevice => write!(f, "no default microphone input device available"),
            Self::Config(message) => write!(f, "{message}"),
            Self::Audio(message) => write!(f, "audio error: {message}"),
            Self::Http(message) => write!(f, "speech-to-text error: {message}"),
            Self::TextInjection(message) => write!(f, "text injection error: {message}"),
        }
    }
}

impl std::error::Error for DictationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Cursor,
        sync::atomic::{AtomicBool, Ordering},
    };

    #[test]
    fn parse_elevenlabs_response_returns_text() {
        let text =
            parse_elevenlabs_text(r#"{"text":"hello world"}"#).expect("response should parse");

        assert_eq!(text, "hello world");
    }

    #[test]
    fn parse_elevenlabs_response_rejects_invalid_json() {
        assert!(matches!(
            parse_elevenlabs_text(r#"{"unexpected":"shape"}"#),
            Err(DictationError::Http(_))
        ));
    }

    #[test]
    fn manager_rejects_start_without_api_key() {
        let manager = DictationManager::with_services(
            DictationConfig::default(),
            Arc::new(StubTranscriber::new("ignored")),
            Arc::new(RecordingInjector::default()),
        );

        assert!(matches!(
            manager.start_recording(),
            Err(DictationError::Config(_))
        ));
        assert!(matches!(manager.status(), DictationStatus::Error(_)));
    }

    #[test]
    fn manager_reports_not_recording_on_stop() {
        let manager = DictationManager::with_services(
            configured_dictation(),
            Arc::new(StubTranscriber::new("ignored")),
            Arc::new(RecordingInjector::default()),
        );

        assert_eq!(
            manager.stop_and_transcribe(),
            Err(DictationError::NotRecording)
        );
        assert_eq!(manager.status(), DictationStatus::Idle);
    }

    #[test]
    fn finish_transcription_sets_idle_on_success() {
        let manager = DictationManager::with_services(
            configured_dictation(),
            Arc::new(StubTranscriber::new("hello")),
            Arc::new(RecordingInjector::default()),
        );

        manager.finish_transcription(Ok(()));

        assert_eq!(manager.status(), DictationStatus::Idle);
    }

    #[test]
    fn finish_transcription_sets_error_on_failure() {
        let manager = DictationManager::with_services(
            configured_dictation(),
            Arc::new(StubTranscriber::new("hello")),
            Arc::new(RecordingInjector::default()),
        );

        manager.finish_transcription(Err(DictationError::TextInjection("denied".to_string())));

        assert_eq!(
            manager.status(),
            DictationStatus::Error("text injection error: denied".to_string())
        );
    }

    #[test]
    fn inject_text_skips_blank_transcription() {
        let injector = RecordingInjector::default();

        inject_text(&injector, "  ".to_string()).expect("blank text should be ignored");

        assert_eq!(injector.take(), None);
    }

    #[test]
    fn inject_text_passes_trimmed_text_to_injector() {
        let injector = RecordingInjector::default();

        inject_text(&injector, " hello ".to_string()).expect("text should inject");

        assert_eq!(injector.take(), Some("hello".to_string()));
    }

    #[test]
    fn inject_text_returns_injection_error() {
        let injector = FailingInjector;

        assert!(matches!(
            inject_text(&injector, "hello".to_string()),
            Err(DictationError::TextInjection(_))
        ));
    }

    #[test]
    fn encode_wav_writes_readable_pcm_audio() {
        let clip = encode_wav(vec![0, i16::MAX, i16::MIN], 16_000, 1)
            .expect("wav encoding should succeed");
        let reader = hound::WavReader::new(Cursor::new(clip.bytes()))
            .expect("encoded wav should be readable");

        assert_eq!(reader.spec().sample_rate, 16_000);
        assert_eq!(reader.spec().channels, 1);
    }

    #[test]
    fn manager_finishes_after_background_transcription() {
        let injector = Arc::new(RecordingInjector::default());
        let manager = DictationManager::with_services(
            configured_dictation(),
            Arc::new(StubTranscriber::new("hello")),
            injector.clone(),
        );

        manager.finish_transcription(
            StubTranscriber::new("hello")
                .transcribe(AudioClip::wav(vec![1, 2, 3]), &configured_dictation())
                .and_then(|text| inject_text(injector.as_ref(), text)),
        );

        assert_eq!(manager.status(), DictationStatus::Idle);
        assert_eq!(injector.take(), Some("hello".to_string()));
    }

    #[test]
    fn stub_transcriber_can_signal_failure() {
        let transcriber = StubTranscriber {
            text: "hello".to_string(),
            fail: true,
            called: AtomicBool::new(false),
        };

        assert!(matches!(
            transcriber.transcribe(AudioClip::wav(vec![1]), &configured_dictation()),
            Err(DictationError::Http(_))
        ));
    }

    fn configured_dictation() -> DictationConfig {
        DictationConfig {
            api_key: "test-key".to_string(),
            ..Default::default()
        }
    }

    struct StubTranscriber {
        text: String,
        fail: bool,
        called: AtomicBool,
    }

    impl StubTranscriber {
        fn new(text: &str) -> Self {
            Self {
                text: text.to_string(),
                fail: false,
                called: AtomicBool::new(false),
            }
        }
    }

    impl SpeechToText for StubTranscriber {
        fn transcribe(
            &self,
            _audio: AudioClip,
            _config: &DictationConfig,
        ) -> Result<String, DictationError> {
            self.called.store(true, Ordering::Relaxed);
            if self.fail {
                Err(DictationError::Http("failed".to_string()))
            } else {
                Ok(self.text.clone())
            }
        }
    }

    #[derive(Default)]
    struct RecordingInjector {
        text: Mutex<Option<String>>,
    }

    impl RecordingInjector {
        fn take(&self) -> Option<String> {
            self.text
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .take()
        }
    }

    impl TextInjector for RecordingInjector {
        fn input_text(&self, text: &str) -> Result<(), DictationError> {
            *self.text.lock().unwrap_or_else(|err| err.into_inner()) = Some(text.to_string());
            Ok(())
        }
    }

    struct FailingInjector;

    impl TextInjector for FailingInjector {
        fn input_text(&self, _text: &str) -> Result<(), DictationError> {
            Err(DictationError::TextInjection("failed".to_string()))
        }
    }
}
