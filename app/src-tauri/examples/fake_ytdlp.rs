//! Fake yt-dlp for tests (never touches the network). Copy this exe somewhere and put `<exe>.json` next to it:
//! {"fixture": "<video file served as the stream>", "captions": "<vtt file, optional>", "live": bool,
//!  "broken_until_update": bool, "always_broken": bool, "update_fails": bool, "bad_first_cut": bool, "bad_cuts": bool}.
//! Every call is appended to `<exe>.state/calls.log`; `-U` calls and cuts are counted in `updates` and `cuts`.

use serde_json::{json, Value};
use std::{fs, io::Write, path::Path, process::{exit, Command}};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let exe = std::env::current_exe().unwrap();
    let cfg: Value = fs::read(exe.with_extension("json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(json!({}));
    let state = exe.with_extension("state");
    fs::create_dir_all(&state).unwrap();
    let mut log = fs::OpenOptions::new().create(true).append(true).open(state.join("calls.log")).unwrap();
    writeln!(log, "{}", args.join(" ")).unwrap();
    let flag = |k: &str| cfg[k].as_bool().unwrap_or(false);
    let count = |name: &str| fs::read_to_string(state.join(name)).ok().and_then(|s| s.trim().parse::<u32>().ok()).unwrap_or(0);
    let bump = |name: &str| {
        let n = count(name) + 1;
        fs::write(state.join(name), n.to_string()).unwrap();
        n
    };
    let after = |a: &str| args.iter().position(|x| x == a).and_then(|i| args.get(i + 1).cloned());

    if args.first().map(String::as_str) == Some("-U") {
        bump("updates");
        if flag("update_fails") {
            eprintln!("ERROR: unable to update");
            exit(1);
        }
        println!("yt-dlp is up to date");
        return;
    }
    if flag("always_broken") || (flag("broken_until_update") && count("updates") == 0) {
        eprintln!("ERROR: [youtube] abc: Unable to extract player response; please report this issue");
        exit(1);
    }
    let fixture = cfg["fixture"].as_str().expect("fixture in config");
    if args.iter().any(|a| a == "-J") {
        let mut info = json!({"title": "Fake VOD", "duration": 600, "fps": 30, "width": 320, "height": 180, "language": "en", "live_status": "not_live"});
        if flag("live") {
            info["is_live"] = json!(true);
            info["live_status"] = json!("is_live");
        }
        if cfg["captions"].is_string() {
            info["subtitles"] = json!({"en": [{"ext": "vtt"}]});
        }
        println!("{info}");
        return;
    }
    let template = after("-o").expect("-o").replace("%(ext)s", "mp4");
    if args.iter().any(|a| a == "--skip-download") {
        if let Some(vtt) = cfg["captions"].as_str() {
            fs::copy(vtt, format!("{template}.{}.vtt", after("--sub-langs").unwrap())).unwrap();
        }
    } else if let Some(section) = after("--download-sections") {
        let (s, e) = section.trim_start_matches('*').split_once('-').unwrap();
        let (s, e): (f64, f64) = (s.parse().unwrap(), e.parse().unwrap());
        let n = bump("cuts");
        let len = if flag("bad_cuts") || (flag("bad_first_cut") && n == 1) { 2.0 } else { e - s };
        let status = Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-ss", &s.to_string(), "-t", &len.to_string(), "-i", fixture])
            .args(["-c:v", "libx264", "-preset", "ultrafast", "-c:a", "aac"])
            .arg(Path::new(&template))
            .status()
            .unwrap();
        exit(status.code().unwrap_or(1));
    } else {
        fs::copy(fixture, &template).unwrap(); // audio-only download: the fixture's audio track is inside
    }
}
