//! Measures preview decode throughput: `cargo run --release --example decode_bench -- video.mp4`
use telemetryvibe::video::decoder::{DecoderConfig, VideoEngine};
use telemetryvibe::video::{ffmpeg, probe};
use std::time::{Duration, Instant};

fn main() {
    let path = std::path::PathBuf::from(std::env::args().nth(1).expect("video path"));
    let ff = ffmpeg::detect(None).expect("ffmpeg");
    let info = probe::probe(&ff, &path).expect("probe");
    for hw in [false, true] {
        let engine = VideoEngine::new(|| {});
        engine.configure(DecoderConfig {
            ffmpeg: ff.ffmpeg.clone(),
            source: path.clone(),
            fps: info.fps,
            duration: info.duration,
            out_w: 1100,
            out_h: 620,
            hwaccel_args: if hw { ff.hwaccel_args() } else { vec![] },
            hwaccel_for_seeks: hw,
        });
        let t0 = Instant::now();
        let g = engine.seek(12.0);
        loop {
            let f = engine.frames.recv_timeout(Duration::from_secs(10)).unwrap();
            if f.generation == g {
                break;
            }
        }
        let seek_ms = t0.elapsed().as_millis();
        let g = engine.play(12.0);
        let t0 = Instant::now();
        let mut n = 0;
        let mut first = None;
        while t0.elapsed() < Duration::from_secs(3) {
            if let Ok(f) = engine.frames.recv_timeout(Duration::from_millis(500))
                && f.generation == g
            {
                n += 1;
                first.get_or_insert(t0.elapsed());
            }
        }
        println!(
            "hw={hw} seek={seek_ms}ms first_frame={:?} play_fps={:.1}",
            first,
            n as f64 / 3.0
        );
    }
}
