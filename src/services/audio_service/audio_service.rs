// External.
use rusty_audio::Audio;
use sfml::audio::SoundRecorderDriver;
use sfml::audio::SoundSource;
use sfml::audio::SoundStreamPlayer;
use system_wide_key_state::*;

// Std.
use std::collections::VecDeque;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

// Custom.
use super::voice_player::*;
use super::voice_recorder::*;
use crate::global_params::*;
use crate::services::net_service::*;

const INTERVAL_PROCESS_VOICE_MS: i32 = 10;
const INTERVAL_WAIT_FOR_NEW_CHUNKS: u64 = 10;
const SAMPLES_IN_CHUNK: usize = 679; // 35 ms (= 'sampleRate' (19400) * 0.035)
const SAMPLE_RATE: u32 = 19400;
const MIN_CHUNKS_TO_RECORD: usize = 6;
const MIN_CHUNKS_TO_START_PLAY: usize = 3;
const INTERVAL_CHECK_PUSH_TO_TALK_MS: u64 = 5;

pub struct UserVoiceData {
    pub username: String,
    pub user_volume: i32,
    chunks: VecDeque<Vec<i16>>,
    mtx_output_playing: Mutex<bool>,
}

impl UserVoiceData {
    pub fn new(username: String) -> Self {
        UserVoiceData {
            username,
            chunks: VecDeque::new(),
            mtx_output_playing: Mutex::new(false),
            user_volume: 100,
        }
    }
}

pub struct AudioService {
    pub users_voice_data: Arc<Mutex<Vec<Arc<Mutex<UserVoiceData>>>>>,
    net_service: Option<Arc<Mutex<NetService>>>,
    mtx_listen_push_to_talk: Mutex<bool>,
    master_output_volume: i32,
}

impl Default for AudioService {
    fn default() -> Self {
        AudioService {
            net_service: None,
            mtx_listen_push_to_talk: Mutex::new(false),
            users_voice_data: Arc::new(Mutex::new(Vec::new())),
            master_output_volume: 0,
        }
    }
}

impl AudioService {
    pub fn init(&mut self, net_service: Arc<Mutex<NetService>>, master_volume: i32) {
        self.net_service = Some(net_service);
        self.master_output_volume = master_volume;
    }
    pub fn add_user_voice_chunk(&mut self, username: String, voice_data: Vec<i16>) {
        let users_voice_data_guard = self.users_voice_data.lock().unwrap();

        let mut found = false;
        let mut found_index = 0usize;

        for (i, user) in users_voice_data_guard.iter().enumerate() {
            if user.lock().unwrap().username == username {
                found = true;
                found_index = i;
                break;
            }
        }

        if found {
            let mut user_guard = users_voice_data_guard[found_index].lock().unwrap();
            user_guard.chunks.push_back(voice_data);
            if user_guard.chunks.len() == 1 {
                let mut play_guard = user_guard.mtx_output_playing.lock().unwrap();
                if *play_guard == false {
                    // start output
                    *play_guard = true; // playing
                    let user_copy = Arc::clone(&users_voice_data_guard[found_index]);
                    let master_volume = self.master_output_volume;
                    thread::spawn(move || {
                        AudioService::play_user_voice(user_copy, master_volume);
                    });
                }
            }
        } else {
            println!(
                "warning: can't find user ('{}') to add voice chunk at [{}, {}]",
                username,
                file!(),
                line!()
            );
            // // create new user voice data struct
            // let mut user_voice = UserVoiceData::new(username.clone());
            // {
            //     user_voice.chunks.push_back(voice_data);
            //     *user_voice.mtx_output_playing.lock().unwrap() = true;
            // }
            // users_voice_data_guard.push(Arc::new(Mutex::new(user_voice)));

            // let user_copy = Arc::clone(users_voice_data_guard.last().unwrap());
            // let master_volume = self.master_output_volume;
            // thread::spawn(move || {
            //     AudioService::play_user_voice(
            //         user_copy,
            //         users_voice_copy,
            //         username.clone(),
            //         master_volume,
            //     );
            // });
        }
    }
    pub fn start_waiting_for_voice(
        &self,
        push_to_talk_key: KeyCode,
        audio_service: Arc<Mutex<AudioService>>,
    ) {
        let mut guard = self.mtx_listen_push_to_talk.lock().unwrap();
        if *guard {
            // already listening
            return;
        } else {
            *guard = true;
        }

        thread::spawn(move || {
            AudioService::record_voice(push_to_talk_key, audio_service);
        });
    }
}

impl AudioService {
    pub fn play_user_voice(user: Arc<Mutex<UserVoiceData>>, master_volume: i32) {
        let mut stop = false;
        let mut last_time_recv_chunk = chrono::Local::now();

        loop {
            let mut sleep = true;
            {
                let user_guard = user.lock().unwrap();

                // check if end of voice message
                if let Some(chunk) = user_guard.chunks.back() {
                    last_time_recv_chunk = chrono::Local::now();
                    if chunk.len() == 0 {
                        // zero-sized chunk means end of voice message
                        // finished
                        stop = true;
                    }
                }

                if user_guard.chunks.len() >= MIN_CHUNKS_TO_START_PLAY {
                    sleep = false;
                }
            }

            let time_delta = chrono::Local::now() - last_time_recv_chunk;
            if time_delta.num_seconds() as u64 >= MAX_WAIT_TIME_IN_VOICE_PLAYER_SEC {
                stop = true;
            }

            if stop {
                // Clear data.
                {
                    let mut user_guard = user.lock().unwrap();
                    user_guard.chunks.clear();
                    *user_guard.mtx_output_playing.lock().unwrap() = false;
                }
                return;
            } else if sleep {
                thread::sleep(Duration::from_millis(INTERVAL_WAIT_FOR_NEW_CHUNKS as u64));
            } else {
                break;
            }
        }

        // Ready to play audio.
        let (sample_sender, sample_receiver) = mpsc::channel();
        let mut voice_player = VoicePlayer::new(sample_receiver, SAMPLE_RATE);
        let mut player = SoundStreamPlayer::new(&mut voice_player);

        let mut _sent_chunks: usize = 0;
        let mut _user_volume = 100;
        // Send initial chunks.
        {
            let mut user_guard = user.lock().unwrap();
            for chunk in user_guard.chunks.iter() {
                sample_sender.send(chunk.clone()).unwrap();
                _sent_chunks += 1;
            }
            user_guard.chunks.clear();
            _user_volume = user_guard.user_volume;
        }

        let mut volume_before = master_volume * _user_volume;
        player.set_volume(master_volume as f32 * (_user_volume as f32 / 100.0));
        player.play();

        // Wait for new chunks.
        thread::sleep(Duration::from_millis(INTERVAL_WAIT_FOR_NEW_CHUNKS as u64));

        stop = false;

        last_time_recv_chunk = chrono::Local::now();

        loop {
            let mut sleep = true;
            {
                let mut user_guard = user.lock().unwrap();

                if user_guard.chunks.len() != 0 {
                    if volume_before != master_volume * user_guard.user_volume {
                        volume_before = master_volume * user_guard.user_volume;
                        player.set_volume(master_volume as f32 * (_user_volume as f32 / 100.0));
                    }

                    sleep = false;
                    last_time_recv_chunk = chrono::Local::now();
                    for chunk in user_guard.chunks.iter() {
                        if chunk.len() == 0 {
                            // last chunk
                            stop = true;
                            // don't 'break' here, we need to send this to voice player
                        }
                        sample_sender.send(chunk.clone()).unwrap();
                    }
                    user_guard.chunks.clear();
                }
            }

            let time_delta = chrono::Local::now() - last_time_recv_chunk;
            if time_delta.num_seconds() as u64 >= MAX_WAIT_TIME_IN_VOICE_PLAYER_SEC {
                stop = true;
            }

            if stop {
                break;
            }
            if sleep {
                thread::sleep(Duration::from_millis(INTERVAL_WAIT_FOR_NEW_CHUNKS as u64));
            }
        }

        // Clear data.
        {
            let mut user_guard = user.lock().unwrap();
            user_guard.chunks.clear();
            *user_guard.mtx_output_playing.lock().unwrap() = false;
        }
    }
    pub fn record_voice(push_to_talk_key: KeyCode, audio_service: Arc<Mutex<AudioService>>) {
        let (sample_sender, sample_receiver) = mpsc::channel();
        let mut voice_recorder = VoiceRecorder::new(sample_sender);
        let mut driver = SoundRecorderDriver::new(&mut voice_recorder);

        driver.set_processing_interval(sfml::system::Time::milliseconds(INTERVAL_PROCESS_VOICE_MS));
        driver.set_channel_count(1);

        let mut push_to_talk_pressed = false;

        loop {
            if is_key_pressed(push_to_talk_key) && push_to_talk_pressed == false {
                push_to_talk_pressed = true;

                // Play push-to-talk sound.
                thread::spawn(move || {
                    let mut audio = Audio::new();
                    audio.add("sound", PUSH_TO_TALK_PRESS_SOUND);
                    audio.play("sound"); // Execution continues while playback occurs in another thread.
                    audio.wait(); // Block until sounds finish playing
                });

                driver.start(SAMPLE_RATE);

                let mut recorded_chunk_count = 0usize;
                let mut samples: Vec<i16> = Vec::new();
                let mut stop = false;

                loop {
                    let res = sample_receiver.recv();
                    if let Err(e) = res {
                        panic!("error: {} at [{}, {}]", e, file!(), line!());
                    }

                    let mut current_chunk = res.unwrap();

                    samples.append(&mut current_chunk);

                    while samples.len() >= SAMPLES_IN_CHUNK {
                        let voice_chunk: Vec<i16> = samples.drain(0..SAMPLES_IN_CHUNK).collect();

                        {
                            // Send to net service.
                            let audio_service_guard = audio_service.lock().unwrap();
                            let net_service_guard = audio_service_guard
                                .net_service
                                .as_ref()
                                .unwrap()
                                .lock()
                                .unwrap();

                            net_service_guard
                                .user_udp_service
                                .lock()
                                .unwrap()
                                .send_voice_message(voice_chunk);
                        }

                        recorded_chunk_count += 1;
                        if recorded_chunk_count >= MIN_CHUNKS_TO_RECORD {
                            // see if we need to stop
                            if is_key_pressed(push_to_talk_key) == false {
                                stop = true;
                                break;
                            }
                        }
                    }

                    if stop {
                        break;
                    }
                }

                driver.stop();

                // Play push-to-talk sound.
                thread::spawn(move || {
                    let mut audio = Audio::new();
                    audio.add("sound", PUSH_TO_TALK_UNPRESS_SOUND);
                    audio.play("sound"); // Execution continues while playback occurs in another thread.
                    audio.wait(); // Block until sounds finish playing
                });

                // Send emtpy packet as final
                {
                    let empty_data: Vec<i16> = Vec::new();
                    let audio_service_guard = audio_service.lock().unwrap();

                    let net_service_guard = audio_service_guard
                        .net_service
                        .as_ref()
                        .unwrap()
                        .lock()
                        .unwrap();
                    net_service_guard
                        .user_udp_service
                        .lock()
                        .unwrap()
                        .send_voice_message(empty_data);
                }
            } else if push_to_talk_pressed {
                push_to_talk_pressed = false;
            }

            thread::sleep(Duration::from_millis(INTERVAL_CHECK_PUSH_TO_TALK_MS));
        }
    }
}
